use crate::buttonstate::{ButtonStates, Buttons, CurrentButtonStates};
use crate::channelstate::ChannelState;
use crate::commands::SystemInfoCommand;
use crate::commands::SystemInfoCommand::SupportsDCPCategory;
use crate::commands::{Command, HardwareInfoCommand};
use crate::dcp::DCPCategory;
use crate::error::{CommandError, ConnectError};
use crate::routing::InputDevice;
use byteorder::{ByteOrder, LittleEndian, ReadBytesExt, WriteBytesExt};
use enumset::EnumSet;
use goxlr_types::{
    ChannelName, EffectKey, EncoderName, FaderName, FirmwareVersions, MicrophoneParamKey,
    MicrophoneType, VersionNumber,
};
use log::{debug, error, info, warn};
use rusb::Error::Pipe;
use rusb::{
    Device, DeviceDescriptor, DeviceHandle, Direction, Language, Recipient, RequestType, UsbContext,
};
use std::io::{Cursor, Write};
use std::thread::sleep;
use std::time::Duration;

#[derive(Debug)]
pub struct GoXLR<T: UsbContext> {
    handle: DeviceHandle<T>,
    device: Device<T>,
    device_descriptor: DeviceDescriptor,
    timeout: Duration,
    language: Language,
    command_count: u16,
    device_is_claimed: bool,
}

pub const VID_GOXLR: u16 = 0x1220;
pub const PID_GOXLR_MINI: u16 = 0x8fe4;
pub const PID_GOXLR_FULL: u16 = 0x8fe0;

impl<T: UsbContext> GoXLR<T> {
    pub fn from_device(
        mut handle: DeviceHandle<T>,
        device_descriptor: DeviceDescriptor,
    ) -> Result<Self, ConnectError> {
        let device = handle.device();
        let timeout = Duration::from_secs(1);

        info!("Connected to possible GoXLR device at {:?}", device);

        let languages = handle.read_languages(timeout)?;
        let language = languages
            .get(0)
            .ok_or(ConnectError::DeviceNotGoXLR)?
            .to_owned();

        debug!(
            "Set Active Config: {:?}",
            handle.set_active_configuration(1)
        );
        let device_is_claimed = handle.claim_interface(0).is_ok();

        let mut goxlr = Self {
            handle,
            device,
            device_descriptor,
            timeout,
            language,
            command_count: 0,
            device_is_claimed,
        };

        // Resets the state of the device (unconfirmed - Might just be the command id counter)
        let result = goxlr.write_control(1, 0, 0, &[]);

        if result == Err(Pipe) {
            // The GoXLR is not initialised, we need to fix that..
            info!("Found uninitialised GoXLR, attempting initialisation..");
            if device_is_claimed {
                goxlr.handle.release_interface(0)?;
            }
            goxlr.handle.set_auto_detach_kernel_driver(true)?;

            if goxlr.handle.claim_interface(0).is_err() {
                return Err(ConnectError::DeviceNotClaimed);
            }

            debug!("Activating Vendor Interface...");
            goxlr.read_control(0, 0, 0, 24)?;

            // Now activate audio..
            debug!("Activating Audio...");
            goxlr.write_class_control(1, 0x0100, 0x2900, &[0x80, 0xbb, 0x00, 0x00])?;

            goxlr.handle.release_interface(0)?;

            // Reset the device, so ALSA can pick it up again..
            goxlr.handle.reset()?;

            // Reattempt the reset..
            goxlr.write_control(1, 0, 0, &[])?;

            warn!(
                "Initialisation complete. If you are using the JACK script, you may need to reboot for audio to work."
            );

            // Pause for a second, as we can grab devices a little too quickly!
            sleep(Duration::from_secs(1));
        }

        // Force command pipe activation in all cases.
        debug!("Handling initial request");
        goxlr.read_control(3, 0, 0, 1040)?;
        Ok(goxlr)
    }

    pub fn usb_device_descriptor(&self) -> &DeviceDescriptor {
        &self.device_descriptor
    }

    pub fn usb_device_manufacturer(&self) -> Result<String, rusb::Error> {
        self.handle.read_manufacturer_string(
            self.language,
            &self.device_descriptor,
            Duration::from_millis(100),
        )
    }

    pub fn usb_device_product_name(&self) -> Result<String, rusb::Error> {
        self.handle.read_product_string(
            self.language,
            &self.device_descriptor,
            Duration::from_millis(100),
        )
    }

    pub fn usb_device_is_claimed(&self) -> bool {
        self.device_is_claimed
    }

    pub fn usb_device_has_kernel_driver_active(&self) -> Result<bool, rusb::Error> {
        self.handle.kernel_driver_active(0)
    }

    pub fn usb_bus_number(&self) -> u8 {
        self.device.bus_number()
    }

    pub fn usb_address(&self) -> u8 {
        self.device.address()
    }

    pub fn read_control(
        &mut self,
        request: u8,
        value: u16,
        index: u16,
        length: usize,
    ) -> Result<Vec<u8>, rusb::Error> {
        let mut buf = vec![0; length];
        let response_length = self.handle.read_control(
            rusb::request_type(Direction::In, RequestType::Vendor, Recipient::Interface),
            request,
            value,
            index,
            &mut buf,
            self.timeout,
        )?;
        buf.truncate(response_length);
        Ok(buf)
    }

    pub fn write_control(
        &mut self,
        request: u8,
        value: u16,
        index: u16,
        data: &[u8],
    ) -> Result<(), rusb::Error> {
        self.handle.write_control(
            rusb::request_type(Direction::Out, RequestType::Vendor, Recipient::Interface),
            request,
            value,
            index,
            data,
            self.timeout,
        )?;

        Ok(())
    }

    fn write_class_control(
        &mut self,
        request: u8,
        value: u16,
        index: u16,
        data: &[u8],
    ) -> Result<(), rusb::Error> {
        self.handle.write_control(
            rusb::request_type(Direction::Out, RequestType::Class, Recipient::Interface),
            request,
            value,
            index,
            data,
            self.timeout,
        )?;

        Ok(())
    }

    pub fn request_data(&mut self, command: Command, body: &[u8]) -> Result<Vec<u8>, rusb::Error> {
        self.perform_request(command, body, false)
    }

    fn perform_request(
        &mut self,
        command: Command,
        body: &[u8],
        is_retry: bool,
    ) -> Result<Vec<u8>, rusb::Error> {
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

        self.write_control(2, 0, 0, &full_request)?;

        // The full fat GoXLR can handle requests incredibly quickly..
        let mut sleep_time = Duration::from_millis(3);
        if self.device_descriptor.product_id() == PID_GOXLR_MINI {
            // The mini, however, cannot.
            sleep_time = Duration::from_millis(10);
        }
        sleep(sleep_time);

        // Interrupt reading doesnt work, because we can't claim the interface.
        //self.await_interrupt(Duration::from_secs(2));

        let mut response = vec![];

        for i in 0..20 {
            let response_value = self.read_control(3, 0, 0, 1040);
            if response_value == Err(Pipe) {
                if i < 20 {
                    debug!("Response not arrived yet for {:?}, sleeping and retrying (Attempt {} of 20)", command, i + 1);
                    sleep(sleep_time);
                    continue;
                } else {
                    debug!("Failed to receive response (Attempt 20 of 20), possible Dead GoXLR?");
                    return Err(response_value.err().unwrap());
                }
            }
            if response_value.is_err() {
                let err = response_value.err().unwrap();
                debug!("Error Occurred during packet read: {}", err);
                return Err(err);
            }

            let mut response_header = response_value.unwrap();
            if response_header.len() < 16 {
                error!(
                    "Invalid Response received from the GoXLR, Expected: 16, Received: {}",
                    response_header.len()
                );
                return Err(Pipe);
            }

            response = response_header.split_off(16);
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

                return if !is_retry {
                    debug!("Attempting Resync and Retry");
                    let _ = self.perform_request(Command::ResetCommandIndex, &[], true)?;

                    debug!("Resync complete, retrying Command..");
                    self.perform_request(command, body, true)
                } else {
                    debug!("Resync Failed, Throwing Error..");
                    Err(rusb::Error::Other)
                };
            }

            debug_assert!(response.len() == response_length as usize);
            break;
        }

        Ok(response)
    }

    pub fn supports_dcp_category(&mut self, category: DCPCategory) -> Result<bool, rusb::Error> {
        let mut out = [0; 2];
        LittleEndian::write_u16(&mut out, category.id());
        let result = self.request_data(Command::SystemInfo(SupportsDCPCategory), &out)?;
        Ok(LittleEndian::read_u16(&result) == 1)
    }

    pub fn get_system_info(&mut self) -> Result<(), rusb::Error> {
        let _result =
            self.request_data(Command::SystemInfo(SystemInfoCommand::FirmwareVersion), &[])?;
        // TODO: parse that?
        Ok(())
    }

    pub fn get_firmware_version(&mut self) -> Result<FirmwareVersions, CommandError> {
        let result = self.request_data(
            Command::GetHardwareInfo(HardwareInfoCommand::FirmwareVersion),
            &[],
        )?;
        let mut cursor = Cursor::new(result);
        let firmware_packed = cursor.read_u32::<LittleEndian>()?;
        let firmware_build = cursor.read_u32::<LittleEndian>()?;
        let firmware = VersionNumber(
            firmware_packed >> 12,
            (firmware_packed >> 8) & 0xF,
            firmware_packed & 0xFF,
            firmware_build,
        );

        let _unknown = cursor.read_u32::<LittleEndian>()?;
        let fpga_count = cursor.read_u32::<LittleEndian>()?;

        let dice_build = cursor.read_u32::<LittleEndian>()?;
        let dice_packed = cursor.read_u32::<LittleEndian>()?;
        let dice = VersionNumber(
            (dice_packed >> 20) & 0xF,
            (dice_packed >> 12) & 0xFF,
            dice_packed & 0xFFF,
            dice_build,
        );

        Ok(FirmwareVersions {
            firmware,
            fpga_count,
            dice,
        })
    }

    pub fn get_serial_number(&mut self) -> Result<(String, String), CommandError> {
        let result = self.request_data(
            Command::GetHardwareInfo(HardwareInfoCommand::SerialNumber),
            &[],
        )?;

        let serial_slice = &result[..24];
        let serial_len = serial_slice
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(serial_slice.len()) as usize;
        let serial_number = String::from_utf8_lossy(&serial_slice[..serial_len]).to_string();

        let date_slice = &result[24..];
        let date_len = date_slice
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(date_slice.len()) as usize;
        let manufacture_date = String::from_utf8_lossy(&date_slice[..date_len]).to_string();

        Ok((serial_number, manufacture_date))
    }

    pub fn set_fader(&mut self, fader: FaderName, channel: ChannelName) -> Result<(), rusb::Error> {
        // Channel ID, unknown, unknown, unknown
        self.request_data(Command::SetFader(fader), &[channel as u8, 0x00, 0x00, 0x00])?;
        Ok(())
    }

    pub fn set_volume(&mut self, channel: ChannelName, volume: u8) -> Result<(), rusb::Error> {
        self.request_data(Command::SetChannelVolume(channel), &[volume])?;
        Ok(())
    }

    pub fn set_encoder_value(
        &mut self,
        encoder: EncoderName,
        value: i8,
    ) -> Result<(), rusb::Error> {
        self.request_data(Command::SetEncoderValue(encoder), &[value as u8])?;
        Ok(())
    }

    pub fn set_encoder_mode(
        &mut self,
        encoder: EncoderName,
        mode: u8,
        resolution: u8,
    ) -> Result<(), rusb::Error> {
        self.request_data(Command::SetEncoderMode(encoder), &[mode, resolution])?;
        Ok(())
    }

    pub fn set_channel_state(
        &mut self,
        channel: ChannelName,
        state: ChannelState,
    ) -> Result<(), rusb::Error> {
        self.request_data(Command::SetChannelState(channel), &[state.id()])?;
        Ok(())
    }

    pub fn set_button_states(&mut self, data: [ButtonStates; 24]) -> Result<(), rusb::Error> {
        self.request_data(Command::SetButtonStates(), &data.map(|state| state as u8))?;
        Ok(())
    }

    pub fn set_button_colours(&mut self, data: [u8; 328]) -> Result<(), rusb::Error> {
        self.request_data(Command::SetColourMap(), &data)?;
        Ok(())
    }

    pub fn set_button_colours_1_3_40(&mut self, data: [u8; 520]) -> Result<(), rusb::Error> {
        self.request_data(Command::SetColourMap(), &data)?;
        Ok(())
    }

    pub fn set_fader_display_mode(
        &mut self,
        fader: FaderName,
        gradient: bool,
        meter: bool,
    ) -> Result<(), rusb::Error> {
        // This one really doesn't need anything fancy..
        let gradient_byte = u8::from(gradient);
        let meter_byte = u8::from(meter);

        // TODO: Seemingly broken?
        self.request_data(
            Command::SetFaderDisplayMode(fader),
            &[gradient_byte, meter_byte],
        )?;
        Ok(())
    }

    pub fn set_fader_scribble(
        &mut self,
        fader: FaderName,
        data: [u8; 1024],
    ) -> Result<(), rusb::Error> {
        // Dump it, see what happens..
        self.request_data(Command::SetScribble(fader), &data)?;
        Ok(())
    }

    pub fn set_routing(
        &mut self,
        input_device: InputDevice,
        data: [u8; 22],
    ) -> Result<(), rusb::Error> {
        self.request_data(Command::SetRouting(input_device), &data)?;
        Ok(())
    }

    pub fn set_microphone_gain(
        &mut self,
        microphone_type: MicrophoneType,
        gain: u16,
    ) -> Result<(), CommandError> {
        let mut gain_value = [0; 4];
        LittleEndian::write_u16(&mut gain_value[2..], gain);
        self.set_mic_param(&[
            (
                MicrophoneParamKey::MicType,
                match microphone_type.has_phantom_power() {
                    true => [0x01, 0x00, 0x00, 0x00],
                    false => [0x00, 0x00, 0x00, 0x00],
                },
            ),
            (microphone_type.get_gain_param(), gain_value),
        ])?;
        Ok(())
    }

    pub fn get_microphone_level(&mut self) -> Result<u16, rusb::Error> {
        let result = self.request_data(Command::GetMicrophoneLevel, &[])?;

        Ok(LittleEndian::read_u16(&result))
    }

    pub fn set_effect_values(&mut self, effects: &[(EffectKey, i32)]) -> Result<(), CommandError> {
        let mut data = Vec::with_capacity(effects.len() * 8);
        let mut cursor = Cursor::new(&mut data);
        for (key, value) in effects {
            cursor.write_u32::<LittleEndian>(*key as u32)?;
            cursor.write_i32::<LittleEndian>(*value)?;
        }
        self.request_data(Command::SetEffectParameters, &data)?;

        Ok(())
    }

    pub fn set_mic_param(
        &mut self,
        params: &[(MicrophoneParamKey, [u8; 4])],
    ) -> Result<(), CommandError> {
        let mut data = Vec::with_capacity(params.len() * 8);
        let mut cursor = Cursor::new(&mut data);
        for (key, value) in params {
            cursor.write_u32::<LittleEndian>(*key as u32)?;
            cursor.write_all(value)?;
        }
        self.request_data(Command::SetMicrophoneParameters, &data)?;

        Ok(())
    }

    pub fn get_button_states(&mut self) -> Result<CurrentButtonStates, rusb::Error> {
        let result = self.request_data(Command::GetButtonStates, &[])?;
        let mut pressed = EnumSet::empty();
        let mut mixers = [0; 4];
        let mut encoders = [0; 4];
        let button_states = LittleEndian::read_u32(&result[0..4]);

        mixers[0] = result[8];
        mixers[1] = result[9];
        mixers[2] = result[10];
        mixers[3] = result[11];

        // These can technically be negative, cast straight to i8
        encoders[0] = result[4] as i8; // Pitch
        encoders[1] = result[5] as i8; // Gender
        encoders[2] = result[6] as i8; // Reverb
        encoders[3] = result[7] as i8; // Echo

        for button in EnumSet::<Buttons>::all() {
            if button_states & (1 << button as u8) != 0 {
                pressed.insert(button);
            }
        }

        Ok(CurrentButtonStates {
            pressed,
            volumes: mixers,
            encoders,
        })
    }

    pub fn await_interrupt(&mut self, duration: Duration) -> bool {
        let mut buffer = [0u8; 6];
        let message = self.handle.read_interrupt(0x81, &mut buffer, duration);
        if message.is_err() {
            println!("Error Reading Interrupt..");
        }

        matches!(
            //self.handle.read_interrupt(0x81, &mut buffer, duration),
            message,
            Ok(_)
        )
    }

    pub fn is_connected(&mut self) -> bool {
        debug!("Checking Disconnect for device: {:?}", self.device);
        let active_configuration = self.handle.active_configuration();
        if active_configuration.is_ok() {
            let result = self.request_data(Command::ResetCommandIndex, &[]);
            return if result.is_ok() {
                debug!("Device {:?} is still connected", self.device);
                true
            } else {
                debug!("Device {:?} has been disconnected", self.device);
                false
            };
        }
        false
    }
}
