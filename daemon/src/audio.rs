use crate::{OVERRIDE_SAMPLER_INPUT, OVERRIDE_SAMPLER_OUTPUT};
use anyhow::{anyhow, bail, Result};
use enum_map::EnumMap;
use fancy_regex::Regex;
use goxlr_audio::player::{Player, PlayerState};
use goxlr_audio::recorder::BufferedRecorder;
use goxlr_audio::recorder::RecorderState;
use goxlr_audio::{get_audio_inputs, AtomicF64};
use goxlr_types::SampleBank;
use goxlr_types::SampleButtons;
use log::{debug, error, info, warn};
use std::ops::Deref;
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

    process_task: Option<ProcessTask>,
}

pub struct AudioFile {
    pub(crate) file: PathBuf,
    pub(crate) name: String,
    pub(crate) gain: Option<f64>,
    pub(crate) start_pct: Option<f64>,
    pub(crate) stop_pct: Option<f64>,
    pub(crate) fade_on_stop: bool,
}

#[derive(Debug)]
pub struct ProcessTask {
    bank: SampleBank,
    button: SampleButtons,
    file: PathBuf,

    player: AudioPlaybackState,
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

            process_task: None,
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
        let override_output = OVERRIDE_SAMPLER_OUTPUT.lock().unwrap().deref().clone();
        if let Some(device) = override_output {
            return vec![Regex::new(&device).expect("Invalid Regex in Audio Handler")];
        }

        let patterns = vec![
            Regex::new("goxlr_sample").expect("Invalid Regex in Audio Handler"),
            Regex::new("GoXLR_0_8_9").expect("Invalid Regex in Audio Handler"),
            Regex::new("GoXLR.*HiFi__Line3__sink").expect("Invalid Regex in Audio Handler"),
            Regex::new("CoreAudio\\*Sample").expect("Invalid Regex in Audio Handler"),
            Regex::new("^WASAPI\\*Sample(?:(?!Mini).)*$").expect("Invalid Regex in Audio Handler"),
            // If we ever support the sampler on the Mini, this can be used as a fallback, so we defer
            // to any attached Full Sized device, but if one isn't present, we can use the mini.
            //Regex::new("^WASAPI\\*Sample.*$").expect("Invalid Regex in Audio Handler"),
        ];
        patterns
    }

    #[allow(dead_code)]
    fn get_output_device_string_patterns(&self) -> Vec<String> {
        let override_output = OVERRIDE_SAMPLER_OUTPUT.lock().unwrap().deref().clone();
        if let Some(device) = override_output {
            return vec![device];
        }

        let patterns = vec![
            String::from("goxlr_sample"),
            String::from("GoXLR_0_8_9"),
            String::from("GoXLR.*HiFi__Line3__sink"),
            String::from("CoreAudio\\*Sample"),
            String::from("^WASAPI\\*Sample(?:(?!Mini).)*$"),
            //String::from("^WASAPI\\*Sample.*$"),
        ];
        patterns
    }

    fn get_input_device_patterns(&self) -> Vec<Regex> {
        let override_input = OVERRIDE_SAMPLER_INPUT.lock().unwrap().deref().clone();
        if let Some(device) = override_input {
            return vec![Regex::new(&device).expect("Invalid Regex in Audio Handler")];
        }

        let patterns = vec![
            Regex::new("goxlr_sample.*source").expect("Invalid Regex in Audio Handler"),
            Regex::new("GoXLR_0_4_5.*source").expect("Invalid Regex in Audio Handler"),
            Regex::new("GoXLR.*HiFi__Line5__source").expect("Invalid Regex in Audio Handler"),
            Regex::new("CoreAudio\\*Sampler").expect("Invalid Regex in Audio Handler"),
            Regex::new("^WASAPI\\*Sample(?:(?!Mini).)*$").expect("Invalid Regex in Audio Handler"),
            //Regex::new("^WASAPI\\*Sample.*$").expect("Invalid Regex in Audio Handler"),
        ];
        patterns
    }

    fn get_input_device_string_patterns(&self) -> Vec<String> {
        let override_input = OVERRIDE_SAMPLER_INPUT.lock().unwrap().deref().clone();
        if let Some(device) = override_input {
            return vec![device];
        }

        let patterns = vec![
            String::from("goxlr_sample.*source"),
            String::from("GoXLR_0_4_5.*source"),
            String::from("GoXLR.*HiFi__Line5__source"),
            String::from("CoreAudio\\*Sampler"),
            String::from("^WASAPI\\*Sample(?:(?!Mini).)*$"),
            //String::from("^WASAPI\\*Sample.*$"),
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
                pattern_matchers.iter().any(|pattern| {
                    if let Ok(result) = pattern.is_match(output) {
                        return result;
                    }
                    false
                })
            })
            .cloned();

        if let Some(device) = &device {
            debug!("Found Device: {}", device);
        } else {
            warn!("Audio Device Not Found, Available Devices:");
            device_list.iter().for_each(|name| info!("{}", name));
        }

        if is_output {
            self.output_device = device;
        }
    }

    pub async fn check_playing(&mut self) -> bool {
        let mut state_changed = false;

        // Iterate over the Sampler Banks..
        for bank in SampleBank::iter() {
            // Iterate over the buttons..
            for button in SampleButtons::iter() {
                if let Some(state) = &self.active_streams[bank][button] {
                    if state.stream_type == StreamType::Recording {
                        if let Some(recording) = &state.recording {
                            if recording.is_finished() {
                                self.active_streams[bank][button] = None;
                                state_changed = true;
                            }
                        }
                    } else if let Some(playback) = &state.playback {
                        if playback.is_finished() {
                            self.active_streams[bank][button] = None;
                            state_changed = true;
                        }
                    }
                }
            }
        }

        state_changed
    }

    pub fn is_sample_playing(&self, bank: SampleBank, button: SampleButtons) -> bool {
        if let Some(stream) = &self.active_streams[bank][button] {
            if stream.playback.is_some() {
                return true;
            }
        }
        false
    }

    pub fn sample_recording(&self, bank: SampleBank, button: SampleButtons) -> bool {
        if let Some(stream) = &self.active_streams[bank][button] {
            if stream.recording.is_some() {
                return true;
            }
        }
        false
    }

    pub fn is_sample_recording(&self) -> bool {
        for bank in SampleBank::iter() {
            for button in SampleButtons::iter() {
                if let Some(manager) = &self.active_streams[bank][button] {
                    if manager.recording.is_some() {
                        return true;
                    }
                }
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
        if let Some(recorder) = &self.buffered_input {
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
                gain: Arc::new(AtomicF64::new(1.)),
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
    ) -> Result<Option<(String, f64)>> {
        let mut file = None;

        if let Some(player) = &mut self.active_streams[bank][button] {
            if player.stream_type == StreamType::Playback {
                bail!("Attempted to Stop Recording on Playback Stream..");
            }

            if let Some(recording_state) = &mut player.recording {
                recording_state.state.stop.store(true, Ordering::Relaxed);
                recording_state.wait();

                debug!(
                    "Calculated Gain: {}",
                    recording_state.state.gain.load(Ordering::Relaxed)
                );

                // Recording Complete, check the file was made...
                if recording_state.file.exists() {
                    if let Some(file_name) = recording_state.file.file_name() {
                        let gain = recording_state.state.gain.load(Ordering::Relaxed);
                        file.replace((String::from(file_name.to_string_lossy()), gain));
                    } else {
                        bail!("Unable to Extract Filename from Path! (This shouldn't be possible!)")
                    }
                }
            }
        } else {
            bail!("Attempted to stop inactive recording..");
        }

        // Sample has been stopped, clear the state of this button.
        self.active_streams[bank][button] = None;
        Ok(file)
    }

    pub fn calculate_gain_thread(
        &mut self,
        path: PathBuf,
        bank: SampleBank,
        button: SampleButtons,
    ) -> Result<()> {
        if self.process_task.is_some() {
            bail!("Sample already being processed");
        }

        // Create the player..
        let mut player = Player::new(&path, None, None, None, None, None)?;

        // Grab the State..
        let state = player.get_state();

        // Spawn the Thread and Grab the Handler..
        let handler = thread::spawn(move || {
            player.calculate_gain();
        });

        // Store this into the processing task..
        self.process_task.replace(ProcessTask {
            bank,
            button,
            file: path,
            player: AudioPlaybackState {
                handle: Some(handler),
                state,
            },
        });

        Ok(())
    }

    pub fn is_calculating(&self) -> bool {
        self.process_task.is_some()
    }

    pub fn is_calculating_complete(&self) -> Result<bool> {
        if self.process_task.is_none() {
            bail!("Calculation not in progress");
        }

        if let Some(task) = &self.process_task {
            return Ok(task.player.is_finished());
        }
        bail!("Task exists, but also doesn't exist!");
    }

    pub fn get_calculating_progress(&self) -> Result<u8> {
        if self.process_task.is_none() {
            bail!("Calculation not in progress");
        }

        if let Some(task) = &self.process_task {
            return Ok(task.player.state.progress.load(Ordering::Relaxed));
        }

        bail!("Task exists, but also doesn't exist!");
    }

    pub fn get_and_clear_calculating_result(&mut self) -> Result<CalculationResult> {
        if self.process_task.is_none() {
            bail!("Calculation not in progress");
        }

        let result;
        if let Some(task) = &mut self.process_task {
            // We need to make sure the thread is finished..
            task.player.wait();

            let error = task.player.state.error.lock().unwrap();
            let task_result = if error.is_some() {
                Err(anyhow!(error.as_ref().unwrap().clone()))
            } else {
                Ok(())
            };

            result = CalculationResult {
                result: task_result,
                file: task.file.clone(),
                bank: task.bank,
                button: task.button,
                gain: task.player.state.calculated_gain.load(Ordering::Relaxed),
            };
        } else {
            bail!("Unable to obtain Task");
        }

        // In all cases, when we get here, we're done, so cleanup and go home
        self.process_task = None;
        Ok(result)
    }
}

impl Drop for AudioHandler {
    fn drop(&mut self) {
        if let Some(buffered_recorder) = &self.buffered_input {
            buffered_recorder.stop();
        }
    }
}

pub struct CalculationResult {
    pub result: Result<()>,
    pub file: PathBuf,
    pub bank: SampleBank,
    pub button: SampleButtons,
    pub gain: f64,
}
