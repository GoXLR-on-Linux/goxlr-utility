use anyhow::{anyhow, Result};

use core::default::Default;
use log::{debug, warn};
use std::fs::File;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::audio::get_output;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::errors::Error;
use symphonia::core::formats::{SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::{Hint, ProbeResult};
use symphonia::default::get_codecs;

pub struct Player {
    probe: ProbeResult,

    volume: f32,
    stopping: Arc<AtomicBool>,
    force_stop: Arc<AtomicBool>,

    device: Option<String>,
    fade_duration: Option<f32>,
    start_pct: Option<f64>,
    stop_pct: Option<f64>,
    gain: Option<f32>,
}

impl Player {
    /// Load up the Player, and prepare for playback..
    pub fn new(file: &PathBuf, device: Option<String>) -> Result<Self> {
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

        if probe_result.is_err() {
            return Err(anyhow!("Unable to Probe audio file"));
        }

        Ok(Self {
            probe: probe_result.unwrap(),
            volume: 1.0_f32,
            stopping: Arc::new(AtomicBool::new(false)),
            force_stop: Arc::new(AtomicBool::new(false)),

            device,
            fade_duration: Some(1.0),
            start_pct: None,
            stop_pct: None,
            gain: None,
        })
    }

    pub fn play(&mut self) -> Result<(), Error> {
        println!("{}", self.volume);
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

        if let Some(rate) = sample_rate {
            let channels = match track.codec_params.channels {
                None => 2, // Assume 2 playback Channels..
                Some(channels) => channels.count(),
            };

            if let Some(fade_duration) = self.fade_duration {
                // Calculate the Change in Volume per sample..
                fade_amount = Some(1.0 / (rate as f32 * fade_duration) / channels as f32);
            }

            if let Some(start_pct) = self.start_pct {
                if let Some(frames) = frames {
                    // Calculate the first frame based on the percent..
                    first_frame = Some(((frames as f64 / 100.0) * start_pct).round() as u64);
                    debug!("Starting Sample: {}", first_frame.unwrap());
                }
            }

            if let Some(stop_pct) = self.stop_pct {
                if let Some(frames) = frames {
                    stop_sample =
                        Some(((frames as f64 / 100.0) * stop_pct).round() as u64 * channels as u64);
                    debug!("Stop Sample: {}", stop_sample.unwrap());
                }
            }
        } else {
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
                Ok(seeked_to) => seeked_to.actual_ts,
                Err(_) => 0,
            }
        } else {
            0
        };

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
                    if audio_output.is_none() {
                        let spec = *decoded.spec();
                        let duration = decoded.capacity() as u64;

                        audio_output.replace(get_output(spec, self.device.clone()).unwrap());
                        sample_buffer = Some(SampleBuffer::<f32>::new(duration, spec));
                    }

                    if let Some(ref mut buf) = sample_buffer {
                        // Grab out the samples..
                        buf.copy_interleaved_ref(decoded.clone());

                        let mut break_playback = false;
                        let mut samples = buf.samples().to_vec();

                        // Apply any gain to the samples..
                        if let Some(gain) = self.gain {
                            for i in 0..samples.len() - 1 {
                                samples[i] *= gain;
                            }
                        }

                        if self.stopping.load(Ordering::Relaxed) {
                            if self.force_stop.load(Ordering::Relaxed) {
                                // Don't care about the buffer, just end it.
                                debug!("Force Stop Requested, terminating.");
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

        decoder.finalize();
        result
    }

    pub fn get_state(&self) -> PlayerState {
        PlayerState {
            stopping: self.stopping.clone(),
            force_stop: self.force_stop.clone(),
        }
    }
}

pub struct PlayerState {
    pub stopping: Arc<AtomicBool>,
    pub force_stop: Arc<AtomicBool>,
}
