use anyhow::Result;
use anyhow::{anyhow, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rb::*;
use std::time::Duration;
use symphonia::core::audio::SignalSpec;

use crate::audio::AudioOutput;
use log::{debug, error};

// Create a 200ms Buffer Size for playback, this should be short enough to ensure there aren't
// any obvious delays when playing samples.
const BUFFER_SIZE: usize = 200;

pub struct CpalAudioOutput;

impl CpalAudioOutput {
    pub fn open(spec: SignalSpec, device: Option<String>) -> Result<Box<dyn AudioOutput>> {
        // Device handling is a little more awkward on CPAL, we need the audio host and device.
        // We request them as 'HOST*DEVICE' on the Command Line, now we can look them up :p

        let mut cpal_device = None;

        // Basically, if *ANYTHING* goes wrong here, we'll fall through to default.
        if let Some(device_name) = device {
            if let Some(position) = device_name.find('*') {
                let str_host = &device_name[0..position];
                let str_device = &device_name[position + 1..device_name.len()];

                debug!("Searching For Host: {}, Device: {}", str_host, str_device);

                // Ok, now for cpal, find the correct host..
                let cpal_host_list = cpal::available_hosts();
                let host_id = cpal_host_list.iter().find(|x| x.name() == str_host);

                if let Some(host_id) = host_id {
                    debug!("Audio Host Found: {:?}", host_id);

                    if let Ok(host) = cpal::host_from_id(*host_id) {
                        // We have found our host, now try to find the device..
                        debug!("Looking For Device..");
                        if let Ok(mut devices) = host.output_devices() {
                            if let Some(device) = devices.find(|x| {
                                x.name().unwrap_or_else(|_| "UNKNOWN".to_string()) == str_device
                            }) {
                                debug!("Device Found.");
                                cpal_device = Some(device);
                            }
                        }
                    }
                }
            }
        }

        let final_device;
        if let Some(device) = cpal_device {
            final_device = device;
        } else {
            debug!("Device not found, looking for default..");
            let host = cpal::default_host();
            final_device = match host.default_output_device() {
                Some(device) => device,
                None => return Err(anyhow!("Unable to find Default Device")),
            };
        }

        // Select proper playback routine based on sample format.
        CpalAudioOutputImpl::try_open(spec, &final_device)
    }
}

struct CpalAudioOutputImpl {
    ring_buffer: SpscRb<f32>,
    ring_buf_producer: Producer<f32>,
    _stream: cpal::Stream,
}

impl CpalAudioOutputImpl {
    pub fn try_open(spec: SignalSpec, device: &cpal::Device) -> Result<Box<dyn AudioOutput>> {
        let num_channels = spec.channels.count();

        // Output audio stream config.
        let config = cpal::StreamConfig {
            channels: num_channels as cpal::ChannelCount,
            sample_rate: cpal::SampleRate(spec.rate),
            buffer_size: cpal::BufferSize::Default,
        };

        // Build the ring buffer based on the buffer size
        let ring_len = ((BUFFER_SIZE * spec.rate as usize) / 1000) * num_channels;
        let ring_buf = SpscRb::<f32>::new(ring_len);
        let (ring_buf_producer, ring_buf_consumer) = (ring_buf.producer(), ring_buf.consumer());

        let stream_result = device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                // Read from the ring buffer, and write them to the data array
                let written = ring_buf_consumer.read(data).unwrap_or(0);

                // Data expects a certain number of samples, if we didn't get enough from above,
                // mute anything afterwards as we're probably EoS
                data[written..].iter_mut().for_each(|s| *s = 0.0);
            },
            move |_| error!("Audio Output Error.."),
        );

        if stream_result.is_err() {
            bail!("Unable to open Stream: {:?}", stream_result.err());
        }

        let stream = stream_result.unwrap();

        // Start the output stream.
        if let Err(error) = stream.play() {
            bail!("Unable to begin playback: {}", error);
        }

        Ok(Box::new(CpalAudioOutputImpl {
            ring_buffer: ring_buf,
            ring_buf_producer,
            _stream: stream,
        }))
    }
}

impl AudioOutput for CpalAudioOutputImpl {
    fn write(&mut self, decoded: &[f32]) -> Result<()> {
        // Do nothing if there are no audio frames.
        if decoded.is_empty() {
            return Ok(());
        }

        let mut position = 0;
        while let Some(written) = self
            .ring_buf_producer
            .write_blocking(decoded.split_at(position).1)
        {
            position += written;
        }

        Ok(())
    }

    fn flush(&mut self) {
        // We need to make sure all audio data has been read and played back prior to dropping the
        // stream, so we'll block here until the buffer has been emptied.
        while !self.ring_buffer.is_empty() {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
