use crate::audio::{AudioOutput, AudioSpecification, OpenOutputStream};
use crate::cpal::cpal_config::CpalConfiguration;
use anyhow::{bail, Result};
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::Stream;
use log::warn;
use rb::{Producer, RbConsumer, RbInspector, RbProducer, SpscRb, RB};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

// Create a 200ms Buffer Size for playback, this should be short enough to ensure there aren't
// any obvious delays when playing samples.
const BUFFER_SIZE: usize = 200;

pub(crate) struct CpalPlayback {
    stream: Stream,
    stream_closed: Arc<AtomicBool>,

    buffer: SpscRb<f32>,
    buffer_producer: Producer<f32>,
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
                buffer_size: cpal::BufferSize::Default,
            }
        };

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

        Ok(Box::new(Self {
            stream,
            stream_closed,

            buffer,
            buffer_producer,
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

        let mut position = 0;
        while let Some(written) = self
            .buffer_producer
            .write_blocking(samples.split_at(position).1)
        {
            position += written;
        }

        Ok(())
    }

    fn flush(&mut self) {
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

        let _ = self.stream.pause();
    }
}
