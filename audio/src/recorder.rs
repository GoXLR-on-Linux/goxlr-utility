use crate::audio::get_input;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct Recorder {
    file: PathBuf,
    device: Option<String>,
    stop: Arc<AtomicBool>,
}

impl Recorder {
    pub fn new(file: &Path, device: Option<String>) -> Result<Self> {
        Ok(Self {
            file: file.to_path_buf(),
            device,
            stop: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn record(&mut self) -> Result<()> {
        // Prep the file writer..
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(&self.file, spec)?;

        // Grab the Audio Reader..
        let mut input = get_input(self.device.clone())?;

        // Being the Read Loop..
        while !self.stop.load(Ordering::Relaxed) {
            if let Ok(samples) = input.read() {
                for sample in samples {
                    writer.write_sample(sample)?;
                }
            }
        }

        // Flush and Finalise the WAV file..
        writer.flush()?;
        writer.finalize()?;

        Ok(())
    }

    pub fn get_state(&self) -> RecorderState {
        RecorderState {
            stop: self.stop.clone(),
        }
    }
}

#[derive(Debug)]
pub struct RecorderState {
    pub stop: Arc<AtomicBool>,
}
