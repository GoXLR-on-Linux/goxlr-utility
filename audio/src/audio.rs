use anyhow::Result;
use symphonia::core::audio::SignalSpec;

pub trait AudioOutput {
    fn write(&mut self, samples: &[f32]) -> Result<()>;
    fn flush(&mut self);
}

pub trait AudioInput {}

pub trait AudioConfiguration {
    fn get_outputs(&mut self) -> Vec<String>;
    fn get_inputs(&mut self) -> Vec<String>;
}

#[cfg(target_os = "linux")]
pub fn get_configuration() -> Box<dyn AudioConfiguration> {
    Box::new(crate::pulse::configuration::get_configuration())
}

#[cfg(not(target_os = "linux"))]
pub fn get_configuration() -> Box<dyn AudioConfiguration> {
    Box::new(crate::cpal::configuration::get_configuration())
}

#[cfg(target_os = "linux")]
pub(crate) fn get_output(
    signal_spec: SignalSpec,
    device: Option<String>,
) -> Result<Box<dyn AudioOutput>> {
    crate::pulse::playback::PulseAudioOutput::open(signal_spec, device)
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn get_output(
    signal_spec: SignalSpec,
    device: Option<String>,
) -> Result<Box<dyn AudioOutput>> {
    crate::cpal::playback::CpalAudioOutput::open(signal_spec, device)
    //crate::pulse::playback::PulseAudioOutput::open(signal_spec, device)
}
