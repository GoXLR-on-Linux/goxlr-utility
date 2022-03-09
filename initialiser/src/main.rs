use std::process::Command;
use std::time::Duration;
use rusb::{DeviceHandle, Direction, GlobalContext, Recipient, RequestType};
use rusb::Direction::{In, Out};
use rusb::Error::Pipe;
use rusb::Recipient::Interface;
use rusb::RequestType::{Class, Vendor};

pub const VID_GOXLR: u16 = 0x1220;
pub const PID_GOXLR_MINI: u16 = 0x8fe4;
pub const PID_GOXLR_FULL: u16 = 0x8fe0;

fn main() {
    println!("Checking for available GoXLR devices..");
    find_devices();
}

fn find_devices() {
    if let Ok(devices) = rusb::devices() {
        for device in devices.iter() {
            if let Ok(descriptor) = device.device_descriptor() {
                if descriptor.vendor_id() == VID_GOXLR
                    && (descriptor.product_id() == PID_GOXLR_FULL
                    || descriptor.product_id() == PID_GOXLR_MINI)
                {
                    match device.open() {
                        Ok(mut handle) => {
                            println!("Found GoXLR Device at {:?}, checking state..", handle);

                            handle.set_active_configuration(1);

                            // Send a single vendor command across, see what happens..
                            let request_type = rusb::request_type(Out, Vendor, Interface);
                            let result = handle.write_control(request_type, 1, 0, 0, &[], Duration::from_secs(1));

                            if result == Err(Pipe) {
                                println!("Device not initialised, preparing..");
                                // The GoXLR is not initialised, we need to fix that..
                                handle.set_auto_detach_kernel_driver(true);

                                if !handle.claim_interface(0).is_ok() {
                                    println!("Unable to claim, failed to initialise..");
                                    continue;
                                }

                                // Activate the GoXLR Vendor interface..
                                let request_type = rusb::request_type(In, Vendor, Interface);
                                let mut buf = vec![0; 24];
                                handle.read_control(request_type, 0, 0, 0, &mut buf, Duration::from_secs(1));

                                // Now activate audio..
                                let request_type = rusb::request_type(Out, Class, Interface);
                                handle.write_control(request_type, 1, 0x0100, 0x2900, &[0x80, 0xbb, 0x00, 0x00], Duration::from_secs(1));
                                handle.release_interface(0);

                                // Trigger a reset on the device, to hard reload kernel drivers, and reinit audio..
                                handle.reset();

                                println!("Device Initialised");
                            } else {
                                println!("Device already initialised");
                            }
                        }
                        Err(e) => {
                            println!("Unable to open the device.. {}", e);
                        }
                    }
                }
            }
        }
    }
}