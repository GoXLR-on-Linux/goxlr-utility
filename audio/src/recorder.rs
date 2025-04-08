use std::cmp::max;
use std::fmt::{Debug, Formatter};
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};
use std::{fs, vec};

use anyhow::{bail, Result};
use ebur128::{EbuR128, Mode};
use fancy_regex::Regex;
use hound::WavWriter;
use log::{debug, error, info, trace, warn};
use rb::{Producer, RbConsumer, RbError, RbProducer, SpscRb, RB};
use symphonia::core::audio::{Layout, SignalSpec};

use crate::audio::{get_input, AudioInput, AudioSpecification};
use crate::ringbuffer::RingBuffer;
use crate::{get_audio_inputs, AtomicF64};

static NEXT_ID: AtomicU32 = AtomicU32::new(0);
static READ_TIMEOUT: Duration = Duration::from_millis(100);
static CHECK_PERIOD: Duration = Duration::from_secs(60 * 15);

pub struct BufferedRecorder {
    devices: Vec<Regex>,
    producers: Mutex<Vec<RingProducer>>,
    buffer_size: usize,
    buffer: RingBuffer<f32>,
    stop: Arc<AtomicBool>,
    is_ready: Arc<AtomicBool>,
}

pub struct RingProducer {
    id: u32,
    producer: Producer<f32>,
}

#[derive(Debug, Clone)]
pub struct RecorderState {
    pub stop: Arc<AtomicBool>,
    pub gain: Arc<AtomicF64>,
}

impl Debug for BufferedRecorder {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BufferedRecorder")
            .field("device", &self.devices)
            .field("producers", &self.producers.lock().unwrap().len())
            .finish()
    }
}

impl BufferedRecorder {
    pub fn new(devices: Vec<String>, buffer_millis: usize) -> Result<Self> {
        // We need to attempt to accommodate for the time it takes between a user hitting a button
        // on the GoXLR, and us being signalled for record.
        //
        // From my testing, the time from 'Button Notification' to 'Ready for Samples' takes about
        // 4ms.
        //
        // On Linux, the polling time for 'Notifications' is 20ms, so we can safely assume that
        // 25ms is the absolute max amount of time between someone pressing a button, and the
        // audio handler being ready.
        //
        // On Windows, we receive 'Notification' messages via USB URB INTERRUPTS, which are far
        // more responsive, allowing us to get the Notification in under 5ms from when the button
        // is pressed, so a 15ms buffer should be sufficient.
        //
        // So we'll make this buffer OS relevant.
        let forced_buffer = if cfg!(target_os = "windows") {
            (48 * 2) * 15
        } else {
            (48 * 2) * 30
        };
        let user_buffer = (48 * 2) * buffer_millis;
        let buffer_size = max(forced_buffer, user_buffer);

        // Convert the list of Strings into a Regexp vec..
        let regex = devices
            .iter()
            .map(|expression| {
                Regex::new(expression)
                    .unwrap_or_else(|_| panic!("Unable to Parse Regular Expression: {expression}"))
            })
            .collect();

        Ok(Self {
            devices: regex,
            producers: Mutex::new(vec![]),

            buffer_size,
            //buffer: Mutex::new(BoundedVecDeque::new(buffer_size)),
            buffer: RingBuffer::new(buffer_size),

            stop: Arc::new(AtomicBool::new(false)),
            is_ready: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn listen(&self) {
        debug!("Starting Audio Listener..");

        // We need to find a matching input..
        let mut input: Option<Box<dyn AudioInput>> = None;

        let mut now = Instant::now();

        while !self.stop.load(Ordering::Relaxed) {
            if input.is_none() {
                // Try and locate the matching input device name..
                if let Some(device) = self.locate_device() {
                    let spec = AudioSpecification {
                        device: Some(device),
                        spec: SignalSpec::new_with_layout(48000, Layout::Stereo),
                        buffer: 0,
                    };

                    // Attempt to load the input stream on the device..
                    if let Ok(found_input) = get_input(spec) {
                        // We good, reset the loop so we can start work.
                        input.replace(found_input);
                        self.is_ready.store(true, Ordering::Relaxed);
                        continue;
                    }
                }

                // We don't have a device, and haven't found a device, wait and try again.
                sleep(Duration::from_millis(500));
                continue;
            } else {
                // Read the latest samples from the input...
                match input.as_mut().unwrap().read() {
                    Ok(samples) => {
                        if self.buffer_size > 0 {
                            if let Err(e) = self.buffer.write_into(&samples) {
                                warn!("Error writing samples to buffer: {}", e);
                            }
                        }
                        for producer in self.producers.lock().unwrap().iter() {
                            let result = producer.producer.write(&samples);
                            if let Err(e) = result {
                                match e {
                                    RbError::Full => {
                                        // This can happen when buffers are full prior to general
                                        // setup being complete, so we'll just ignore it for now.
                                    }
                                    e => {
                                        warn!("Error writing to producer: {:?}", e);
                                    }
                                }
                            }
                        }
                    }
                    Err(error) => {
                        // Something has gone wrong, we need to shut down, drop the input, and
                        // being again. Hopefully we can pick it back up!
                        warn!("Error Reading audio input: {}", error);
                        debug!("Shutting down input, and clearing buffer.");
                        input = None;
                        self.is_ready.store(false, Ordering::Relaxed);

                        // Clear the Buffer
                        self.buffer.clear();
                    }
                }
            }

            // Has a minute passed?
            if now.elapsed() > CHECK_PERIOD {
                // Update the timer for next poll regardless..
                now = Instant::now();

                if !self.producers.lock().unwrap().is_empty() {
                    // Something is actively recording, don't break the loop..
                    continue;
                }

                // If the EBU failed to initialise, we're SOL really..
                if let Ok(ebu) = &mut EbuR128::new(2, 48000, Mode::SAMPLE_PEAK) {
                    // Grab the samples from the buffer..
                    let samples = self.get_samples_from_buffer();

                    // Check if any of them would constitute 'recordable' audio..
                    let mut received_audio = false;
                    if let Ok(has_audio) = self.is_audio(ebu, samples.as_slice()) {
                        received_audio = has_audio;
                    }

                    // Push out and let it continue..
                    if received_audio {
                        continue;
                    }
                } else {
                    continue;
                }

                // If we get here, nothing has stopped us, tear down the audio handler, and sleep..
                input.unwrap().flush();
                input = None;
                self.is_ready.store(false, Ordering::Relaxed);
                self.buffer.clear();
            }
        }

        debug!("Audio Listener Terminated");
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    pub fn is_ready(&self) -> bool {
        self.is_ready.load(Ordering::Relaxed)
    }

    pub fn add_producer(&self, producer: RingProducer) {
        self.producers.lock().unwrap().push(producer);
    }

    pub fn del_producer(&self, producer_id: u32) {
        self.producers
            .lock()
            .unwrap()
            .retain(|x| x.id != producer_id);
    }

    pub fn record(&self, path: &Path, state: RecorderState) -> Result<()> {
        if !self.is_ready() {
            warn!("Possible problem locating the Sampler Output, available devices:");
            get_audio_inputs().iter().for_each(|name| info!("{}", name));

            bail!("Attempted to start a recording on an unprepared Sampler");
        }

        // So this will likely be spawned in a different thread, to actually handle the record
        // process.. with path being the file path to handle!

        // We create a 4-second buffer for audio input as we need to continue receiving
        // audio while we're creating files, setting up the encoder, and handling the initial buffer.
        let ring_buf = SpscRb::<f32>::new(48000 * 4);
        let (ring_buf_producer, ring_buf_consumer) = (ring_buf.producer(), ring_buf.consumer());

        let producer_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

        // Add the producer to our handler
        self.add_producer(RingProducer {
            id: producer_id,
            producer: ring_buf_producer,
        });

        // Grab the contents of the buffer, and push it into a simple vec
        let pre_samples = self.get_samples_from_buffer();

        // Get the read buffer to pull a quarter of a second at a time..
        let mut read_buffer: [f32; 24000] = [0.0; 24000];

        // Prepare the Writer..
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec)?;

        // EBU Prep is here to make sure that recent samples have hit a threshold to start recording.
        let mut ebu_prep_r128 = EbuR128::new(2, 48000, Mode::SAMPLE_PEAK)?;

        // EBU Rec is here to perform the needed gain calculations on what has already been recorded
        let mut ebu_rec_r128 = EbuR128::new(2, 48000, Mode::I)?;

        // Whether we're writing to a file.
        let mut writing = false;

        state.gain.store(2., Ordering::Relaxed);

        // We are all setup, now write the contents of the buffer into the file..
        if self.buffer_size > 0 {
            match self.handle_samples(
                pre_samples,
                &mut ebu_prep_r128,
                &mut ebu_rec_r128,
                writing,
                &mut writer,
            ) {
                Ok(result) => writing = result,
                Err(error) => {
                    error!("Error Writing Samples {}", error);
                    state.stop.store(true, Ordering::Relaxed);
                }
            };
        }

        // Now jump into the current 'live' audio. This is essentially a do-while loop, just to
        // make sure we don't lose the last 'chunk' of audio which may have been partially recorded
        // when the button was released.
        loop {
            if let Ok(Some(samples)) =
                ring_buf_consumer.read_blocking_timeout(&mut read_buffer, READ_TIMEOUT)
            {
                // Read these out into a vec..
                let samples: Vec<f32> = Vec::from(&read_buffer[0..samples]);
                match self.handle_samples(
                    samples,
                    &mut ebu_prep_r128,
                    &mut ebu_rec_r128,
                    writing,
                    &mut writer,
                ) {
                    Ok(result) => writing = result,
                    Err(error) => {
                        // Something's gone wrong, we need to fail safe..
                        error!("Error Writing Samples: {}", error);
                        writing = false;
                        state.stop.store(true, Ordering::Relaxed);
                    }
                }
            }
            if state.stop.load(Ordering::Relaxed) {
                break;
            }
        }

        // Flush and Finalise the WAV file..
        writer.flush()?;
        writer.finalize()?;

        // Before we do anything else, was any noise recorded?
        if !writing {
            // No noise received..
            info!("No Noise Received, or error in recording, Cancelling.");
            fs::remove_file(path)?;
        } else {
            // We have noise recorded, try to normalise it..
            let mut loudness = ebu_rec_r128.loudness_global()?;
            if loudness == f64::NEG_INFINITY {
                debug!("Unable to Obtain loudness in Mode I, trying M..");
                loudness = ebu_rec_r128.loudness_momentary()?;
            }

            if loudness == f64::NEG_INFINITY {
                debug!("Unable to Obtain loudness in Mode M, Setting Default..");
                state.gain.store(1.0, Ordering::Relaxed);
            } else {
                let target = -23.0;
                let gain_db = target - loudness;
                let value = f64::powf(10., gain_db / 20.);

                // If we need to multiply the input by over 200, we're pulling in something
                // *FAR* to quiet to handle properly, so we'll reject it.
                if value > 200. {
                    debug!("Received Noise too quiet, cannot handle sanely, Cancelling.");
                    fs::remove_file(path)?;
                } else {
                    state.gain.store(value, Ordering::Relaxed);
                }
            }
        }

        self.del_producer(producer_id);
        Ok(())
    }

    fn get_samples_from_buffer(&self) -> Vec<f32> {
        if self.buffer_size > 0 {
            return self.buffer.read_buffer().unwrap_or_else(|e| {
                warn!("Error Reading Samples from Buffer: {}", e);
                vec![]
            });
        }
        vec![]
    }

    fn handle_samples(
        &self,
        samples: Vec<f32>,
        ebu_prep_r128: &mut EbuR128,
        ebu_rec_r128: &mut EbuR128,
        writing: bool,
        writer: &mut WavWriter<BufWriter<File>>,
    ) -> Result<bool> {
        let mut recording_started = writing;

        // Split into 50ms chunks
        for slice in samples.chunks(4800) {
            if !recording_started {
                recording_started = self.is_audio(ebu_prep_r128, slice)?;
            }

            if recording_started {
                // We are recording, add the samples to the recorded gain calc
                let _ = ebu_rec_r128.add_frames_f32(slice);

                for sample in slice {
                    // Multiply the sample by 2^23, to convert to a pseudo I24
                    writer.write_sample((*sample * 8388608.0) as i32)?;
                }
            }
        }

        Ok(recording_started)
    }

    fn is_audio(&self, ebu_r128: &mut EbuR128, samples: &[f32]) -> Result<bool> {
        // We're going to check this on a 8 frame basis..
        for samples in samples.chunks(16) {
            ebu_r128.add_frames_f32(samples)?;

            // We're now going to take a look at the 'Loudness' of these 8 frames..
            if let Ok(loudness) = ebu_r128.loudness_window((samples.len() / 2) as u32) {
                // We have a target of -23dB, work out the distance from there..
                let target = -23.0;
                let gain_db = target - loudness;
                let value = f64::powf(10., gain_db / 20.);

                // So when we get here, loudness * value = -23dB, this gives 'value' a linear
                // distance from the target, so if we're having to multiply the samples over 200
                // times to get there, the source audio is likely too quiet to use.
                if value < 200. {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn locate_device(&self) -> Option<String> {
        let device_list = get_audio_inputs();

        let device = device_list
            .iter()
            .find(|output| {
                self.devices.iter().any(|pattern| {
                    if let Ok(result) = pattern.is_match(output) {
                        return result;
                    }
                    false
                })
            })
            .cloned();

        if let Some(device) = &device {
            trace!("Found Device: {}", device);
            return Some(device.clone());
        }
        None
    }
}

impl Drop for BufferedRecorder {
    fn drop(&mut self) {
        debug!("Recorder Dropped, stopping thread..");

        // We probably don't need to do this, as drop will only be called after the main
        // thread has terminated, but safety first :)
        self.stop.store(true, Ordering::Relaxed);
    }
}
