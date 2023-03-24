use anyhow::Result;
use symphonia::core::audio::SignalSpec;

pub trait OpenOutputStream {
    fn open(spec: AudioSpecification) -> Result<Box<dyn AudioOutput>>;
}

pub trait OpenInputStream {
    fn open(spec: AudioSpecification) -> Result<Box<dyn AudioInput>>;
}

pub trait AudioOutput {
    fn write(&mut self, samples: &[f32]) -> Result<()>;
    fn flush(&mut self);
    fn stop(&mut self);
}

pub trait AudioInput {
    fn read(&mut self) -> Result<Vec<f32>>;
    fn flush(&mut self);
}

pub struct AudioSpecification {
    pub device: Option<String>,
    pub spec: SignalSpec,
    pub buffer: usize,
}

#[cfg(target_os = "linux")]
pub(crate) fn get_output(spec: AudioSpecification) -> Result<Box<dyn AudioOutput>> {
    crate::pulse::pulse_playback::PulsePlayback::open(spec)
}

#[cfg(target_os = "linux")]
pub(crate) fn get_input(spec: AudioSpecification) -> Result<Box<dyn AudioInput>> {
    crate::pulse::pulse_record::PulseRecord::open(spec)
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn get_output(spec: AudioSpecification) -> Result<Box<dyn AudioOutput>> {
    crate::cpal::cpal_playback::CpalPlayback::open(spec)
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn get_input(spec: AudioSpecification) -> Result<Box<dyn AudioInput>> {
    crate::cpal::cpal_record::CpalRecord::open(spec)
}
