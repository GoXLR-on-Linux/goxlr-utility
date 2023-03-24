use crate::audio::{AudioOutput, AudioSpecification};
use anyhow::{anyhow, Result};
use libpulse_binding::def::BufferAttr;
use libpulse_binding::sample::{Format, Spec};
use libpulse_binding::stream::Direction;
use libpulse_simple_binding::Simple;
use std::thread;
use std::time::Duration;

pub struct PulsePlayback {
    pulse_simple: Simple,
}

impl PulsePlayback {
    pub fn open(spec: AudioSpecification) -> Result<Box<dyn AudioOutput>> {
        let pulse_spec = Spec {
            format: Format::FLOAT32NE,
            channels: spec.spec.channels.count() as u8,
            rate: spec.spec.rate,
        };

        if !pulse_spec.is_valid() {
            // Invalid Specification, Error Out..
            return Err(anyhow!("Invalid Pulse Specification"));
        }

        let device_string;
        let device_str = match spec.device {
            None => None,
            Some(value) => {
                device_string = value;
                Some(device_string.as_str())
            }
        };

        // We need to maintain a relatively small buffer..
        let pulse_buffer_attributes = BufferAttr {
            maxlength: u32::MAX,
            tlength: 1024,
            prebuf: u32::MAX,
            minreq: u32::MAX,
            fragsize: u32::MAX,
        };

        // Create the Connection (Use Pulse Simple for this, because, well, it's simple!)
        let pulse = Simple::new(
            None,
            "GoXLR Utility",
            Direction::Playback,
            device_str,
            "Media",
            &pulse_spec,
            Default::default(),
            Some(&pulse_buffer_attributes),
        );

        match pulse {
            Ok(pulse_simple) => {
                thread::sleep(Duration::from_millis(75));
                Ok(Box::new(PulsePlayback { pulse_simple }))
            }
            Err(_) => Err(anyhow!("Unable to Connect to Pulse")),
        }
    }
}

impl AudioOutput for PulsePlayback {
    fn write(&mut self, samples: &[f32]) -> Result<()> {
        let mut buffer = vec![];

        for sample in samples {
            buffer.extend_from_slice(&sample.to_le_bytes());
        }

        if self.pulse_simple.write(buffer.as_slice()).is_ok() {}
        Ok(())
    }

    fn flush(&mut self) {
        let _ = self.pulse_simple.drain();
    }

    fn stop(&mut self) {}
}
