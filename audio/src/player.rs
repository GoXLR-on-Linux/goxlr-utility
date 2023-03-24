use anyhow::{anyhow, bail, Result};

use core::default::Default;
use ebur128::{EbuR128, Mode};
use log::{debug, warn};
use std::fs::File;
use std::io::ErrorKind::UnexpectedEof;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::audio::{get_output, AudioSpecification};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::errors::Error;
use symphonia::core::formats::{SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::{Hint, ProbeResult};
use symphonia::default::get_codecs;

pub struct Player {
    file: PathBuf,
    probe: ProbeResult,

    volume: f32,
    stopping: Arc<AtomicBool>,
    force_stop: Arc<AtomicBool>,

    device: Option<String>,
    fade_duration: Option<f32>,
    start_pct: Option<f64>,
    stop_pct: Option<f64>,
    gain: Option<f64>,

    // Used for processing Gain..
    process_only: bool,
    normalized_gain: Option<f64>,
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
            return Err(anyhow!("Unable to Probe audio file"));
        }

        Ok(Self {
            file: file.clone(),

            probe: probe_result.unwrap(),
            volume: 1.0_f32,
            stopping: Arc::new(AtomicBool::new(false)),
            force_stop: Arc::new(AtomicBool::new(false)),

            device,
            fade_duration,
            start_pct,
            stop_pct,
            gain,

            process_only: false,
            normalized_gain: None,
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

        let probe_result = symphonia::default::get_probe().format(
            &hint,
            stream,
            &format_options,
            &metadata_options,
        );
        probe_result
    }

    pub fn calculate_gain(&mut self) -> Result<Option<f64>> {
        self.process_only = true;
        self.play()?;

        Ok(self.normalized_gain)
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
            None => return Ok(()),
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
            None => {
                debug!("Unable to ascertain channel count, assuming 2..");
                2
            } // Assume 2 playback Channels..
            Some(channels) => channels.count(),
        };

        if let Some(rate) = sample_rate {
            if self.process_only {
                ebu_r128 = Some(EbuR128::new(channels as u32, rate, Mode::I)?);
            } else {
                if let Some(fade_duration) = self.fade_duration {
                    // Calculate the Change in Volume per sample..
                    fade_amount = Some(1.0 / (rate as f32 * fade_duration) / channels as f32);
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
            if self.process_only {
                bail!("Unable to obtain Rate, cannot normalise.");
            }
            warn!("Unable to get the Sample Rate, Fade and Seek Unavailable");
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

        let mut break_playback = false;

        // Loop over the input file..
        let result = loop {
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
                        let capacity = decoded.capacity() as u64;

                        sample_buffer = Some(SampleBuffer::<f32>::new(capacity, spec));

                        if !self.process_only {
                            let audio_spec = AudioSpecification {
                                device: self.device.clone(),
                                spec,
                                buffer: capacity as usize,
                            };

                            audio_output.replace(get_output(audio_spec)?);
                        }
                    }

                    if let Some(ref mut buf) = sample_buffer {
                        // Grab out the samples..
                        buf.copy_interleaved_ref(decoded.clone());
                        let mut samples = buf.samples().to_vec();

                        if let Some(ref mut ebu_r128) = ebu_r128 {
                            ebu_r128.add_frames_f32(samples.as_slice())?;

                            // Skip straight to the next packet..
                            continue;
                        }

                        // Apply any gain to the samples..
                        if let Some(gain) = self.gain {
                            // Clippy doesn't seem to understand that I'm actually changing the
                            // values here, so we'll ignore this warning.
                            #[allow(clippy::needless_range_loop)]
                            for i in 0..samples.len() {
                                samples[i] *= gain as f32;
                            }
                        }

                        if self.stopping.load(Ordering::Relaxed) {
                            if self.force_stop.load(Ordering::Relaxed) {
                                // Don't care about the buffer, just end it.
                                debug!("Force Stop Requested, terminating.");

                                if let Some(audio_output) = &mut audio_output {
                                    audio_output.stop();
                                }

                                break Ok(());
                            }

                            if let Some(fade_amount) = fade_amount {
                                // Technically, this is a little weird, we don't do a 'per-channel' check on the samples,
                                // so each channel will have a slightly different volume, for now it's small enough to not
                                // actually notice :p

                                for i in 0..=samples.len() - 1 {
                                    samples[i] *= self.volume;
                                    self.volume -= fade_amount;
                                    if self.volume < 0.0 {
                                        // We've reached the end, ensure already processed samples  make it
                                        samples = samples[0..i].to_vec();
                                        break_playback = true;
                                        break;
                                    }
                                }
                            } else {
                                // No fade duration, clear out sample buffer and end.
                                debug!("Stop Requested, No Fade Out set, Stopping Playback.");
                                samples = vec![];
                                break_playback = true;
                            }
                        }

                        // Flush the samples to the Audio Stream..
                        if let Some(audio_output) = &mut audio_output {
                            audio_output.write(&samples).unwrap()
                        }

                        samples_processed += samples.len() as u64;

                        if let Some(stop_sample) = stop_sample {
                            if samples_processed >= stop_sample {
                                break Ok(());
                            }
                        }

                        // If we've been instructed to break, end it here.
                        if break_playback {
                            break Ok(());
                        }
                    }
                }
                Err(err) => break Err(err),
            }
        };
        if !self.force_stop.load(Ordering::Relaxed) {
            if let Some(mut audio_output) = audio_output {
                // We should always flush the last samples, unless forced to stop
                audio_output.flush();
            }
        }

        if let Some(ebu_r128) = ebu_r128 {
            // Calculate Gain..
            let loudness = ebu_r128.loudness_global()?;
            let target = -23.0;

            let gain_db = target - loudness;
            self.normalized_gain = Some(f64::powf(10., gain_db / 20.));
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

    pub fn get_state(&self) -> PlayerState {
        PlayerState {
            stopping: self.stopping.clone(),
            force_stop: self.force_stop.clone(),
        }
    }
}

#[derive(Debug)]
pub struct PlayerState {
    pub stopping: Arc<AtomicBool>,
    pub force_stop: Arc<AtomicBool>,
}
