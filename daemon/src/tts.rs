use crate::settings::SettingsHandle;
use crate::shutdown::Shutdown;
use anyhow::Result;
use log::{debug, info, warn};
use std::time::Duration;
use tokio::sync::mpsc::Receiver;
use tokio::time;

#[cfg(feature = "tts")]
use tts::Tts;

#[allow(clippy::upper_case_acronyms)]
pub(crate) struct TTS {
    settings: SettingsHandle,
    tts: Option<Tts>,
}

impl TTS {
    pub fn new(settings: SettingsHandle) -> Result<TTS> {
        Ok(Self {
            settings,
            tts: None,
        })
    }

    pub async fn listen(&mut self, mut rx: Receiver<String>, mut shutdown: Shutdown) {
        let mut ticker = time::interval(Duration::from_secs(5));

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    //self.check_active().await;
                },
                () = shutdown.recv() => {
                    info!("Shutting down TTS Service");
                    return;
                },
                Some(message) = rx.recv() => {
                    debug!("Received TTS Message: {}", message);
                    self.speak_tts(message).await;
                },
            }
        }
    }

    // So this is problematic due to a bug in `windows::Media::Playback::MediaPlayer`. Dropping
    // a MediaPlayer instance does not correctly clean up left over resources, resulting in
    // huge numbers of MediaPlayers spawning if I try to drop them.
    #[allow(dead_code)]
    async fn check_active(&mut self) {
        if let Some(tts) = &self.tts {
            // If the follow get_tts_enabled code returns 'None', this code doesn't exist,
            // as they're both behind the same TTS feature flag!
            if let Some(enabled) = self.settings.get_tts_enabled().await {
                // We're no longer enabled, teardown the TTS handler..
                if !enabled {
                    self.tts.take();
                    return;
                }
            }

            if let Ok(speaking) = tts.is_speaking() {
                // If we're not currently speaking, teardown the TTS handler
                if !speaking {
                    self.tts.take();
                }
            }
        }
    }

    /*
    Ok, so this attempts to send a TTS message, but we shouldn't error out if it fails. Ultimately
    the GoXLR and the Utility will continue to function if this is erroring, and we shouldn't abort
    any normal behaviours because TTS didn't work.
     */
    pub async fn speak_tts(&mut self, message: String) {
        if self.settings.get_tts_enabled().await.is_none() {
            // TTS isn't available..
            return;
        }

        if !self.settings.get_tts_enabled().await.unwrap() {
            return;
        }

        if self.tts.is_none() {
            let tts = match Tts::default() {
                Ok(mut tts) => {
                    if cfg!(target_os = "macos") {
                        let _ = tts.set_rate(tts.max_rate());
                    }
                    tts
                }
                Err(e) => {
                    warn!("Unable to Spawn TTS instance: {:?}", e);
                    return;
                }
            };
            self.tts.replace(tts);
        }

        // This should, in 100% of cases, be true..
        if let Some(tts) = &mut self.tts {
            if let Err(e) = tts.stop() {
                warn!("Error Stopping TTS {:?}", e);
                return;
            }

            if let Err(e) = tts.speak(message, true) {
                warn!("Error Sending TTS: {:?}", e);
            }
        }
    }
}

pub async fn spawn_tts_service(settings: SettingsHandle, rx: Receiver<String>, shutdown: Shutdown) {
    info!("Starting TTS Service..");
    let tts = TTS::new(settings);
    if tts.is_err() {
        warn!("Unable to Start TTS Service");
        return;
    }
    tts.unwrap().listen(rx, shutdown).await;
}

/*
Below is a 'Dummy' implementation of the Tts struct, which is a simple 'Nothing' for use if the
actual Tts package isn't included.
 */

#[cfg(not(feature = "tts"))]
struct Tts {}

#[cfg(not(feature = "tts"))]
impl Tts {
    fn default() -> Result<Self> {
        warn!("TTS Feature is not enabled in build, TTS will not work.");
        Ok(Self {})
    }

    pub fn stop(&self) -> Result<()> {
        Ok(())
    }

    pub fn is_speaking(&self) -> Result<bool> {
        Ok(true)
    }

    pub fn speak(&self, _: String, _: bool) -> Result<()> {
        Ok(())
    }

    pub fn set_rate(&mut self, _rate: f32) -> Result<()> {
        Ok(())
    }

    pub fn max_rate(&self) -> f32 {
        0.
    }
}
