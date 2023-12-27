use crate::audio::{AudioOutput, AudioSpecification, OpenOutputStream};
use crate::cpal::cpal_config::CpalConfiguration;
use anyhow::{bail, Result};
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::Stream;
use log::{debug, warn};
use rb::{Producer, RbConsumer, RbInspector, RbProducer, SpscRb, RB};
use rubato::{FftFixedIn, Resampler};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

// Create a 50ms Buffer Size for playback, this should be short enough to ensure there aren't
// any obvious delays when playing samples.
const BUFFER_SIZE: usize = 50;

pub(crate) struct CpalPlayback {
    stream: Option<Stream>,
    stream_closed: Arc<AtomicBool>,

    buffer: SpscRb<f32>,
    buffer_producer: Producer<f32>,

    // Resampler Related Variables..
    resampler: Option<CpalResampler>,
}

struct CpalResampler {
    resampler: FftFixedIn<f32>,
    input_buffer: Vec<f32>,
    input: Vec<Vec<f32>>,
    output: Vec<Vec<f32>>,
    interleaved: Vec<f32>,
}

impl OpenOutputStream for CpalPlayback {
    fn open(spec: AudioSpecification) -> Result<Box<dyn AudioOutput>> {
        let device = CpalConfiguration::get_device(spec.device, false)?;

        let config = if cfg!(target_os = "windows") {
            // Windows expects the file to be resampled to the output config, so we can't use the
            // input audio. Instead, we gotta resample.
            device.default_output_config()?.config()
        } else {
            // MacOS will resample inside CoreAudio, so we send the samples directly.
            cpal::StreamConfig {
                channels: spec.spec.channels.count() as cpal::ChannelCount,
                sample_rate: cpal::SampleRate(spec.spec.rate),
                buffer_size: cpal::BufferSize::Fixed(64),
            }
        };

        // Before we go any further, is the channel count of the audio correct?
        if spec.spec.channels.count() != 2 {
            bail!("Only stereo audio is supported");
        }

        // Calculate the buffer size based on the sample count in BUFFER_SIZE milliseconds..
        let size = (BUFFER_SIZE * config.sample_rate.0 as usize) / 1000;
        let buffer_length = size * config.channels as usize;

        // Create the Actual Buffer
        let buffer = SpscRb::<f32>::new(buffer_length);
        let buffer_producer = buffer.producer();
        let buffer_consumer = buffer.consumer();

        // Prepare a bool to close the reader if CPAL throws an error..
        let stream_closed = Arc::new(AtomicBool::new(false));
        let stream_closed_inner = stream_closed.clone();

        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                // Read from the ring buffer, and write them to the data array
                let written = buffer_consumer.read(data).unwrap_or(0);

                // Data expects a certain number of samples, if we didn't get enough from above,
                // mute anything afterwards as we're probably EoS
                data[written..].iter_mut().for_each(|s| *s = 0.0);
            },
            move |e| {
                warn!("Error on Playback Stream, Stopping.. {}", e);
                stream_closed_inner.store(true, Ordering::Relaxed);
            },
            Some(Duration::from_millis(500)),
        )?;
        stream.play()?;

        // Do we need to resample?
        let resampler = if spec.spec.rate != config.sample_rate.0 {
            debug!(
                "Creating Resampler from {} to {}",
                spec.spec.rate, config.sample_rate.0
            );

            // Create a resampler..
            let resampler = FftFixedIn::<f32>::new(
                spec.spec.rate as usize,
                config.sample_rate.0 as usize,
                spec.buffer,
                2,
                spec.spec.channels.count(),
            )?;

            // Create a buffer to hold samples until we can resample..
            let input_buffer = Vec::with_capacity(spec.buffer * spec.spec.channels.count());

            // Allocate the Input and Output Buffers..
            let input = vec![vec![0_f32; spec.buffer]; spec.spec.channels.count()];
            let output = Resampler::output_buffer_allocate(&resampler);

            Some(CpalResampler {
                resampler,
                input_buffer,
                input,
                output,
                interleaved: vec![],
            })
        } else {
            None
        };

        Ok(Box::new(Self {
            stream: Some(stream),
            stream_closed,

            buffer,
            buffer_producer,

            resampler,
        }))
    }
}

impl AudioOutput for CpalPlayback {
    fn write(&mut self, samples: &[f32]) -> Result<()> {
        if self.stream_closed.load(Ordering::Relaxed) {
            bail!("Stream has been closed");
        }

        // Do nothing if there are no samples sent..
        if samples.is_empty() {
            return Ok(());
        }

        let out_samples = if let Some(resampler) = &mut self.resampler {
            // First thing we need to do, is append these samples to the input buffer..
            //dst.extend(src.iter().map(|&s| s.into_sample()));
            resampler.input_buffer.extend(samples);

            let required_samples = resampler.input[0].capacity() * resampler.input.len();
            if resampler.input_buffer.len() < required_samples {
                // Don't do anything with these samples, we're not ready.
                return Ok(());
            }

            // So, first problem we run into here, is that our samples are already
            // interleaved, and our resampler expects them to not be, so lets split them.
            resampler.input[0] = resampler.input_buffer.iter().step_by(2).copied().collect();
            resampler.input[1] = resampler
                .input_buffer
                .iter()
                .skip(1)
                .step_by(2)
                .copied()
                .collect();

            // Attempt to perform the resample
            let result = resampler.resampler.process_into_buffer(
                &resampler.input,
                &mut resampler.output,
                None,
            );

            // Regardless of whether this works or not, we should clear our buffer, lest we get stuck.
            resampler.input_buffer.clear();

            match result {
                Ok(_) => {
                    // We need to re-interleave the results, channels * channel length
                    let channels = resampler.output.len();

                    let length = channels * resampler.output[0].len();
                    if resampler.interleaved.len() != length {
                        resampler.interleaved.resize(length, 0_f32);
                    }

                    // Iterate over each frame, and replace the samples
                    for (i, frame) in resampler.interleaved.chunks_exact_mut(channels).enumerate() {
                        for (channel, sample) in frame.iter_mut().enumerate() {
                            *sample = resampler.output[channel][i];
                        }
                    }

                    // Send back the result as a slice
                    resampler.interleaved.as_slice()
                }
                Err(err) => {
                    debug!("Resampling Failed: {}, falling back", err);
                    samples
                }
            }
        } else {
            // No resampler needed, send back the samples
            samples
        };

        let mut position = 0;
        while let Some(written) = self
            .buffer_producer
            .write_blocking(out_samples.split_at(position).1)
        {
            position += written;
        }

        Ok(())
    }

    fn flush(&mut self) {
        if let Some(resampler) = &mut self.resampler {
            let length = resampler.input_buffer.len();
            let capacity = resampler.input_buffer.capacity();

            if length != 0 {
                // There's some stuff left in the buffer, fill the buffer to capacity to
                // flush it through to the output buffer.
                let filling_samples = vec![0.; capacity - length];
                let _ = self.write(&filling_samples);
            }
        }

        // Make sure the playback buffer is empty, to prevent premature pausing at
        // the end of playback
        while !self.buffer.is_empty() {
            // Make sure the Stream hasn't closed while handling the buffer..
            if self.stream_closed.load(Ordering::Relaxed) {
                return;
            }

            // Wait briefly for samples to flush
            std::thread::sleep(Duration::from_millis(5));
        }

        if let Some(stream) = &self.stream {
            let _ = stream.pause();
        }
    }

    fn stop(&mut self) {
        // We're going to take the stream and drop it, this should stop playback immediately.
        self.stream_closed.store(true, Ordering::Relaxed);
        self.stream.take();
    }
}
