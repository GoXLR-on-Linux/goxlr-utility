use crate::audio::AudioConfiguration;
use crate::pulse::configuration::get_configuration;

mod audio;
mod player;

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
