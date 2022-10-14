use anyhow::{anyhow, Result};
use enum_map::EnumMap;
use goxlr_types::SampleBank;
use goxlr_types::SampleButtons;
use log::debug;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use strum::IntoEnumIterator;
use tokio::process::{Child, Command};

static output_patterns: Vec<&str> = ["a", "b"].to_vec();

#[derive(Debug)]
pub struct AudioHandler {
    output_device: Option<String>,
    _input_device: Option<String>,

    last_device_check: Option<Instant>,
    active_streams: EnumMap<SampleBank, EnumMap<SampleButtons, Option<Child>>>,
}

impl AudioHandler {
    pub fn new() -> Result<Self> {
        let mut handler = Self {
            output_device: None,
            _input_device: None,

            last_device_check: None,
            active_streams: EnumMap::default(),
        };

        Ok(handler)
    }

    fn find_device(&self, is_output: bool) -> Result<Option<String>> {
        debug!("Attempting to Find Device..");
        let mut input_device = None;

        if let Some(last_check) = self.last_device_check {
            if last_check + Duration::from_secs(5) > Instant::now() {
                return Ok(None);
            }
        }

        let outputs = goxlr_audio::get_audio_outputs();

        // let output = outputs.iter().find(|output| {
        //     output_patterns
        //         .iter()
        //         .find(|pattern| {
        //             output.matches(pattern);
        //             true
        //         })
        //         .is_some()
        // });

        Ok(input_device)
    }

    pub async fn check_playing(&mut self) {
        let map = &mut self.active_streams;

        // Iterate over the Sampler Banks..
        for bank in SampleBank::iter() {
            // Iterate over the buttons..
            for button in SampleButtons::iter() {
                if let Some(current) = &mut map[bank][button] {}
            }
        }
    }

    pub fn is_sample_playing(&self, bank: SampleBank, button: SampleButtons) -> bool {
        self.active_streams[bank][button].is_some()
    }

    pub async fn play_for_button(
        &mut self,
        bank: SampleBank,
        button: SampleButtons,
        file: PathBuf,
    ) -> Result<()> {
        if self.output_device.is_none() {
            self.find_device(true)?;
        }

        if let Some(output_device) = &self.output_device {
            self.active_streams[bank][button] = None;
        } else {
            return Err(anyhow!("Unable to play Sample, Output device not found"));
        }

        Ok(())
    }

    pub async fn stop_playback(&mut self, bank: SampleBank, button: SampleButtons) -> Result<()> {
        if let Some(child) = &mut self.active_streams[bank][button] {
            if let Some(pid) = child.id() {
                debug!("Killing child {}..", pid);
                Command::new("kill")
                    .args(["-TERM", pid.to_string().as_str()])
                    .output()
                    .await?;
            }
            self.active_streams[bank][button] = None;
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn record_for_button(&mut self, _button: SampleButtons) -> Result<()> {
        Ok(())
    }
}
