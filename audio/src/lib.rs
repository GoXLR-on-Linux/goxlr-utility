use std::sync::atomic::{AtomicU64, Ordering};

mod audio;
pub mod player;
pub mod recorder;
mod ringbuffer;

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

// This is mostly a helper struct for converting between f64 and u64..
#[derive(Debug)]
pub struct AtomicF64 {
    storage: AtomicU64,
}
impl AtomicF64 {
    pub fn new(value: f64) -> Self {
        let as_u64 = value.to_bits();
        Self {
            storage: AtomicU64::new(as_u64),
        }
    }
    pub fn store(&self, value: f64, ordering: Ordering) {
        let as_u64 = value.to_bits();
        self.storage.store(as_u64, ordering)
    }
    pub fn load(&self, ordering: Ordering) -> f64 {
        let as_u64 = self.storage.load(ordering);
        f64::from_bits(as_u64)
    }
}
