use std::fmt::{Debug, Formatter};
use std::fs;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

use anyhow::{bail, Result};
use bounded_vec_deque::BoundedVecDeque;
use ebur128::{EbuR128, Mode};
use hound::WavWriter;
use log::{debug, info, warn};
use rb::{Producer, RbConsumer, RbProducer, SpscRb, RB};
use regex::Regex;
use symphonia::core::audio::{Layout, SignalSpec};

use crate::audio::{get_input, AudioInput, AudioSpecification};
use crate::get_audio_inputs;

static NEXT_ID: AtomicU32 = AtomicU32::new(0);
static READ_TIMEOUT: Duration = Duration::from_millis(100);

pub struct BufferedRecorder {
    devices: Vec<Regex>,
    producers: Mutex<Vec<RingProducer>>,
    buffer_size: usize,
    buffer: Mutex<BoundedVecDeque<f32>>,
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
        // Buffer size is simple, 48 samples a milli for 2 channels..
        let buffer_size = (48 * 2) * buffer_millis;

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
            buffer: Mutex::new(BoundedVecDeque::new(buffer_size)),

            stop: Arc::new(AtomicBool::new(false)),
            is_ready: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn listen(&self) {
        debug!("Starting Audio Listener..");

        // We need to find a matching input..
        let mut input: Option<Box<dyn AudioInput>> = None;

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
                // Read the latest samples from the input..
                match input.as_mut().unwrap().read() {
                    Ok(samples) => {
                        if self.buffer_size > 0 {
                            let mut buffer = self.buffer.lock().unwrap();
                            for sample in &samples {
                                buffer.push_back(*sample);
                            }
                        }
                        for producer in self.producers.lock().unwrap().iter() {
                            let result = producer.producer.write(&samples);
                            if result.is_err() {
                                debug!("Error writing to producer: {:?}", result.err());
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
                        self.buffer.lock().unwrap().clear();
                    }
                }
            }
        }
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
            debug!("Possible problem locating the Sampler Output, available devices:");
            get_audio_inputs()
                .iter()
                .for_each(|name| debug!("{}", name));

            bail!("Attempted to start a recording on an unprepared Sampler");
        }

        // So this will likely be spawned in a different thread, to actually handle the record
        // process.. with path being the file path to handle!

        // We create a second long buffer for audio input as we need to continue receiving
        // audio while we're creating files, setting up the encoder, and handling the initial buffer.
        let ring_buf = SpscRb::<f32>::new(48000 * 2);
        let (ring_buf_producer, ring_buf_consumer) = (ring_buf.producer(), ring_buf.consumer());

        let producer_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

        // Add the producer to our handler
        self.add_producer(RingProducer {
            id: producer_id,
            producer: ring_buf_producer,
        });

        // Grab the contents of the buffer, and push it into a simple vec
        let mut pre_samples = vec![];
        if self.buffer_size > 0 {
            let buffer = self.buffer.lock().unwrap();
            let (front, back) = buffer.as_slices();
            for sample in front {
                pre_samples.push(*sample);
            }
            for sample in back {
                pre_samples.push(*sample);
            }
        }

        // Get the read buffer to pull a quarter of a second at a time..
        let mut read_buffer: [f32; 24000] = [0.0; 24000];

        // Prepare the Writer..
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec)?;

        // Set up the Audio Checker for volume..
        let mut ebu_r128 = EbuR128::new(2, 48000, Mode::I)?;
        let mut writing = false;

        // We are all setup, now write the contents of the buffer into the file..
        if self.buffer_size > 0 {
            writing = self.handle_samples(pre_samples, &mut ebu_r128, writing, &mut writer)?;
        }

        // Now jump into the current 'live' audio.
        while !state.stop.load(Ordering::Relaxed) {
            if let Ok(Some(samples)) =
                ring_buf_consumer.read_blocking_timeout(&mut read_buffer, READ_TIMEOUT)
            {
                // Read these out into a vec..
                let samples: Vec<f32> = Vec::from(&read_buffer[0..samples]);
                writing = self.handle_samples(samples, &mut ebu_r128, writing, &mut writer)?;
            }
        }

        // Flush and Finalise the WAV file..
        writer.flush()?;
        writer.finalize()?;

        // Before we do anything else, was any noise recorded?
        if !writing {
            // No noise received..
            info!("No Noise Received in recording, Cancelling.");
            fs::remove_file(path)?;
        }

        self.del_producer(producer_id);
        Ok(())
    }

    fn handle_samples(
        &self,
        samples: Vec<f32>,
        ebu_r128: &mut EbuR128,
        writing: bool,
        writer: &mut WavWriter<BufWriter<File>>,
    ) -> Result<bool> {
        let mut recording_started = writing;

        // Split into 50ms chunks
        for slice in samples.chunks(4800) {
            if !recording_started {
                recording_started = self.is_audio(ebu_r128, slice)?;
            }

            if recording_started {
                for sample in slice {
                    writer.write_sample(*sample)?;
                }
            }
        }
        Ok(recording_started)
    }

    fn is_audio(&self, ebu_r128: &mut EbuR128, samples: &[f32]) -> Result<bool> {
        // The GoXLR seems to have a noise floor of roughly -100dB, so we're going
        // to listen for anything louder than -80dB and consider that 'useful' audio.
        ebu_r128.add_frames_f32(samples)?;
        if let Ok(loudness) = ebu_r128.loudness_momentary() {
            if loudness > -45. {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn locate_device(&self) -> Option<String> {
        let device_list = get_audio_inputs();

        let device = device_list
            .iter()
            .find(|output| self.devices.iter().any(|pattern| pattern.is_match(output)))
            .cloned();

        if let Some(device) = &device {
            debug!("Found Device: {}", device);
            return Some(device.clone());
        }
        None
    }
}

impl Drop for BufferedRecorder {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
