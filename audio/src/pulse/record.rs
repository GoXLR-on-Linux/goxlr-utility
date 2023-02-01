use crate::audio::AudioInput;
use anyhow::{anyhow, Result};
use libpulse_binding::def::BufferAttr;
use libpulse_binding::sample::{Format, Spec};
use libpulse_binding::stream::Direction;
use libpulse_simple_binding::Simple;

pub struct PulseAudioInput {
    pulse_simple: Simple,
    buffer: [u8; 1024],
}

impl PulseAudioInput {
    pub fn open(device: Option<String>) -> Result<Box<dyn AudioInput>> {
        // We know the spec of the input stream..
        let pulse_spec = Spec {
            format: Format::F32le,
            channels: 2,
            rate: 48000,
        };

        if !pulse_spec.is_valid() {
            // Invalid Specification, Error Out..
            return Err(anyhow!("Invalid Pulse Specification"));
        }

        let device_string;
        let device_str = match device {
            None => None,
            Some(value) => {
                device_string = value;
                Some(device_string.as_str())
            }
        };

        // Super small buffer to prevent latency..
        let pulse_buffer_attributes = BufferAttr {
            maxlength: 1024,
            tlength: u32::MAX,
            prebuf: 0,
            minreq: u32::MAX,
            fragsize: 0,
        };

        // Create the Connection (Use Pulse Simple for this, because, well, it's simple!)
        let pulse = Simple::new(
            None,
            "GoXLR Utility",
            Direction::Record,
            device_str,
            "Media",
            &pulse_spec,
            Default::default(),
            Some(&pulse_buffer_attributes),
        );

        match pulse {
            Ok(pulse_simple) => Ok(Box::new(PulseAudioInput {
                buffer: [0; 1024],
                pulse_simple,
            })),
            Err(_) => Err(anyhow!("Unable to Connect to Pulse")),
        }
    }
}

impl AudioInput for PulseAudioInput {
    fn read(&mut self) -> Result<Vec<f32>> {
        self.pulse_simple.read(&mut self.buffer)?;

        // Convert the buffer into f32 samples..
        let mut samples = vec![];
        for chunk in self.buffer.chunks(4) {
            samples.push(f32::from_le_bytes(<[u8; 4]>::try_from(chunk)?));
        }

        // Throw back the samples.
        Ok(samples)
    }

    fn flush(&mut self) {
        let _ = self.pulse_simple.flush();
    }
}
