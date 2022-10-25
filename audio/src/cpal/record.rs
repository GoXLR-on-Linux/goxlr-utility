use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use log::{debug, error};
use rb::{Consumer, RbConsumer, RbProducer, SpscRb, RB};

use crate::audio::AudioInput;
pub struct CpalAudioInput;

impl CpalAudioInput {
    pub fn open(device: Option<String>) -> Result<Box<dyn AudioInput>> {
        // Device handling is a little more awkward on CPAL, we need the audio host and device.
        // We request them as 'HOST*DEVICE' on the Command Line, now we can look them up :p

        let mut cpal_device = None;

        // We need to abstract this device finding code :p

        // Basically, if *ANYTHING* goes wrong here, we'll fall through to default.
        if let Some(device_name) = device {
            if let Some(position) = device_name.find('*') {
                let str_host = &device_name[0..position];
                let str_device = &device_name[position + 1..device_name.len()];

                // Ok, now for cpal, find the correct host..
                let cpal_host_list = cpal::available_hosts();
                let host_id = cpal_host_list.iter().find(|x| x.name() == str_host);

                if let Some(host_id) = host_id {
                    if let Ok(host) = cpal::host_from_id(*host_id) {
                        // We have found our host, now try to find the device..
                        if let Ok(mut devices) = host.output_devices() {
                            if let Some(device) = devices.find(|x| {
                                x.name().unwrap_or_else(|_| "UNKNOWN".to_string()) == str_device
                            }) {
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
            let host = cpal::default_host();
            final_device = match host.default_input_device() {
                Some(device) => device,
                None => return Err(anyhow!("Unable to find Default Device")),
            };
        }

        // Select proper recording routine based on sample format.
        CpalAudioInputImpl::try_open(&final_device)
    }
}

struct CpalAudioInputImpl {
    ring_buf_consumer: Consumer<f32>,
    stream: cpal::Stream,

    // Use a shared read buffer..
    read_buffer: [f32; 1024],
}

impl CpalAudioInputImpl {
    pub fn try_open(device: &cpal::Device) -> Result<Box<dyn AudioInput>> {
        // Input
        let config = cpal::StreamConfig {
            channels: 2 as cpal::ChannelCount,
            sample_rate: cpal::SampleRate(48000),
            buffer_size: cpal::BufferSize::Fixed(1024),
        };

        // Prepare the Read Buffer, grab the producer and consumer..
        let ring_buf = SpscRb::<f32>::new(4096);
        let (ring_buf_producer, ring_buf_consumer) = (ring_buf.producer(), ring_buf.consumer());

        let stream_result = device.build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // Write out as many samples as possible from the ring buffer to the audio output.
                if let Err(samples) = ring_buf_producer.write(data) {
                    debug!("{}", samples);
                }
            },
            move |_| error!("Audio Input Error.."),
        );

        if stream_result.is_err() {
            return Err(anyhow!("Unable to open Stream"));
        }

        let stream = stream_result.unwrap();

        // Start the input stream.
        if stream.play().is_err() {
            return Err(anyhow!("Unable to begin Playback"));
        }

        Ok(Box::new(CpalAudioInputImpl {
            read_buffer: [0.0; 1024],
            ring_buf_consumer,
            stream,
        }))
    }
}

impl AudioInput for CpalAudioInputImpl {
    fn read(&mut self) -> Result<Vec<f32>> {
        // Do a blocking read on the samples,
        if let Some(samples) = self.ring_buf_consumer.read_blocking(&mut self.read_buffer) {
            return Ok(Vec::from(&self.read_buffer[0..samples]));
        }
        Ok(vec![])
    }

    fn flush(&mut self) {
        let _ = self.stream.pause();
    }
}
