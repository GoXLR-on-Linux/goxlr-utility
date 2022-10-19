use anyhow::{anyhow, bail, Result};
use enum_map::EnumMap;
use goxlr_audio::player::{Player, PlayerState};
use goxlr_audio::recorder::{Recorder, RecorderState};
use goxlr_types::SampleBank;
use goxlr_types::SampleButtons;
use log::debug;
use regex::Regex;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use strum::IntoEnumIterator;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct AudioHandler {
    output_device: Option<String>,
    _input_device: Option<String>,

    last_device_check: Option<Instant>,
    active_streams: EnumMap<SampleBank, EnumMap<SampleButtons, Option<StateManager>>>,
}

pub struct AudioFile {
    pub(crate) file: PathBuf,
    pub(crate) gain: Option<f64>,
    pub(crate) start_pct: Option<f64>,
    pub(crate) stop_pct: Option<f64>,
    pub(crate) fade_on_stop: bool,
}

#[derive(Debug)]
struct AudioPlaybackState {
    handle: Option<JoinHandle<()>>,
    state: PlayerState,
}

#[derive(Debug)]
struct AudioRecordingState {
    file_name: String,
    handle: Option<JoinHandle<()>>,
    state: RecorderState,
}

#[derive(Debug)]
struct StateManager {
    pub(crate) stream_type: StreamType,
    pub(crate) recording: Option<AudioRecordingState>,
    pub(crate) playback: Option<AudioPlaybackState>,
}

#[derive(Debug, PartialEq)]
enum StreamType {
    Playback,
    Recording,
}

// I could probably use a trait for this..
impl AudioPlaybackState {
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

impl AudioRecordingState {
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
                if let Some(state) = &self.active_streams[bank][button] {
                    if state.stream_type == StreamType::Recording {
                        if let Some(recording) = &state.recording {
                            if recording.is_finished() {
                                self.active_streams[bank][button] = None;
                            }
                        }
                    } else if let Some(playback) = &state.playback {
                        if playback.is_finished() {
                            self.active_streams[bank][button] = None;
                        }
                    }
                }
            }
        }
    }

    pub fn is_sample_playing(&self, bank: SampleBank, button: SampleButtons) -> bool {
        self.active_streams[bank][button].is_some()
    }

    pub fn is_sample_stopping(&self, bank: SampleBank, button: SampleButtons) -> bool {
        if let Some(state) = &self.active_streams[bank][button] {
            if state.stream_type == StreamType::Recording {
                return false;
            }

            if let Some(player) = &state.playback {
                return player.state.stopping.load(Ordering::Relaxed);
            }
        }

        false
    }

    pub async fn play_for_button(
        &mut self,
        bank: SampleBank,
        button: SampleButtons,
        audio: AudioFile,
        loop_track: bool,
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
                if !loop_track {
                    let _ = player.play();
                } else {
                    let _ = player.play_loop();
                }
            });

            self.active_streams[bank][button] = Some(StateManager {
                stream_type: StreamType::Playback,
                recording: None,
                playback: Some(AudioPlaybackState {
                    handle: Some(handler),
                    state,
                }),
            });
        } else {
            return Err(anyhow!("Unable to play Sample, Output device not found"));
        }

        Ok(())
    }

    pub async fn stop_playback(&mut self, bank: SampleBank, button: SampleButtons) -> Result<()> {
        if let Some(player) = &mut self.active_streams[bank][button] {
            if player.stream_type == StreamType::Recording {
                // TODO: We can proably use this..
                return Err(anyhow!("Attempted to Stop Playback on Recording Stream.."));
            }

            if let Some(playback_state) = &mut player.playback {
                if playback_state.state.stopping.load(Ordering::Relaxed) {
                    // We should be stopping already, force the shutdown.
                    debug!("Forcing Stop of Audio on {} {}..", bank, button);
                    playback_state
                        .state
                        .force_stop
                        .store(true, Ordering::Relaxed);

                    // We'll wait for this thread to complete before proceeding..
                    playback_state.wait();
                    self.active_streams[bank][button] = None;
                    return Ok(());
                }

                // We're not currently in a stopping state, trigger it.
                playback_state.state.stopping.store(true, Ordering::Relaxed);
            }
        }
        Ok(())
    }

    pub fn record_for_button(
        &mut self,
        path: PathBuf,
        bank: SampleBank,
        button: SampleButtons,
    ) -> Result<()> {
        if self._input_device.is_none() {
            self.find_device(false);
        }

        if let Some(device) = &self._input_device {
            let mut recorder = Recorder::new(&path, Some(device.clone()))?;
            let state = recorder.get_state();

            let handler = thread::spawn(move || {
                let _ = recorder.record();
            });

            if let Some(file_name) = path.file_name() {
                self.active_streams[bank][button] = Some(StateManager {
                    stream_type: StreamType::Recording,
                    recording: Some(AudioRecordingState {
                        file_name: String::from(file_name.to_string_lossy()),
                        handle: Some(handler),
                        state,
                    }),
                    playback: None,
                });
            } else {
                bail!("Unable to Extract Filename, something is wrong..");
            }
        } else {
            bail!("No valid Input Device was Found");
        }

        Ok(())
    }

    pub fn stop_record(
        &mut self,
        bank: SampleBank,
        button: SampleButtons,
    ) -> Result<Option<String>> {
        if let Some(player) = &mut self.active_streams[bank][button] {
            if player.stream_type == StreamType::Playback {
                return Err(anyhow!("Attempted to Stop Recording on Playback Stream.."));
            }

            if let Some(recording_state) = &mut player.recording {
                recording_state.state.stop.store(true, Ordering::Relaxed);
                recording_state.wait();

                return Ok(Some(recording_state.file_name.clone()));
            }
        }
        bail!("Attempted to stop inactive recording..");
    }

    pub async fn calculate_gain(&self, path: &PathBuf) -> Result<Option<f64>> {
        let mut player = Player::new(path, None, None, None, None, None)?;

        let gain_value: Arc<Mutex<Option<f64>>> = Arc::new(Mutex::new(None));
        let internal = gain_value.clone();
        tokio::spawn(async move {
            if let Ok(value) = player.calculate_gain() {
                // Grab our Mutex, and change the value..
                let mut mutex_value = internal.lock().await;
                *mutex_value = value;
            }
        })
        .await?;
        Ok(Arc::try_unwrap(gain_value).unwrap().into_inner())
    }
}
