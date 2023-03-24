use anyhow::{bail, Result};
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::Device;
use log::{debug, info};

pub struct CpalConfiguration {}

impl CpalConfiguration {
    pub(crate) fn get_device(device: Option<String>, input: bool) -> Result<Device> {
        let mut cpal_device = None;

        // Basically, if *ANYTHING* goes wrong here, we'll fall through to default.
        if let Some(device_name) = device {
            debug!("Looking for Device: {}", device_name);
            if let Some(position) = device_name.find('*') {
                let str_host = &device_name[0..position];
                let str_device = &device_name[position + 1..device_name.len()];

                debug!("Split: {} {}", str_host, str_device);

                // Ok, now for cpal, find the correct host..
                let cpal_host_list = cpal::available_hosts();
                let host_id = cpal_host_list.iter().find(|x| x.name() == str_host);

                if let Some(host_id) = host_id {
                    debug!("Host {} found, checking devices", host_id.name());
                    if let Ok(host) = cpal::host_from_id(*host_id) {
                        debug!("Found Host..");
                        if let Ok(mut devices) = host.devices() {
                            if let Some(device) = devices.find(|x| {
                                debug!(
                                    "Checking Device: {}",
                                    x.name().unwrap_or("UNKNOWN".to_string())
                                );
                                let is_input = CpalConfiguration::device_is_input(x);
                                let is_output = CpalConfiguration::device_is_output(x);

                                // Only do checks if this device isn't an input AND output
                                if !is_input || !is_output {
                                    if is_input && !input {
                                        debug!("This device is Input, looking for Output");
                                        return false;
                                    }
                                    if is_output && input {
                                        debug!("This device is Output, looking for Input");
                                        return false;
                                    }
                                }

                                if x.name().unwrap_or_else(|_| "UNKNOWN".to_string()) == str_device
                                {
                                    debug!("Device Found");
                                    return true;
                                }
                                debug!("Not the Device we are looking for!");
                                false
                            }) {
                                cpal_device = Some(device)
                            }
                        } else {
                            debug!("Audio Devices not found");
                        }
                    } else {
                        debug!("Audio Host not Found");
                    }
                } else {
                    debug!("Audio Host ID Not Found");
                }
            }
        }

        if let Some(device) = cpal_device {
            info!(
                "Sampler device found: {}",
                device.name().unwrap_or_else(|_| String::from("UNKNOWN"))
            );
            Ok(device)
        } else {
            info!("Sampler Device not Found");
            let host = cpal::default_host();
            let default_device = if input {
                host.default_input_device()
            } else {
                host.default_output_device()
            };

            match default_device {
                Some(device) => Ok(device),
                None => bail!("Unable to find Default Device"),
            }
        }
    }

    pub(crate) fn get_outputs() -> Vec<String> {
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

    pub(crate) fn get_inputs() -> Vec<String> {
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

    fn device_is_input(device: &Device) -> bool {
        device
            .supported_input_configs()
            .map(|mut iter| iter.next().is_some())
            .unwrap_or(false)
    }

    fn device_is_output(device: &Device) -> bool {
        device
            .supported_output_configs()
            .map(|mut iter| iter.next().is_some())
            .unwrap_or(false)
    }
}
