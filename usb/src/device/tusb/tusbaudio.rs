use crate::device::base::GoXLRDevice;
use anyhow::{anyhow, bail, Result};
use byteorder::{ByteOrder, LittleEndian};
use lazy_static::lazy_static;
use libloading::{Library, Symbol};
use log::{debug, error, info, warn};
use std::ffi::CStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::sync::mpsc::{Receiver, Sender};
use widestring::U16CStr;
use windows::Win32::Foundation::{HANDLE, WIN32_ERROR};
use windows::Win32::System::Threading::{CreateEventA, WaitForSingleObject};

// Define the Types of the various methods..
type EnumerateDevices = unsafe extern "C" fn() -> u32;
type GetAPIVersion = unsafe extern "C" fn() -> ApiVersion;
type CheckAPIVersion = unsafe extern "C" fn(u32, u32) -> bool;
type GetDeviceCount = unsafe extern "C" fn() -> u32;
type OpenDeviceByIndex = unsafe extern "C" fn(u32, &u32) -> u32;
type GetDeviceInstanceIdString = unsafe extern "C" fn(u32, *const u16, u32) -> u32;
type GetDeviceProperties = unsafe extern "C" fn(u32, *mut Properties) -> u32;
type GetUsbConfigDescriptor = unsafe extern "C" fn(u32, *mut u8, u32, &u32) -> u32;
type VendorRequestOut =
    unsafe extern "C" fn(u32, u32, u32, u32, u32, u16, u16, *const u8, *mut u8, u32) -> u32;
type VendorRequestIn =
    unsafe extern "C" fn(u32, u32, u32, u32, u32, u16, u16, *mut u8, *mut u8, u32) -> u32;
type RegisterDeviceNotification = unsafe extern "C" fn(u32, u32, HANDLE, u32) -> u32;
type RegisterPnpNotification = unsafe extern "C" fn(HANDLE, HANDLE, u32, u32, u32) -> u32;
type ReadDeviceNotification = unsafe extern "C" fn(u32, &u32, *mut u8, u32, &u32) -> u32;
type StatusCodeString = unsafe extern "C" fn(u32) -> *const i8;
type CloseDevice = unsafe extern "C" fn(u32) -> u32;

lazy_static! {
    // Initialise the Library..
    static ref LIBRARY: Library = unsafe {
        libloading::Library::new("C:/Program Files/TC-HELICON/GoXLR_Audio_Driver/W10_x64/goxlr_audioapi_x64.dll").expect("Unable to Load GoXLR API Driver")
    };
    pub static ref TUSB_INTERFACE: TUSBAudio<'static> = TUSBAudio::new().expect("Unable to Parse GoXLR API Driver");
}

#[allow(dead_code)]
pub struct TUSBAudio<'lib> {
    // Need to enumerate..
    initial_enumeration: Arc<Mutex<bool>>,
    pnp_thread_running: Arc<Mutex<bool>>,
    discovered_devices: Arc<Mutex<Vec<String>>>,

    // API Related Commands
    get_api_version: Symbol<'lib, GetAPIVersion>,
    check_api_version: Symbol<'lib, CheckAPIVersion>,

    // Enumeration / Opening..
    enumerate_devices: Symbol<'lib, EnumerateDevices>,
    open_device_by_index: Symbol<'lib, OpenDeviceByIndex>,

    get_device_count: Symbol<'lib, GetDeviceCount>,
    get_device_id_string: Symbol<'lib, GetDeviceInstanceIdString>,
    get_device_properties: Symbol<'lib, GetDeviceProperties>,
    get_device_usb: Symbol<'lib, GetUsbConfigDescriptor>,

    // Sending and Receiving..
    vendor_request_out: Symbol<'lib, VendorRequestOut>,
    vendor_request_in: Symbol<'lib, VendorRequestIn>,

    register_pnp_notification: Symbol<'lib, RegisterPnpNotification>,
    register_device_notification: Symbol<'lib, RegisterDeviceNotification>,
    read_device_notification: Symbol<'lib, ReadDeviceNotification>,

    status_code_string: Symbol<'lib, StatusCodeString>,

    // Closing
    close_device: Symbol<'lib, CloseDevice>,
}

impl TUSBAudio<'_> {
    pub fn new() -> Result<Self> {
        let get_api_version: Symbol<_> = unsafe { LIBRARY.get(b"TUSBAUDIO_GetApiVersion")? };
        let check_api_version = unsafe { LIBRARY.get(b"TUSBAUDIO_CheckApiVersion")? };

        let enumerate_devices = unsafe { LIBRARY.get(b"TUSBAUDIO_EnumerateDevices")? };
        let open_device_by_index = unsafe { LIBRARY.get(b"TUSBAUDIO_OpenDeviceByIndex")? };

        let get_device_count = unsafe { LIBRARY.get(b"TUSBAUDIO_GetDeviceCount")? };
        let get_device_id_string = unsafe { LIBRARY.get(b"TUSBAUDIO_GetDeviceInstanceIdString")? };
        let get_device_properties = unsafe { LIBRARY.get(b"TUSBAUDIO_GetDeviceProperties")? };
        let get_device_usb = unsafe { LIBRARY.get(b"TUSBAUDIO_GetUsbConfigDescriptor")? };

        let vendor_request_out = unsafe { LIBRARY.get(b"TUSBAUDIO_ClassVendorRequestOut")? };
        let vendor_request_in = unsafe { LIBRARY.get(b"TUSBAUDIO_ClassVendorRequestIn")? };

        let register_device_notification =
            unsafe { LIBRARY.get(b"TUSBAUDIO_RegisterDeviceNotification")? };
        let register_pnp_notification =
            unsafe { LIBRARY.get(b"TUSBAUDIO_RegisterPnpNotification")? };
        let read_device_notification = unsafe { LIBRARY.get(b"TUSBAUDIO_ReadDeviceNotification")? };

        let status_code_string = unsafe { LIBRARY.get(b"TUSBAUDIO_StatusCodeStringA")? };
        let close_device = unsafe { LIBRARY.get(b"TUSBAUDIO_CloseDevice")? };

        let tusb_audio = Self {
            initial_enumeration: Arc::new(Mutex::new(false)),
            pnp_thread_running: Arc::new(Mutex::new(false)),
            discovered_devices: Arc::new(Mutex::new(Vec::new())),

            get_api_version,
            check_api_version,
            enumerate_devices,
            open_device_by_index,
            get_device_count,
            get_device_id_string,
            get_device_properties,
            get_device_usb,

            vendor_request_out,
            vendor_request_in,

            register_pnp_notification,
            register_device_notification,
            read_device_notification,

            status_code_string,
            close_device,
        };

        let api_version = unsafe { (tusb_audio.get_api_version)() };
        if api_version.major != 7 || api_version.minor != 5 {
            warn!(
                "API VERSION DETECTED: {}.{}",
                api_version.major, api_version.minor
            );
            warn!("API VERSION MISMATCH: This code was made with Version 7.5 of the API");
            warn!("Please install version 5.12.0 of the GoXLR Drivers");
            warn!("We'll try to keep going, but you may experience instability");
        } else {
            info!(
                "Using GoXLR API Version {}.{}",
                api_version.major, api_version.minor
            );
        }

        Ok(tusb_audio)
    }

    fn get_error(&self, error: u32) -> String {
        let res = unsafe { (self.status_code_string)(error) };
        let text = unsafe { CStr::from_ptr(res) };

        return text.to_string_lossy().to_string();
    }

    // We need to mildly abuse inner mutability here, due to the nature of lazy_static..
    fn enumerate_devices(&self) {
        unsafe { (self.enumerate_devices)() };
    }

    fn detect_devices(&self) -> Result<()> {
        let mut result_vec = Vec::new();
        self.enumerate_devices();

        let device_count = self.get_device_count();
        for i in 0..device_count {
            let handle = self.open_device_by_index(i)?;
            result_vec.push(self.get_device_id_string(handle)?);
            self.close_device(handle)?;
        }

        // All devices handled, replace the stored vec..
        let mut discovered = self.discovered_devices.lock().unwrap();
        *discovered = result_vec;

        Ok(())
    }

    pub fn get_devices(&self) -> Vec<String> {
        let mut initial_check = self.initial_enumeration.lock().unwrap();
        if !*initial_check {
            if let Err(error) = self.detect_devices() {
                error!("Error Detecting Devices: {}", error);
                return Vec::new();
            }
            *initial_check = true;
        }

        self.discovered_devices.lock().unwrap().clone()
    }

    fn get_device_count(&self) -> u32 {
        unsafe { (self.get_device_count)() }
    }

    pub fn get_properties_by_handle(&self, handle: u32) -> Result<Properties> {
        // Create the properties struct..
        let mut properties = Properties::default();

        // Grab the Pointer for it..
        let properties_ptr: *mut Properties = &mut properties;

        // Attempt to get the Properties for this device..
        let result = unsafe { (self.get_device_properties)(handle, properties_ptr) };

        if result != 0 {
            bail!("Unable to Get Properties: {}", result);
        }

        Ok(properties)
    }

    fn get_device_id_string(&self, handle: u32) -> Result<String> {
        let buffer: Vec<u16> = Vec::with_capacity(100);
        let buffer_pointer = buffer.as_ptr();
        let result = unsafe { (self.get_device_id_string)(handle, buffer_pointer, 100) };

        if result != 0 {
            let error = self.get_error(result);
            bail!("Error Getting Device Id: {}", error);
        }

        let device_id = unsafe { U16CStr::from_ptr_truncate(buffer_pointer, 100)? };
        Ok(device_id.to_string_lossy())
    }

    pub fn send_request(
        &self,
        handle: u32,
        request: u8,
        value: u16,
        index: u16,
        data: &[u8],
    ) -> Result<()> {
        let data_length: u16 = data.len().try_into().unwrap();
        let data_pointer = data.as_ptr();

        // The Driver writes number of bytes written to a buffer, so create it here.
        let bytes_written_len: u32 = 64;
        let mut bytes_written = Vec::with_capacity(bytes_written_len as usize);
        let bytes_written_ptr = bytes_written.as_mut_ptr();

        let result = unsafe {
            (self.vendor_request_out)(
                handle,
                1_u32,
                0_u32,
                request.into(),
                value.into(),
                index,
                data_length,
                data_pointer,
                bytes_written_ptr,
                bytes_written_len,
            )
        };

        if result != 0 {
            // Known Errors:
            // 4009754628 - ?
            // 3992977412 - Invalid Request
            // 3992977480 - INVALID HANDLE!

            //3992977411
            bail!("{}", self.get_error(result));
        }

        Ok(())
    }

    pub fn read_response(
        &self,
        handle: u32,
        request: u8,
        value: u16,
        index: u16,
        length: usize,
    ) -> Result<Vec<u8>> {
        let mut buffer: Vec<u8> = Vec::with_capacity(length);
        let buffer_ptr = buffer.as_mut_ptr();

        let buffer2_len = 64;
        let mut buffer2: Vec<u8> = Vec::with_capacity(buffer2_len as usize);
        let buffer2_ptr = buffer2.as_mut_ptr();

        // This likely has 'Bytes Returned' somewhere in there, we need to check for that!
        let result = unsafe {
            (self.vendor_request_in)(
                handle,
                1_u32,
                0_u32,
                request as u32,
                value as u32,
                index,
                length.try_into().unwrap(),
                buffer_ptr,
                buffer2_ptr,
                buffer2_len,
            )
        };

        if result != 0 {
            bail!("{}", self.get_error(result));
        }

        // Ok, Buffer2 contains a u32 containing the length of the returned response..
        let len = unsafe { std::slice::from_raw_parts(buffer2_ptr, 4) };
        let read_len = LittleEndian::read_u32(len);

        let data = unsafe { std::slice::from_raw_parts(buffer_ptr, read_len as usize) };
        Ok(Vec::from(data))
    }

    pub fn open_device_by_identifier(&self, identifier: String) -> Result<u32> {
        let device_index = self
            .discovered_devices
            .lock()
            .unwrap()
            .iter()
            .position(|id| identifier == id.clone())
            .ok_or_else(|| anyhow!("Cannot Find Device"))?;
        self.open_device_by_index(device_index.try_into()?)
    }

    fn open_device_by_index(&self, device_index: u32) -> Result<u32> {
        let handle: u32 = 0;
        let result = unsafe { (self.open_device_by_index)(device_index, &handle) };
        if result == 0 {
            return Ok(handle);
        }
        bail!("Unable to Open Device: {}", result)
    }

    pub fn close_device(&self, handle: u32) -> Result<()> {
        let close = unsafe { (self.close_device)(handle) };
        if close != 0 {
            bail!("Unable to Close Handle: {}", close);
        }
        Ok(())
    }

    pub fn event_loop(
        &self,
        device_identifier: String,
        identifier: Arc<Mutex<Option<String>>>,
        callbacks: EventChannelSender,
        terminator: Arc<AtomicBool>,
    ) -> Result<()> {
        // Open a Handle to the Device..
        let mut handle = TUSB_INTERFACE.open_device_by_identifier(device_identifier.clone())?;

        // Register a new windows event..
        let event = unsafe { CreateEventA(None, false, false, None)? };

        // Register this event with the notifier..
        let result = unsafe { (self.register_device_notification)(handle, u32::MAX, event, 0) };
        if result != 0 {
            bail!("Unable to register notifications");
        }

        // Assign useful variables for later :p
        let mut buffer: Vec<u8> = Vec::with_capacity(1024);
        let buffer_ptr = buffer.as_mut_ptr();
        let a: u32 = 0;
        let response_len: u32 = 0;

        // Now we loop :D
        loop {
            // Wait for the event Trigger (I'd love for this to be async one day :p)..
            let wait_result = unsafe { WaitForSingleObject(event, 500) };
            if wait_result != WIN32_ERROR(258) {
                // Check the Queued Events :)
                loop {
                    let event_result = unsafe {
                        (self.read_device_notification)(handle, &a, buffer_ptr, 1024, &response_len)
                    };
                    if event_result != 0 {
                        // We've either hit the end of the list, or something's gone wrong, break
                        // out and double check our handle.
                        if event_result != 3992977442 {
                            debug!("Error Reading Event! {}", event_result);
                        }
                        break;
                    }

                    let event_response =
                        unsafe { std::slice::from_raw_parts(buffer_ptr, response_len as usize) };

                    if event_response.len() != 6 {
                        debug!(
                            "Unexpected Event Response Length: {}: {:?}",
                            event_response.len(),
                            event_response
                        );
                        continue;
                    }

                    if event_response[0] == 1 && event_response[1] == 1 && event_response[2] == 1 {
                        // This event indicates something waiting to be read..
                        let se = callbacks.data_read.blocking_send(true);
                        if se.is_err() {
                            // Something's gone horribly wrong!
                            terminator.store(true, Ordering::Relaxed);
                            break;
                        }
                        continue;
                    }

                    if event_response[0] == 1 && event_response[1] == 1 && event_response[2] == 0 {
                        if let Some(identifier) = &*identifier.lock().unwrap() {
                            // A button or fader interrupt has been received.
                            if callbacks.input_changed.capacity() > 0 {
                                let se = callbacks.input_changed.blocking_send(identifier.clone());
                                if se.is_err() {
                                    // Something's gone horribly wrong!
                                    terminator.store(true, Ordering::Relaxed);
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            if terminator.load(Ordering::Relaxed) {
                debug!("Terminator has told us to terminate.");
                break;
            }

            // This code simply validates our handle is good, and re-establishes if needed.
            if let Err(_error) = self.get_device_id_string(handle) {
                // Handle appears broken, try making a new one..
                let new_handle =
                    TUSB_INTERFACE.open_device_by_identifier(device_identifier.clone());

                // This one is broken too!
                if new_handle.is_err() {
                    // Flag ourself for stop..
                    terminator.store(true, Ordering::Relaxed);

                    // Break any read locks which might be currently occurring, and tell then we're dead.
                    let _ = callbacks.data_read.blocking_send(false);

                    // Also trigger an 'input_changed' event, so the main handler can discover that
                    // we're dead if it wasn't previously active..
                    if let Some(identifier) = &*identifier.lock().unwrap() {
                        // Trigger a callback to the main event handler, so it can check..
                        let _ = callbacks.input_changed.blocking_send(identifier.clone());
                    }
                    break;
                } else {
                    handle = new_handle?;
                }
            }
        }
        let _ = TUSB_INTERFACE.close_device(handle);

        warn!("Event Thread Terminated");
        bail!("Thread Terminated!")
    }

    pub fn spawn_pnp_handle(&self) -> Result<()> {
        let mut spawned = self.pnp_thread_running.lock().unwrap();
        if *spawned {
            bail!("Handler Thread already running..");
        }

        debug!("Spawning PnP Thread..");
        thread::spawn(|| -> Result<()> {
            let event = unsafe { CreateEventA(None, false, false, None)? };

            let result =
                unsafe { (TUSB_INTERFACE.register_pnp_notification)(event, event, 0, 0, 0) };
            if result != 0 {
                bail!("Unable to register notifications");
            }

            loop {
                let wait_result = unsafe { WaitForSingleObject(event, 1000) };
                if wait_result == WIN32_ERROR(258) {
                    // Timeout on wait, go again!
                    continue;
                }

                // We need to re-enumerate the devices..
                let _ = TUSB_INTERFACE.detect_devices();
            }
        });

        *spawned = true;
        Ok(())
    }
}

pub struct DeviceHandle {
    handle: u32,
}

impl DeviceHandle {
    pub fn from_device(device: GoXLRDevice) -> Result<Self> {
        // This is simple enough, iterate the devices until we find the one we want..
        if let Some(identifier) = device.identifier {
            let handle = TUSB_INTERFACE.open_device_by_identifier(identifier)?;
            return Ok(Self { handle });
        }
        bail!("Unable to Locate Device")
    }

    pub fn close_handle(&self) -> Result<()> {
        TUSB_INTERFACE.close_device(self.handle)
    }

    pub fn send_request(&self, request: u8, value: u16, index: u16, data: &[u8]) -> Result<()> {
        // Ok, need to work out what all this is, but still..
        TUSB_INTERFACE.send_request(self.handle, request, value, index, data)
    }

    pub fn read_response(
        &self,
        request: u8,
        value: u16,
        index: u16,
        length: usize,
    ) -> Result<Vec<u8>> {
        TUSB_INTERFACE.read_response(self.handle, request, value, index, length)
    }

    pub fn get_device_id_string(&self) -> Result<String> {
        TUSB_INTERFACE.get_device_id_string(self.handle)
    }

    pub fn get_properties(&self) -> Result<Properties> {
        TUSB_INTERFACE.get_properties_by_handle(self.handle)
    }
}

#[repr(C)]
struct ApiVersion {
    major: u16,
    minor: u16,
}

#[repr(C)]
pub struct Properties {
    vendor_id: i32,
    product_id: i32,
    revision_number: i32,
    serial_number: [u16; 128],
    manufacturer: [u16; 128],
    model: [u16; 128],

    // These two are likely provided specially by the driver, as they don't match anything
    // in the USB Spec.
    unknown_number: i32,
    unknown_string: [u16; 128],
}

impl Properties {
    pub fn vendor_id(&self) -> i32 {
        self.vendor_id
    }
    pub fn product_id(&self) -> i32 {
        self.product_id
    }
    pub fn manufacturer(&self) -> Result<String> {
        // Convert this from wide String, to regular String..
        Ok(U16CStr::from_slice_truncate(&self.manufacturer)?.to_string_lossy())
    }
    pub fn model(&self) -> Result<String> {
        // Convert this from wide String, to regular String..
        Ok(U16CStr::from_slice_truncate(&self.model)?.to_string_lossy())
    }
}

impl Default for Properties {
    fn default() -> Self {
        Self {
            vendor_id: 0,
            product_id: 0,
            revision_number: 0,
            serial_number: [0; 128],
            manufacturer: [0; 128],
            model: [0; 128],
            unknown_number: 0,
            unknown_string: [0; 128],
        }
    }
}

pub fn get_devices() -> Vec<GoXLRDevice> {
    let _ = TUSB_INTERFACE.spawn_pnp_handle();
    let mut list = Vec::new();

    // Ok, this is slightly different now..
    let devices = TUSB_INTERFACE.get_devices();
    for device in devices {
        list.push(GoXLRDevice {
            bus_number: 0,
            address: 0,
            identifier: Some(device),
        })
    }
    list
}

pub struct EventChannelReceiver {
    pub(crate) data_read: Receiver<bool>,
}

pub struct EventChannelSender {
    pub(crate) data_read: Sender<bool>,
    pub(crate) input_changed: Sender<String>,
}
