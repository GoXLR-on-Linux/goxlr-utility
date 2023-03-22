use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Result};
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::ChannelCount;
use log::warn;
use rb::{Consumer, RbConsumer, RbProducer, SpscRb, RB};

use crate::audio::{AudioInput, AudioSpecification, OpenInputStream};
use crate::cpal::cpal_config::CpalConfiguration;

pub struct CpalRecord {
    stream: cpal::Stream,
    stream_closed: Arc<AtomicBool>,

    // Use a shared read buffer..
    buffer_consumer: Consumer<f32>,
    read_buffer: [f32; BUFFER_SIZE],
}

// Set max 'interim' buffer size to 200ms. This defines the maximum time between the audio being
// handled here, and the external reader pulling it out before samples are lost.
//
// This may seem a little high, but the buffered recorder needs to handle thread locks, mutexes,
// and redistributing the samples out to anything that needs them. This can lead to small delays
// when reading. I've seen this reach 6-7,000 samples before, so this buffer should be clear.
const BUFFER_SIZE: usize = 19200;

impl OpenInputStream for CpalRecord {
    fn open(spec: AudioSpecification) -> Result<Box<dyn AudioInput>> {
        // Ok, grab the device we want to open..
        let device = CpalConfiguration::get_device(spec.device, true)?;

        // Input
        let config = cpal::StreamConfig {
            channels: spec.spec.channels.count() as ChannelCount,
            sample_rate: cpal::SampleRate(spec.spec.rate),

            // Set the main read buffer at 10ms
            buffer_size: cpal::BufferSize::Fixed(960),
        };

        // Prepare the Read Buffer, grab the producer and consumer..
        let buffer = SpscRb::<f32>::new(BUFFER_SIZE);
        let buffer_producer = buffer.producer();
        let buffer_consumer = buffer.consumer();

        // Prepare a bool to close the reader if CPAL throws an error..
        let stream_closed = Arc::new(AtomicBool::new(false));
        let stream_closed_inner = stream_closed.clone();

        let stream = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if let Err(e) = buffer_producer.write(data) {
                    warn!("Error Writing Samples: {}", e);
                }
            },
            move |e| {
                warn!("Error on Recording Stream, Stopping.. {}", e);
                stream_closed_inner.store(true, Ordering::Relaxed);
            },
            None,
        )?;

        stream.play()?;

        Ok(Box::new(Self {
            stream,
            stream_closed,

            read_buffer: [0.0; BUFFER_SIZE],
            buffer_consumer,
        }))
    }
}

impl AudioInput for CpalRecord {
    fn read(&mut self) -> Result<Vec<f32>> {
        // Check if the Stream has been Closed by CPAL..
        if self.stream_closed.load(Ordering::Relaxed) {
            bail!("Audio Stream has been closed.");
        }

        // Attempt a read on any samples which may be present in the buffer. It's not the
        // end of the world if this times out, it could just imply there are no samples currently
        // being sent to the channel. We'll let CPAL handle if something has errored.
        let timeout = Duration::from_millis(250);
        let read = self
            .buffer_consumer
            .read_blocking_timeout(&mut self.read_buffer, timeout);

        if let Ok(Some(samples)) = read {
            return Ok(Vec::from(&self.read_buffer[0..samples]));
        };

        // No samples passed in, or timeout hit, return an empty vec..
        Ok(vec![])
    }

    fn flush(&mut self) {
        let _ = self.stream.pause();
    }
}
