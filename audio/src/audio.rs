use anyhow::Result;
use symphonia::core::audio::SignalSpec;

pub trait AudioOutput {
    fn write(&mut self, samples: &[f32]) -> Result<()>;
    fn flush(&mut self);
}

pub trait AudioInput {
    fn read(&mut self) -> Result<Vec<f32>>;
    fn flush(&mut self);
}

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

#[cfg(target_os = "linux")]
pub(crate) fn get_input(device: Option<String>) -> Result<Box<dyn AudioInput>> {
    // I have no idea why IntelliJ throws an error for the next line, it's fine!
    crate::pulse::record::PulseAudioInput::open(device)
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn get_output(
    signal_spec: SignalSpec,
    device: Option<String>,
) -> Result<Box<dyn AudioOutput>> {
    crate::cpal::playback::CpalAudioOutput::open(signal_spec, device)
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn get_input(device: Option<String>) -> Result<Box<dyn AudioInput>> {
    crate::cpal::playback::CpalAudioInput::open(device)
}
