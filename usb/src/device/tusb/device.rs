use crate::commands::Command;
use crate::device::base::{
    AttachGoXLR, ExecutableGoXLR, FullGoXLRDevice, GoXLRCommands, GoXLRDevice, UsbData,
};
use crate::device::tusb::tusbaudio::{
    get_devices, DeviceHandle, EventChannelReceiver, EventChannelSender, TUSB_INTERFACE,
};
use anyhow::{bail, Result};
use byteorder::{ByteOrder, LittleEndian};
use futures::executor::block_on;
use log::{debug, error};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::Sender;

pub struct TUSBAudioGoXLR {
    // Basic Device Information..
    handle: DeviceHandle,
    identifier: Option<String>,
    command_count: u16,

    // Event Handlers..
    event_receivers: EventChannelReceiver,
    disconnect_sender: Sender<String>,
    event_sender: Sender<String>,

    // Identifier for Daemon..
    daemon_identifier: Arc<Mutex<Option<String>>>,

    // Thread states
    stopped: Arc<AtomicBool>,
}

impl TUSBAudioGoXLR {
    fn write_control(&self, request: u8, value: u16, index: u16, data: &[u8]) -> Result<()> {
        self.handle.send_request(request, value, index, data)
    }

    fn read_control(
        &mut self,
        request: u8,
        value: u16,
        index: u16,
        length: usize,
    ) -> Result<Vec<u8>> {
        self.handle.read_response(request, value, index, length)
    }

    fn trigger_disconnect(&self) {
        let _ = self.handle.close_handle();
        self.stopped.store(true, Ordering::Relaxed);

        if let Some(daemon_identifier) = &*self.daemon_identifier.lock().unwrap() {
            let _ = block_on(self.disconnect_sender.send(daemon_identifier.clone()));
        }
    }

    fn await_data(&mut self) -> bool {
        // This is probably not the smartest way of doing this, but attempting to use a tokio future
        // against block_on can cause some weird runtime issues, and never resolve. Given that we
        // know a read event will return incredibly quickly, we can slap a loop in to wait for the
        // data.

        loop {
            let result = self.event_receivers.data_read.try_recv();
            match result {
                Ok(result) => break result,
                Err(TryRecvError::Disconnected) => break false,
                Err(_) => continue,
            }
        }
    }
}

impl AttachGoXLR for TUSBAudioGoXLR {
    fn from_device(
        device: GoXLRDevice,
        disconnect_sender: Sender<String>,
        event_sender: Sender<String>,
    ) -> anyhow::Result<Box<dyn FullGoXLRDevice>>
    where
        Self: Sized,
    {
        let mut device_identifier = None;
        if let Some(identifier) = &device.identifier {
            device_identifier = Some(identifier.clone());
        }

        let handle = DeviceHandle::from_device(device)?;

        // Spawn the Event handler thread..
        let (data_sender, data_receiver) = mpsc::channel(1);

        // In this case, we spawn a thread to manage windows events..
        let event_receivers = EventChannelReceiver {
            data_read: data_receiver,
        };

        let mut goxlr = Box::new(Self {
            handle,
            identifier: device_identifier,

            command_count: 0,

            event_receivers,
            disconnect_sender,
            event_sender,

            daemon_identifier: Arc::new(Mutex::new(None)),

            stopped: Arc::new(AtomicBool::new(false)),
        });

        // Spawn an event loop for this handle..
        let thread_sender = goxlr.event_sender.clone();
        let thread_daemon_identifier = goxlr.daemon_identifier.clone();
        let thread_stopped = goxlr.stopped.clone();
        if let Some(ref thread_device_identifier) = goxlr.identifier {
            // Clone it so we can move it into the thread..
            let thread_device_identifier = thread_device_identifier.clone();

            thread::spawn(move || {
                let sender = EventChannelSender {
                    data_read: data_sender.clone(),
                    input_changed: thread_sender,
                };

                // Spawn the Event Loop..
                let _ = TUSB_INTERFACE.event_loop(
                    thread_device_identifier.clone(),
                    thread_daemon_identifier,
                    sender,
                    thread_stopped,
                );
            });
        } else {
            bail!("Unable to Create Event Loop, Device Identifier not set!");
        }

        // Activate the Vendor interface, also initialises audio on Windows!
        if let Err(error) = goxlr.handle.read_response(0, 0, 0, 24) {
            goxlr.stopped.store(true, Ordering::Relaxed);
            bail!("Error Reading Initial Packet: {}", error);
        }

        // Perform soft reset.
        if let Err(error) = goxlr.handle.send_request(1, 0, 0, &[]) {
            goxlr.stopped.store(true, Ordering::Relaxed);
            bail!("Error Sending initial Reset Packet: {}", error);
        }

        // Wait for the response event, then read..
        if !goxlr.await_data() {
            bail!("Error received from Event Handler..");
        }

        if let Err(error) = goxlr.handle.read_response(3, 0, 0, 1040) {
            goxlr.stopped.store(true, Ordering::Relaxed);
            bail!("Error Reading Response to Initial Reset: {}", error);
        }
        Ok(goxlr)
    }

    fn set_unique_identifier(&mut self, identifier: String) {
        // Spawn Notification Thread..
        let mut local_identifier = self.daemon_identifier.lock().unwrap();
        *local_identifier = Some(identifier);
    }

    fn is_connected(&mut self) -> bool {
        // We need to verify and restore our handle if it's broken..
        if let Err(error) = self.handle.get_device_id_string() {
            debug!(
                "Connection Error: {}, attempting to create new handle..",
                error
            );
            let new_handle = DeviceHandle::from_device(GoXLRDevice {
                bus_number: 0,
                address: 0,
                identifier: self.identifier.clone(),
            });

            if new_handle.is_err() {
                return false;
            }
            self.handle = new_handle.unwrap();
        }
        true
    }
}

impl ExecutableGoXLR for TUSBAudioGoXLR {
    fn perform_request(
        &mut self,
        command: Command,
        body: &[u8],
        retry: bool,
    ) -> anyhow::Result<Vec<u8>> {
        if command == Command::ResetCommandIndex {
            self.command_count = 0;
        } else {
            if self.command_count == u16::MAX {
                let _ = self.request_data(Command::ResetCommandIndex, &[])?;
            }
            self.command_count += 1;
        }

        let command_index = self.command_count;
        let mut full_request = vec![0; 16];
        LittleEndian::write_u32(&mut full_request[0..4], command.command_id());
        LittleEndian::write_u16(&mut full_request[4..6], body.len() as u16);
        LittleEndian::write_u16(&mut full_request[6..8], command_index);
        full_request.extend(body);

        if let Err(error) = self.write_control(2, 0, 0, &full_request) {
            if error.to_string() == "TSTATUS_INVALID_HANDLE" {
                if self.is_connected() {
                    // Try again..
                    if let Err(error) = self.write_control(2, 0, 0, &full_request) {
                        self.trigger_disconnect();
                        bail!(
                            "Recovered Handle, but still unable to send command: {}",
                            error
                        );
                    }
                } else {
                    self.trigger_disconnect();
                    bail!("GoXLR has been Disconnected.");
                }
            } else {
                // Unknown Error,
                self.trigger_disconnect();
                bail!("Unknown Error, Disconnecting: {}", error);
            }
        }

        // We will sit here, and wait for a response.. this may take a few cycles..
        if !self.await_data() {
            self.trigger_disconnect();
            bail!("Event handler has ended, Disconnecting.");
        }

        let mut response_value = self.read_control(3, 0, 0, 1040);
        if let Err(error) = response_value {
            if error.to_string() == "TSTATUS_INVALID_HANDLE" {
                if self.is_connected() {
                    response_value = self.read_control(3, 0, 0, 1040);
                    if let Err(error) = response_value {
                        self.trigger_disconnect();
                        bail!(
                            "Recovered Handle, but still unable to read command response: {}",
                            error
                        );
                    }
                } else {
                    self.trigger_disconnect();
                    bail!("GoXLR has been Disconnected while Reading Response");
                }
            } else {
                self.trigger_disconnect();
                bail!("Unknown Error while Reading, Disconnecting: {}", error);
            }
        }

        let mut response_header = response_value?;
        if response_header.len() < 16 {
            error!(
                "Invalid Response received from the GoXLR, Expected: 16, Received: {}",
                response_header.len()
            );
            bail!("Invalid Response");
        }

        let response = response_header.split_off(16);
        let response_length = LittleEndian::read_u16(&response_header[4..6]);
        let response_command_index = LittleEndian::read_u16(&response_header[6..8]);

        if response_command_index != command_index {
            debug!("Mismatched Command Indexes..");
            debug!(
                "Expected {}, received: {}",
                command_index, response_command_index
            );
            debug!("Full Request: {:?}", full_request);
            debug!("Response Header: {:?}", response_header);
            debug!("Response Body: {:?}", response);

            return if !retry {
                debug!("Attempting Resync and Retry");
                self.perform_request(Command::ResetCommandIndex, &[], true)?;

                debug!("Resync complete, retrying Command..");
                self.perform_request(command, body, true)
            } else {
                debug!("Resync Failed, Throwing Error..");
                self.trigger_disconnect();
                bail!("Invalid Response received from GoXLR, disconnecting!");
            };
        }

        debug_assert!(response.len() == response_length as usize);
        Ok(response)
    }

    fn get_descriptor(&self) -> anyhow::Result<UsbData> {
        let properties = self.handle.get_properties()?;

        Ok(UsbData {
            vendor_id: properties.vendor_id() as u16,
            product_id: properties.product_id() as u16,
            device_version: (2, 0, 0),
            device_manufacturer: properties.manufacturer()?,
            product_name: properties.model()?,
        })
    }
}

impl GoXLRCommands for TUSBAudioGoXLR {}
impl FullGoXLRDevice for TUSBAudioGoXLR {}

pub fn find_devices() -> Vec<GoXLRDevice> {
    get_devices()
}
