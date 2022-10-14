use anyhow::{anyhow, Result};
use enum_map::EnumMap;
use goxlr_audio::player::{Player, PlayerState};
use goxlr_types::SampleBank;
use goxlr_types::SampleButtons;
use log::debug;
use regex::Regex;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use strum::IntoEnumIterator;

#[derive(Debug)]
pub struct AudioHandler {
    output_device: Option<String>,
    _input_device: Option<String>,

    last_device_check: Option<Instant>,
    active_streams: EnumMap<SampleBank, EnumMap<SampleButtons, Option<GoXLRPlayer>>>,
}

pub struct AudioFile {
    pub(crate) file: PathBuf,
    pub(crate) gain: Option<f64>,
    pub(crate) start_pct: Option<f64>,
    pub(crate) stop_pct: Option<f64>,
    pub(crate) fade_on_stop: bool,
}

#[derive(Debug)]
struct GoXLRPlayer {
    handle: Option<JoinHandle<()>>,
    state: PlayerState,
}

impl GoXLRPlayer {
    pub fn wait(&mut self) {
        let _ = self.handle.take().map(JoinHandle::join);
    }

    pub fn is_finished(&self) -> bool {
        if let Some(handle) = &self.handle {
            return handle.is_finished();
        }
        true
    }
}

impl AudioHandler {
    pub fn new() -> Result<Self> {
        let handler = Self {
            output_device: None,
            _input_device: None,

            last_device_check: None,
            active_streams: EnumMap::default(),
        };
        Ok(handler)
    }

    fn get_output_device_patterns(&self) -> Vec<Regex> {
        let patterns = vec![
            Regex::new("goxlr_sample").expect("Invalid Regex in Audio Handler"),
            Regex::new("GoXLR_0_8_9").expect("Invalid Regex in Audio Handler"),
            Regex::new("CoreAudio\\*Sample").expect("Invalid Regex in Audio Handler"),
        ];
        patterns
    }

    fn get_input_device_patterns(&self) -> Vec<Regex> {
        let patterns = vec![
            Regex::new("goxlr_sampler.*source").expect("Invalid Regex in Audio Handler"),
            Regex::new("GoXLR_0_4_5.*source").expect("Invalid Regex in Audio Handler"),
        ];
        patterns
    }

    fn find_device(&mut self, is_output: bool) {
        debug!("Attempting to Find Device..");
        if let Some(last_check) = self.last_device_check {
            if last_check + Duration::from_secs(5) > Instant::now() {
                return;
            }
        }

        let device_list = match is_output {
            true => goxlr_audio::get_audio_outputs(),
            false => goxlr_audio::get_audio_inputs(),
        };

        let pattern_matchers = match is_output {
            true => self.get_output_device_patterns(),
            false => self.get_input_device_patterns(),
        };

        let device = device_list
            .iter()
            .find(|output| {
                pattern_matchers
                    .iter()
                    .any(|pattern| pattern.is_match(output))
            })
            .cloned();

        if let Some(device) = &device {
            debug!("Found Device: {}", device);
        } else {
            debug!("Audio Device Not Found, Available Devices:");
            device_list.iter().for_each(|name| debug!("{}", name));
        }

        if is_output {
            self.output_device = device;
        } else {
            self._input_device = device;
        }
    }

    pub async fn check_playing(&mut self) {
        // Iterate over the Sampler Banks..
        for bank in SampleBank::iter() {
            // Iterate over the buttons..
            for button in SampleButtons::iter() {
                if let Some(current) = &self.active_streams[bank][button] {
                    if current.is_finished() {
                        self.active_streams[bank][button] = None;
                    }
                }
            }
        }
    }

    pub fn is_sample_playing(&self, bank: SampleBank, button: SampleButtons) -> bool {
        self.active_streams[bank][button].is_some()
    }

    pub fn is_sample_stopping(&self, bank: SampleBank, button: SampleButtons) -> bool {
        if let Some(player) = &self.active_streams[bank][button] {
            return player.state.stopping.load(Ordering::Relaxed);
        }

        false
    }

    pub async fn play_for_button(
        &mut self,
        bank: SampleBank,
        button: SampleButtons,
        audio: AudioFile,
    ) -> Result<()> {
        if self.output_device.is_none() {
            self.find_device(true);
        }

        if let Some(output_device) = &self.output_device {
            let fade_duration = match audio.fade_on_stop {
                true => Some(0.5),
                false => None,
            };

            // Ok, we need to grab and configure the player..
            let mut player = Player::new(
                &audio.file,
                Some(output_device.clone()),
                fade_duration,
                audio.start_pct,
                audio.stop_pct,
                audio.gain,
            )?;

            let state = player.get_state();
            let handler = thread::spawn(move || {
                let _ = player.play();
            });

            self.active_streams[bank][button] = Some(GoXLRPlayer {
                handle: Some(handler),
                state,
            });
        } else {
            return Err(anyhow!("Unable to play Sample, Output device not found"));
        }

        Ok(())
    }

    pub async fn stop_playback(&mut self, bank: SampleBank, button: SampleButtons) -> Result<()> {
        if let Some(player) = &mut self.active_streams[bank][button] {
            if player.state.stopping.load(Ordering::Relaxed) {
                // We should be stopping already, force the shutdown.
                debug!("Forcing Stop of Audio on {} {}..", bank, button);
                player.state.force_stop.store(true, Ordering::Relaxed);

                // We'll wait for this thread to complete before proceeding..
                player.wait();
                self.active_streams[bank][button] = None;
                return Ok(());
            }

            // We're not currently in a stopping state, trigger it.
            player.state.stopping.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn record_for_button(&mut self, _button: SampleButtons) -> Result<()> {
        Ok(())
    }
}
