use anyhow::{anyhow, bail, Result};
use enum_map::EnumMap;
use goxlr_audio::get_audio_inputs;
use goxlr_audio::player::{Player, PlayerState};
use goxlr_audio::recorder::BufferedRecorder;
use goxlr_audio::recorder::RecorderState;
use goxlr_types::SampleBank;
use goxlr_types::SampleButtons;
use log::{debug, error, warn};
use regex::Regex;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use strum::IntoEnumIterator;

#[derive(Debug)]
pub struct AudioHandler {
    output_device: Option<String>,

    buffered_input: Option<Arc<BufferedRecorder>>,

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
    file: PathBuf,
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
    pub fn new(recorder_buffer: u16) -> Result<Self> {
        // Find the Input Device..
        let mut handler = Self {
            output_device: None,

            buffered_input: None,

            last_device_check: None,
            active_streams: EnumMap::default(),
        };

        // Immediately initialise the recorder, and let it try to handle stuff.
        let recorder = BufferedRecorder::new(
            handler.get_input_device_string_patterns(),
            recorder_buffer as usize,
        )?;
        let arc_recorder = Arc::new(recorder);
        let inner_recorder = arc_recorder.clone();
        handler.buffered_input.replace(arc_recorder);

        // Fire off the new thread to listen to audio..
        thread::spawn(move || inner_recorder.listen());

        Ok(handler)
    }

    fn get_output_device_patterns(&self) -> Vec<Regex> {
        let patterns = vec![
            Regex::new("goxlr_sample").expect("Invalid Regex in Audio Handler"),
            Regex::new("GoXLR_0_8_9").expect("Invalid Regex in Audio Handler"),
            Regex::new("GoXLR.*HiFi__Line3__sink").expect("Invalid Regex in Audio Handler"),
            Regex::new("CoreAudio\\*Sample").expect("Invalid Regex in Audio Handler"),
            Regex::new("WASAPI\\*Sample.*").expect("Invalid Regex in Audio Handler"),
        ];
        patterns
    }

    #[allow(dead_code)]
    fn get_output_device_string_patterns(&self) -> Vec<String> {
        let patterns = vec![
            String::from("goxlr_sample"),
            String::from("GoXLR_0_8_9"),
            String::from("GoXLR.*HiFi__Line3__sink"),
            String::from("CoreAudio\\*Sample"),
            String::from("WASAPI\\*Sample.*"),
        ];
        patterns
    }

    fn get_input_device_patterns(&self) -> Vec<Regex> {
        let patterns = vec![
            Regex::new("goxlr_sampler.*source").expect("Invalid Regex in Audio Handler"),
            Regex::new("GoXLR_0_4_5.*source").expect("Invalid Regex in Audio Handler"),
            Regex::new("GoXLR.*HiFi__Line5__source").expect("Invalid Regex in Audio Handler"),
            Regex::new("CoreAudio\\*Sampler").expect("Invalid Regex in Audio Handler"),
            Regex::new("WASAPI\\*Sample.*").expect("Invalid Regex in Audio Handler"),
        ];
        patterns
    }

    fn get_input_device_string_patterns(&self) -> Vec<String> {
        let patterns = vec![
            String::from("goxlr_sampler.*source"),
            String::from("GoXLR_0_4_5.*source"),
            String::from("GoXLR.*HiFi__Line5__source"),
            String::from("CoreAudio\\*Sampler"),
            String::from("WASAPI\\*Sample.*"),
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
            false => get_audio_inputs(),
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
        if let Some(stream) = &self.active_streams[bank][button] {
            if stream.playback.is_some() {
                return true;
            }
        }
        false
    }

    pub fn is_sample_recording(&self, bank: SampleBank, button: SampleButtons) -> bool {
        if let Some(stream) = &self.active_streams[bank][button] {
            if stream.recording.is_some() {
                return true;
            }
        }
        false
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
                    let result = player.play();
                    if let Err(error) = result {
                        warn!("Playback Error: {}", error);
                    }
                } else {
                    let result = player.play_loop();
                    if let Err(error) = result {
                        warn!("Loop Playback Error: {}", error);
                    }
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

    pub async fn stop_playback(
        &mut self,
        bank: SampleBank,
        button: SampleButtons,
        force: bool,
    ) -> Result<()> {
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

                // TODO: Tidy this!
                if force {
                    playback_state
                        .state
                        .force_stop
                        .store(true, Ordering::Relaxed);
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
        if let Some(recorder) = &mut self.buffered_input {
            if !recorder.is_ready() {
                warn!("Sampler not ready, possibly missing Sample device. Not recording.");

                debug!("Available Audio Inputs: ");
                get_audio_inputs()
                    .iter()
                    .for_each(|name| debug!("{}", name));

                bail!("Sampler is not ready to handle recording (possibly missing device?)");
            }

            let state = RecorderState {
                stop: Arc::new(AtomicBool::new(false)),
            };

            let inner_recorder = recorder.clone();
            let inner_path = path.clone();
            let inner_state = state.clone();

            let handler = thread::spawn(move || {
                let result = inner_recorder.record(&inner_path, inner_state);
                if result.is_err() {
                    error!("Error: {}", result.err().unwrap());
                }
            });

            self.active_streams[bank][button] = Some(StateManager {
                stream_type: StreamType::Recording,
                recording: Some(AudioRecordingState {
                    file: path,
                    handle: Some(handler),
                    state,
                }),
                playback: None,
            });
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
                bail!("Attempted to Stop Recording on Playback Stream..");
            }

            if let Some(recording_state) = &mut player.recording {
                recording_state.state.stop.store(true, Ordering::Relaxed);
                recording_state.wait();

                // Recording Complete, check the file was made...
                if recording_state.file.exists() {
                    if let Some(file_name) = recording_state.file.file_name() {
                        return Ok(Some(String::from(file_name.to_string_lossy())));
                    } else {
                        bail!("Unable to Extract Filename from Path! (This shouldn't be possible!)")
                    }
                }

                // If we get here, the file was never made.
                return Ok(None);
            }
        }
        bail!("Attempted to stop inactive recording..");
    }

    pub fn calculate_gain(&self, path: &PathBuf) -> Result<Option<f64>> {
        let mut player = Player::new(path, None, None, None, None, None)?;
        player.calculate_gain()
    }
}
