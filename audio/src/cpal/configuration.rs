use crate::AudioConfiguration;
use cpal::traits::{DeviceTrait, HostTrait};

pub struct CpalConfiguration {}

impl CpalConfiguration {
    fn new() -> Self {
        Self {}
    }
}

impl AudioConfiguration for CpalConfiguration {
    fn get_outputs(&mut self) -> Vec<String> {
        let mut list: Vec<String> = vec![];

        let available_hosts = cpal::available_hosts();
        for host_id in available_hosts {
            let host = cpal::host_from_id(host_id).unwrap();
            let devices = host.output_devices().unwrap();
            for (_device_index, device) in devices.enumerate() {
                list.push(format!("{}*{}", host_id.name(), device.name().unwrap()));
            }
        }
        list
    }

    fn get_inputs(&mut self) -> Vec<String> {
        let mut list: Vec<String> = vec![];

        let available_hosts = cpal::available_hosts();
        for host_id in available_hosts {
            let host = cpal::host_from_id(host_id).unwrap();
            let devices = host.input_devices().unwrap();
            for (_device_index, device) in devices.enumerate() {
                list.push(format!("{}*{}", host_id.name(), device.name().unwrap()));
            }
        }
        list
    }
}

pub fn get_configuration() -> CpalConfiguration {
    CpalConfiguration::new()
}
