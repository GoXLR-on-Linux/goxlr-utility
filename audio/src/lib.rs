use crate::audio::get_configuration;

pub mod player;

mod audio;

#[cfg(target_os = "linux")]
mod pulse;

#[cfg(not(target_os = "linux"))]
mod cpal;

pub fn get_audio_outputs() -> Vec<String> {
    get_configuration().get_outputs()
}

pub fn get_audio_inputs() -> Vec<String> {
    get_configuration().get_inputs()
}
