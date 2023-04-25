use crate::settings::SettingsHandle;
use crate::shutdown::Shutdown;
use anyhow::Result;
use log::{debug, info, warn};
use tokio::sync::mpsc::Receiver;

#[cfg(feature = "tts")]
use tts::Tts;

#[allow(clippy::upper_case_acronyms)]
pub(crate) struct TTS {
    settings: SettingsHandle,
    tts: Tts,
}

impl TTS {
    pub fn new(settings: SettingsHandle) -> Result<TTS> {
        let tts = Tts::default()?;
        Ok(Self { tts, settings })
    }

    pub async fn listen(&mut self, mut rx: Receiver<String>, mut shutdown: Shutdown) {
        loop {
            tokio::select! {
                () = shutdown.recv() => {
                    info!("Shutting down TTS Service");
                    return;
                },
                Some(message) = rx.recv() => {
                    debug!("Received TTS Message: {}", message);
                    self.speak_tts(message).await;
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

        if self.tts.stop().is_err() {
            warn!("Unable to Stop TTS Output");
            return;
        }

        match self.tts.speak(message, true) {
            Ok(_) => {}
            Err(error) => {
                warn!("Error Sending TTS: {}", error);
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
        Ok(Self {})
    }

    pub fn stop(&self) -> Result<()> {
        Ok(())
    }

    pub fn speak(&self, _: String, _: bool) -> Result<()> {
        Ok(())
    }
}
