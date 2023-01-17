use crate::audio::get_input;
use crate::recorder::RecorderState;
use anyhow::Result;
use ebur128::{EbuR128, Mode};
use log::info;
use rb::{Producer, RbConsumer, RbProducer, SpscRb, RB};
use std::fmt::{Debug, Formatter};
use std::fs;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

// Experimental code, an open recorder with a buffer..
pub struct BufferedRecorder {
    device: Option<String>,
    producers: Mutex<Vec<Producer<f32>>>,
}

impl Debug for BufferedRecorder {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BufferedRecorder")
            .field("device", &self.device)
            .field("producers", &self.producers.lock().unwrap().len())
            .finish()
    }
}

impl BufferedRecorder {
    pub fn new(device: Option<String>) -> Result<Self> {
        Ok(Self {
            device,
            producers: Mutex::new(vec![]),
        })
    }

    pub fn listen(&self) {
        let mut input = get_input(self.device.clone()).unwrap();
        loop {
            if let Ok(samples) = input.read() {
                for producer in self.producers.lock().unwrap().iter() {
                    let _ = producer.write(&samples);
                }
            }
        }
    }

    pub fn add_producer(&self, producer: Producer<f32>) {
        self.producers.lock().unwrap().push(producer);
    }

    pub fn del_producer(&self, producer: Producer<f32>) {
        self.producers
            .lock()
            .unwrap()
            .retain(|x| !std::ptr::eq(x, &producer));
    }

    pub fn record(&self, path: &Path, state: RecorderState) -> Result<()> {
        // So this will likely be spawned in a different thread, to actually handle the record
        // process.. with path being the file path to handle!

        // Lets create a ring buffer to handle the audio..
        let ring_buf = SpscRb::<f32>::new(4096);
        let (ring_buf_producer, ring_buf_consumer) = (ring_buf.producer(), ring_buf.consumer());

        // Add the producer to our handler
        self.producers.lock().unwrap().push(ring_buf_producer);

        let mut read_buffer: [f32; 2048] = [0.0; 2048];

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
        let mut recording_started = false;

        // While 'Recording' - FIX.
        while !state.stop.load(Ordering::Relaxed) {
            if let Some(samples) = ring_buf_consumer.read_blocking(&mut read_buffer) {
                // Read these out into a vec..
                let samples: Vec<f32> = Vec::from(&read_buffer[0..samples]);

                if !recording_started {
                    ebu_r128.add_frames_f32(samples.as_slice())?;
                    if let Ok(loudness) = ebu_r128.loudness_momentary() {
                        // The GoXLR has a (rough) noise floor of about -100dB when you set a condenser
                        // to 1dB output, and disable the noise gate. So we're going to assume that
                        // anything over -80 is intended noise, and we should start recording.
                        if loudness > -80. {
                            recording_started = true;
                        }
                    }
                }

                if recording_started {
                    for sample in samples {
                        writer.write_sample(sample)?;
                    }
                }
            }
        }

        // Flush and Finalise the WAV file..
        writer.flush()?;
        writer.finalize()?;

        // Before we do anything else, was any noise recorded?
        if !recording_started {
            // No noise received..
            info!("No Noise Received in recording, Cancelling.");
            fs::remove_file(path)?;
        }
        Ok(())
    }
}
