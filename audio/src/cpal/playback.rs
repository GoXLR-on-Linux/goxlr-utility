use anyhow::anyhow;
use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rb::*;
use symphonia::core::audio::SignalSpec;

use crate::audio::AudioOutput;
use log::{debug, error};

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
    ring_buf_producer: Producer<f32>,
    stream: cpal::Stream,
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

        // Create a ring buffer with a capacity for up-to 200ms of audio.
        let ring_len = ((200 * spec.rate as usize) / 1000) * num_channels;
        let ring_buf = SpscRb::<f32>::new(ring_len);
        let (ring_buf_producer, ring_buf_consumer) = (ring_buf.producer(), ring_buf.consumer());

        let stream_result = device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                // Write out as many samples as possible from the ring buffer to the audio output.
                let written = ring_buf_consumer.read(data).unwrap_or(0);

                // Mute any remaining samples.
                data[written..].iter_mut().for_each(|s| *s = 0.0);
            },
            move |_| error!("Audio Output Error.."),
        );

        if stream_result.is_err() {
            return Err(anyhow!("Unable to open Stream"));
        }

        let stream = stream_result.unwrap();

        // Start the output stream.
        if stream.play().is_err() {
            return Err(anyhow!("Unable to begin Playback"));
        }

        Ok(Box::new(CpalAudioOutputImpl {
            ring_buf_producer,
            stream,
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
        // Flush is best-effort, ignore the returned result.
        let _ = self.stream.pause();
    }
}
