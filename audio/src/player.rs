use anyhow::{anyhow, bail, Result};

use core::default::Default;
use ebur128::{EbuR128, Mode};
use log::debug;
use std::fs::File;
use std::io::ErrorKind::UnexpectedEof;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use crate::audio::{get_output, AudioSpecification};
use crate::AtomicF64;
use symphonia::core::audio::{Layout, SampleBuffer, SignalSpec};
use symphonia::core::errors::Error;
use symphonia::core::formats::{SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::{Hint, ProbeResult};
use symphonia::default::get_codecs;

pub struct Player {
    file: PathBuf,
    probe: ProbeResult,

    volume: f32,

    // I should really introduce messaging for this..
    stopping: Arc<AtomicBool>,
    force_stop: Arc<AtomicBool>,
    restart_track: Arc<AtomicBool>,

    device: Option<String>,
    fade_duration: Option<f32>,
    start_pct: Option<f64>,
    stop_pct: Option<f64>,
    gain: Option<f64>,

    progress: Arc<AtomicU8>,
    error: Arc<Mutex<Option<String>>>,

    // Used for processing Gain..
    process_only: bool,
    normalized_gain: Arc<AtomicF64>,
}

impl Player {
    /// Load up the Player, and prepare for playback..
    pub fn new(
        file: &PathBuf,
        device: Option<String>,
        fade_duration: Option<f32>,
        start_pct: Option<f64>,
        stop_pct: Option<f64>,
        gain: Option<f64>,
    ) -> Result<Self> {
        let probe_result = Player::load_file(file);
        if probe_result.is_err() {
            return Err(anyhow!("Unable to Probe Audio File"));
        }

        Ok(Self {
            file: file.clone(),

            probe: probe_result.unwrap(),
            volume: 1.0_f32,
            stopping: Arc::new(AtomicBool::new(false)),
            force_stop: Arc::new(AtomicBool::new(false)),
            restart_track: Arc::new(AtomicBool::new(false)),

            progress: Arc::new(AtomicU8::new(0)),
            error: Arc::new(Mutex::new(None)),

            device,
            fade_duration,
            start_pct,
            stop_pct,
            gain,

            process_only: false,
            normalized_gain: Arc::new(AtomicF64::new(1.0)),
        })
    }

    fn load_file(file: &PathBuf) -> symphonia::core::errors::Result<ProbeResult> {
        // Use the file extension to get a type hint..
        let mut hint = Hint::new();
        if let Some(extension) = file.extension() {
            if let Some(extension_str) = extension.to_str() {
                hint.with_extension(extension_str);
            }
        }

        let media_source = Box::new(File::open(file).unwrap());
        let stream = MediaSourceStream::new(media_source, Default::default());

        let format_options = Default::default();
        let metadata_options = Default::default();

        symphonia::default::get_probe().format(&hint, stream, &format_options, &metadata_options)
    }

    pub fn calculate_gain(&mut self) {
        self.process_only = true;

        let result = self.play();
        if let Err(error) = result {
            let mut res = self.error.lock().unwrap();
            *res = Some(error.to_string());
        }
    }

    pub fn play_loop(&mut self) -> Result<()> {
        while !self.stopping.load(Ordering::Relaxed) {
            // Play the Sample..
            self.play()?;

            // Reload the file for next play..
            let probe = Player::load_file(&self.file);
            if probe.is_err() {
                bail!(probe.err().unwrap());
            }
            self.probe = probe.unwrap();
        }
        Ok(())
    }

    pub fn play(&mut self) -> Result<()> {
        let reader = &mut self.probe.format;

        // Grab the Track and it's ID
        let track = match reader.default_track() {
            Some(track) => track,
            None => bail!("Unable to find Default Track"),
        };
        let track_id = track.id;

        // The per-sample volume change when fading.
        let mut fade_amount: Option<f32> = None;

        // Sample Start and Stop positions..
        let mut first_frame: Option<u64> = None;
        let mut stop_sample: Option<u64> = None;

        let sample_rate = track.codec_params.sample_rate;
        let frames = track.codec_params.n_frames;

        let mut ebu_r128 = None;

        let channels = match track.codec_params.channels {
            None => bail!("Unable to obtain channel count"),
            Some(channels) => channels.count(),
        };

        if channels > 2 {
            bail!("The Sample Player only Supports Mono and Stereo Samples");
        }

        if let Some(rate) = sample_rate {
            if self.process_only {
                ebu_r128 = Some(EbuR128::new(channels as u32, rate, Mode::I)?);
            } else {
                if let Some(fade_duration) = self.fade_duration {
                    // When fading out, we need work out the number of samples related to the fade
                    // duration, so (rate * duration) should give us the expected frame count, but
                    // we also need to multiply by the channel count to get the sample count
                    fade_amount = Some((rate as f32 * fade_duration) * channels as f32);
                }

                if let Some(frames) = frames {
                    if let Some(start_pct) = self.start_pct {
                        // Calculate the first frame based on the percent..
                        first_frame = Some(((frames as f64 / 100.0) * start_pct).round() as u64);
                        debug!(
                            "Starting Sample: {}",
                            first_frame.unwrap() * channels as u64
                        );
                    }

                    if let Some(stop_pct) = self.stop_pct {
                        stop_sample = Some(
                            ((frames as f64 / 100.0) * stop_pct).round() as u64 * channels as u64,
                        );
                        debug!("Stop Sample: {}", stop_sample.unwrap());
                    }
                }
            }
        } else {
            bail!("Unable to Determine the Audio File's Sample Rate");
        }

        // Audio Output Device..
        let mut audio_output = None;

        // Create a Decoder..
        let decoder_opts = Default::default();
        let mut decoder = get_codecs().make(&track.codec_params, &decoder_opts)?;

        // Prepare the Sample Buffer..
        let mut sample_buffer = None;

        // Start the Processed Sample Count..
        let mut samples_processed = if let Some(frame) = first_frame {
            let seek_time = SeekTo::TimeStamp {
                ts: frame,
                track_id,
            };

            match reader.seek(SeekMode::Accurate, seek_time) {
                Ok(seeked_to) => seeked_to.actual_ts * channels as u64,
                Err(_) => 0,
            }
        } else {
            0
        };

        let mut mono_playback = false;

        // Loop over the input file..
        let result = 'main: loop {
            let packet = match reader.next_packet() {
                Ok(packet) => packet,
                Err(err) => {
                    break Err(err);
                }
            };

            if packet.track_id() != track_id {
                // Doesn't belong to us.
                continue;
            }

            match decoder.decode(&packet) {
                Ok(decoded) => {
                    // Is this the first decoded packet?
                    if audio_output.is_none() && sample_buffer.is_none() {
                        let spec = *decoded.spec();
                        let mut output_spec = spec;

                        // This is a mono audio file, we need to replace the spec with a Stereo
                        // spec, then during processing, duplicate and interlace the samples.
                        if spec.channels.count() == 1 {
                            mono_playback = true;
                            output_spec = SignalSpec::new_with_layout(spec.rate, Layout::Stereo);
                        }

                        let capacity = decoded.capacity() as u64;
                        sample_buffer = Some(SampleBuffer::<f32>::new(capacity, spec));

                        if !self.process_only {
                            let audio_spec = AudioSpecification {
                                device: self.device.clone(),
                                spec: output_spec,
                                buffer: capacity as usize,
                            };

                            audio_output.replace(get_output(audio_spec)?);
                        }
                    }

                    if let Some(ref mut buf) = sample_buffer {
                        // Grab out the samples..
                        buf.copy_interleaved_ref(decoded);
                        let mut samples = buf.samples().to_vec();

                        if mono_playback {
                            let mut buffer = vec![];
                            samples.iter().for_each(|s| {
                                buffer.push(*s);
                                buffer.push(*s)
                            });
                            samples = buffer;
                        }

                        if let Some(ref mut ebu_r128) = ebu_r128 {
                            ebu_r128.add_frames_f32(samples.as_slice())?;
                            samples_processed += samples.len() as u64;

                            let progress = Player::processed(frames, samples_processed, channels);
                            if self.progress.load(Ordering::Relaxed) != progress {
                                self.progress.store(progress, Ordering::Relaxed);
                            }

                            // Skip straight to the next packet..
                            continue;
                        }

                        // Apply any gain to the samples..
                        if let Some(gain) = self.gain {
                            for sample in samples.iter_mut() {
                                *sample *= gain as f32;
                            }
                        }

                        if self.stopping.load(Ordering::Relaxed) {
                            if self.force_stop.load(Ordering::Relaxed) {
                                // Don't care about the buffer, just end it.
                                debug!("Force Stop Requested, terminating.");
                                break 'main Ok(());
                            }

                            if let Some(fade_amount) = fade_amount {
                                // Technically, this is a little weird, we don't do a 'per-channel' check on the samples,
                                // so each channel will have a slightly different volume, for now it's small enough to not
                                // actually notice :p

                                for sample in samples.iter_mut() {
                                    *sample *= self.volume;

                                    // Has the fade amount dropped below 0?
                                    self.volume -= fade_amount;
                                    if self.volume < 0.0 {
                                        break 'main Ok(());
                                    }
                                }
                            } else {
                                // No fade duration, clear out sample buffer and end.
                                debug!("Stop Requested, No Fade Out set, Stopping Playback.");
                                break 'main Ok(());
                            }
                        }

                        // Flush the samples to the Audio Stream..
                        if let Some(audio_output) = &mut audio_output {
                            audio_output.write(&samples).unwrap()
                        }

                        samples_processed += samples.len() as u64;

                        // Calculate the Current Processing Percent..
                        let progress = Player::processed(frames, samples_processed, channels);
                        if self.progress.load(Ordering::Relaxed) != progress {
                            self.progress.store(progress, Ordering::Relaxed);
                        }

                        if let Some(stop_sample) = stop_sample {
                            if samples_processed >= stop_sample {
                                break Ok(());
                            }
                        }

                        if self.restart_track.load(Ordering::Relaxed) {
                            // We've been prompted to restart the current track..
                            let start_frame = first_frame.unwrap_or_default();

                            let seek_time = SeekTo::TimeStamp {
                                ts: start_frame,
                                track_id,
                            };

                            // Seek the reader backwards to the start..
                            samples_processed = match reader.seek(SeekMode::Accurate, seek_time) {
                                Ok(seeked_to) => seeked_to.actual_ts * channels as u64,
                                Err(e) => {
                                    debug!("Error Seeking: {}", e);
                                    0
                                }
                            };

                            // Set the back to FALSE
                            self.restart_track.store(false, Ordering::Relaxed);
                        }
                    }
                }
                Err(err) => break Err(err),
            }
        };
        if !self.force_stop.load(Ordering::Relaxed) {
            if let Some(ref mut audio_output) = audio_output {
                // We should always flush the last samples, unless forced to stop
                audio_output.flush();
            }
        }

        // Stop the playback handler..
        if let Some(mut audio_output) = audio_output {
            audio_output.stop();
        }

        if let Some(ebu_r128) = ebu_r128 {
            // Calculate Gain..
            let mut loudness = ebu_r128.loudness_global()?;
            if loudness == f64::NEG_INFINITY {
                debug!("Unable to Obtain loudness in Mode I, trying M..");
                loudness = ebu_r128.loudness_momentary()?;
            }

            if loudness == f64::NEG_INFINITY {
                debug!("Unable to Obtain loudness in Mode M, Setting Default..");
                self.normalized_gain.store(1.0, Ordering::Relaxed);
            } else {
                let target = -23.0;
                let gain_db = target - loudness;
                let value = f64::powf(10., gain_db / 20.);

                self.normalized_gain.store(value, Ordering::Relaxed);
            }
        }
        decoder.finalize();

        // Symphonia's reader doesn't seem to have a way to check whether we've finished reading
        // bytes for the file, so will continue to try until it hits and EoF then drop an error.
        //
        // We're going to suppress that error here, with the assumption that if it's reached, the
        // file has ended, and playback is complete. It's not ideal, but we'll work with it.
        if let Err(error) = result {
            if let Error::IoError(ref error) = error {
                if error.kind() == UnexpectedEof {
                    return Ok(());
                }
            }

            // Otherwise, throw this error up.
            bail!(error)
        }
        Ok(())
    }

    fn processed(total_frames: Option<u64>, current_frame: u64, channels: usize) -> u8 {
        // Calculate the Current Processing Percent..
        if let Some(frames) = total_frames {
            let frames_processed = current_frame / channels as u64;
            let processed_ratio = frames_processed as f64 / frames as f64;
            let calc_percent = (processed_ratio * 100.) as u8;

            return calc_percent;
        }
        0
    }

    pub fn get_state(&self) -> PlayerState {
        PlayerState {
            playing_file: self.file.clone(),
            stopping: self.stopping.clone(),
            force_stop: self.force_stop.clone(),
            restart_track: self.restart_track.clone(),
            progress: self.progress.clone(),
            error: self.error.clone(),
            calculated_gain: self.normalized_gain.clone(),
        }
    }
}

#[derive(Debug)]
pub struct PlayerState {
    // Note the file being played..
    pub playing_file: PathBuf,

    pub stopping: Arc<AtomicBool>,
    pub force_stop: Arc<AtomicBool>,

    // This is used for triggering a seek back the the beginning
    pub restart_track: Arc<AtomicBool>,

    // These are generally read only from the outside..
    pub progress: Arc<AtomicU8>,
    pub error: Arc<Mutex<Option<String>>>,

    // Specifically for calculating the gain..
    pub calculated_gain: Arc<AtomicF64>,
}
