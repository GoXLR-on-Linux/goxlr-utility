pub mod player;
pub mod recorder;

mod audio;

#[cfg(target_os = "linux")]
mod pulse;

#[cfg(not(target_os = "linux"))]
mod cpal;

pub fn get_audio_outputs() -> Vec<String> {
    #[cfg(target_os = "linux")]
    {
        use crate::pulse::pulse_config::PulseAudioConfiguration;
        PulseAudioConfiguration::get_outputs()
    }

    #[cfg(not(target_os = "linux"))]
    {
        use crate::cpal::cpal_config::CpalConfiguration;
        CpalConfiguration::get_outputs()
    }
}

pub fn get_audio_inputs() -> Vec<String> {
    #[cfg(target_os = "linux")]
    {
        use crate::pulse::pulse_config::PulseAudioConfiguration;
        PulseAudioConfiguration::get_inputs()
    }

    #[cfg(not(target_os = "linux"))]
    {
        use crate::cpal::cpal_config::CpalConfiguration;
        CpalConfiguration::get_inputs()
    }
}
