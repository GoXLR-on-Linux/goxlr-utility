use crate::buttonstate::{ButtonStates, Buttons, CurrentButtonStates};
use crate::channelstate::ChannelState;
use crate::commands::Command::{ExecuteFirmwareUpdateAction, ExecuteFirmwareUpdateCommand};
use crate::commands::SystemInfoCommand::SupportsDCPCategory;
use crate::commands::{
    Command, FirmwareAction, FirmwareCommand, HardwareInfoCommand, SystemInfoCommand,
};
use crate::dcp::DCPCategory;
use crate::routing::InputDevice;
use anyhow::{bail, Result};
use byteorder::{ByteOrder, LittleEndian, ReadBytesExt, WriteBytesExt};
use enumset::EnumSet;
use goxlr_types::{
    ChannelName, EffectKey, EncoderName, FaderName, FirmwareVersions, MicrophoneParamKey,
    MicrophoneType, Mix, SubMixChannelName, VersionNumber,
};
use log::debug;
use rand::Rng;
use std::io::{Cursor, Write};
use tokio::sync::mpsc::Sender;

// This is a basic SuperTrait which defines all the 'Parts' of the GoXLR for use.
pub trait FullGoXLRDevice: AttachGoXLR + GoXLRCommands + Sync + Send {}

pub trait AttachGoXLR {
    fn from_device(
        device: GoXLRDevice,
        disconnect_sender: Sender<String>,
        event_sender: Sender<String>,
    ) -> Result<Box<dyn FullGoXLRDevice>>
    where
        Self: Sized;

    fn set_unique_identifier(&mut self, identifier: String);
    fn is_connected(&mut self) -> bool;
    fn stop_polling(&mut self);
}

pub trait ExecutableGoXLR {
    fn request_data(&mut self, command: Command, body: &[u8]) -> Result<Vec<u8>> {
        self.perform_request(command, body, false)
    }

    fn perform_request(&mut self, command: Command, body: &[u8], retry: bool) -> Result<Vec<u8>>;
    fn get_descriptor(&self) -> Result<UsbData>;
}

// These are commands that can be executed, but perform_request must be implemented..
pub trait GoXLRCommands: ExecutableGoXLR {
    fn supports_dcp_category(&mut self, category: DCPCategory) -> Result<bool> {
        let mut out = [0; 2];
        LittleEndian::write_u16(&mut out, category.id());
        let result = self.request_data(Command::SystemInfo(SupportsDCPCategory), &out)?;
        Ok(LittleEndian::read_u16(&result) == 1)
    }

    fn get_system_info(&mut self) -> Result<()> {
        let _result =
            self.request_data(Command::SystemInfo(SystemInfoCommand::FirmwareVersion), &[])?;
        // TODO: parse that?
        Ok(())
    }

    fn get_firmware_version(&mut self) -> Result<FirmwareVersions> {
        let result = self.request_data(
            Command::GetHardwareInfo(HardwareInfoCommand::FirmwareVersion),
            &[],
        )?;
        debug!("{:x?}", result);
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

    fn get_serial_number(&mut self) -> Result<(String, String)> {
        let result = self.request_data(
            Command::GetHardwareInfo(HardwareInfoCommand::SerialNumber),
            &[],
        )?;

        let serial_slice = &result[..24];
        let serial_len = serial_slice
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(serial_slice.len());
        let serial_number = String::from_utf8_lossy(&serial_slice[..serial_len]).to_string();

        let date_slice = &result[24..];
        let date_len = date_slice
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(date_slice.len());
        let manufacture_date = String::from_utf8_lossy(&date_slice[..date_len]).to_string();

        Ok((serial_number, manufacture_date))
    }

    fn set_fader(&mut self, fader: FaderName, channel: ChannelName) -> Result<()> {
        // Channel ID, unknown, unknown, unknown
        self.request_data(Command::SetFader(fader), &[channel as u8, 0x00, 0x00, 0x00])?;
        Ok(())
    }

    fn set_volume(&mut self, channel: ChannelName, volume: u8) -> Result<()> {
        self.request_data(Command::SetChannelVolume(channel), &[volume])?;
        Ok(())
    }

    fn set_encoder_value(&mut self, encoder: EncoderName, value: i8) -> Result<()> {
        self.request_data(Command::SetEncoderValue(encoder), &[value as u8])?;
        Ok(())
    }

    fn set_encoder_mode(&mut self, encoder: EncoderName, mode: u8, resolution: u8) -> Result<()> {
        self.request_data(Command::SetEncoderMode(encoder), &[mode, resolution])?;
        Ok(())
    }

    fn set_channel_state(&mut self, channel: ChannelName, state: ChannelState) -> Result<()> {
        self.request_data(Command::SetChannelState(channel), &[state.id()])?;
        Ok(())
    }

    fn set_button_states(&mut self, data: [ButtonStates; 24]) -> Result<()> {
        self.request_data(Command::SetButtonStates(), &data.map(|state| state as u8))?;
        Ok(())
    }

    fn set_button_colours(&mut self, data: [u8; 328]) -> Result<()> {
        self.request_data(Command::SetColourMap(), &data)?;
        Ok(())
    }

    fn set_button_colours_1_3_40(&mut self, data: [u8; 520]) -> Result<()> {
        self.request_data(Command::SetColourMap(), &data)?;
        Ok(())
    }

    fn set_fader_display_mode(
        &mut self,
        fader: FaderName,
        gradient: bool,
        meter: bool,
    ) -> Result<()> {
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

    fn set_fader_scribble(&mut self, fader: FaderName, data: [u8; 1024]) -> Result<()> {
        // Dump it, see what happens..
        self.request_data(Command::SetScribble(fader), &data)?;
        Ok(())
    }

    fn set_routing(&mut self, input_device: InputDevice, data: [u8; 22]) -> Result<()> {
        self.request_data(Command::SetRouting(input_device), &data)?;
        Ok(())
    }

    // Submix Stuff
    fn set_sub_volume(&mut self, channel: SubMixChannelName, volume: u8) -> Result<()> {
        self.request_data(Command::SetSubChannelVolume(channel), &[volume])?;
        Ok(())
    }

    // TODO: Potentially for later, abstract out the 'data' section into a couple of Vec<>s
    fn set_channel_mixes(&mut self, data: [u8; 8]) -> Result<()> {
        self.request_data(Command::SetChannelMixes, &data)?;
        Ok(())
    }

    fn set_monitored_mix(&mut self, mix: Mix) -> Result<()> {
        self.request_data(Command::SetMonitoredMix, &[mix as u8])?;
        Ok(())
    }

    fn set_microphone_gain(&mut self, microphone_type: MicrophoneType, gain: u16) -> Result<()> {
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

    fn get_microphone_level(&mut self) -> Result<u16> {
        let result = self.request_data(Command::GetMicrophoneLevel, &[])?;
        Ok(LittleEndian::read_u16(&result))
    }

    fn set_effect_values(&mut self, effects: &[(EffectKey, i32)]) -> Result<()> {
        let mut data = Vec::with_capacity(effects.len() * 8);
        let mut cursor = Cursor::new(&mut data);
        for (key, value) in effects {
            cursor.write_u32::<LittleEndian>(*key as u32)?;
            cursor.write_i32::<LittleEndian>(*value)?;
        }
        self.request_data(Command::SetEffectParameters, &data)?;

        Ok(())
    }

    fn set_mic_param(&mut self, params: &[(MicrophoneParamKey, [u8; 4])]) -> Result<()> {
        let mut data = Vec::with_capacity(params.len() * 8);
        let mut cursor = Cursor::new(&mut data);
        for (key, value) in params {
            cursor.write_u32::<LittleEndian>(*key as u32)?;
            cursor.write_all(value)?;
        }
        self.request_data(Command::SetMicrophoneParameters, &data)?;

        Ok(())
    }

    fn get_button_states(&mut self) -> Result<CurrentButtonStates> {
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

    // DO NOT EXECUTE ANY OF THESE, SERIOUSLY!
    fn begin_firmware_upload(&mut self) -> Result<()> {
        let result = self.request_data(
            Command::ExecuteFirmwareUpdateCommand(FirmwareCommand::START),
            &[],
        )?;
        let code = LittleEndian::read_u32(&result[0..4]);
        if code != 0 {
            bail!("Invalid Response Received!");
        }
        Ok(())
    }

    fn begin_erase_nvr(&mut self) -> Result<()> {
        let mut header = [0; 8];
        LittleEndian::write_u32(&mut header[0..4], 7);
        LittleEndian::write_u32(&mut header[4..8], 0);

        self.request_data(ExecuteFirmwareUpdateAction(FirmwareAction::ERASE), &header)?;
        Ok(())
    }

    fn poll_erase_nvr(&mut self) -> Result<u8> {
        let mut header = [0; 8];
        LittleEndian::write_u32(&mut header[0..4], 7);
        LittleEndian::write_u32(&mut header[4..8], 0);

        let result = self.request_data(
            Command::ExecuteFirmwareUpdateAction(FirmwareAction::POLL),
            &header,
        )?;
        if result.len() != 1 {
            bail!("Unexpected Result from NVRam Firmware Erase!");
        }
        Ok(result[0])
    }

    fn send_firmware_packet(&mut self, bytes_sent: u64, data: &[u8]) -> Result<()> {
        let mut header = [0; 12];
        LittleEndian::write_u32(&mut header[0..4], 7);
        LittleEndian::write_u64(&mut header[4..], bytes_sent);

        let mut packet = Vec::with_capacity(header.len() + data.len());
        packet.extend_from_slice(&header);
        packet.extend_from_slice(data);

        self.request_data(
            Command::ExecuteFirmwareUpdateAction(FirmwareAction::SEND),
            &packet,
        )?;
        Ok(())
    }

    fn validate_firmware_packet(
        &mut self,
        verified: u32,
        hash: u32,
        remaining: u32,
    ) -> Result<(u32, u32)> {
        let mut packet = [0; 16];
        LittleEndian::write_u32(&mut packet[0..4], 7);
        LittleEndian::write_u32(&mut packet[4..8], verified);
        LittleEndian::write_u32(&mut packet[8..12], hash);
        LittleEndian::write_u32(&mut packet[12..16], remaining);

        let result = self.request_data(
            Command::ExecuteFirmwareUpdateAction(FirmwareAction::VALIDATE),
            &packet,
        )?;

        // Grab the Hash and Count from the result..
        let hash = LittleEndian::read_u32(&result[0..4]);
        let count = LittleEndian::read_u32(&result[4..8]);

        Ok((hash, count))
    }

    fn verify_firmware_status(&mut self) -> Result<()> {
        let result = self.request_data(
            Command::ExecuteFirmwareUpdateCommand(FirmwareCommand::VERIFY),
            &[],
        )?;

        let output = LittleEndian::read_u32(&result);
        if output != 0 {
            bail!("Unexpected Result from Verify: {}", output);
        }
        Ok(())
    }

    fn poll_verify_firmware_status(&mut self) -> Result<(bool, u32, u32)> {
        let result = self.request_data(
            Command::ExecuteFirmwareUpdateCommand(FirmwareCommand::POLL),
            &[],
        )?;

        let mut cursor = Cursor::new(result);
        let op = cursor.read_u32::<LittleEndian>()?;
        let stage = cursor.read_u32::<LittleEndian>()?;
        let state = cursor.read_u32::<LittleEndian>()?;
        let error_code = cursor.read_u32::<LittleEndian>()?;

        let read_total = cursor.read_u32::<LittleEndian>()?;
        let read_done = cursor.read_u32::<LittleEndian>()?;

        if op == 2 && stage == 0 && state == 2 {
            // Verification complete and good..
            return Ok((true, read_total, read_done));
        }

        if op != 3 {
            bail!("Unexpected Command Code, Expected 3 received {}", op);
        }

        if stage != 0 {
            bail!("Failed with Error: {}", error_code);
        }

        if state == 3 {
            bail!("Failing with Error: {}", error_code);
        }

        if state == 1 && error_code == 0 {
            // More reading to be done..
            return Ok((false, read_total, read_done));
        }

        // 3 0 1 0 - 'More Data'
        // 2 0 2 0 - Completed Succesfully
        // 3 0 3 0b - Failure..
        // 3 1 3 0d - Failure?

        /*
           Best Guess:
           u32: Base State, Update (3) / Complete (2)
           u32: Success state, 0 (success), 1 (failure)
           u32: Current State: 1 (More Data), 2 (Data Finished), 3 (Error)
           u32: Current Error: 0 (No Error), X (Error Code, 0x0d = CRC Failure)
        */

        bail!(
            "Unexpected Packet: {} {} {} {}",
            op,
            stage,
            state,
            read_done
        );
    }

    fn finalise_firmware_upload(&mut self) -> Result<()> {
        let result = self.request_data(
            Command::ExecuteFirmwareUpdateCommand(FirmwareCommand::FINALISE),
            &[],
        )?;

        let output = LittleEndian::read_u32(&result);
        if output != 0 {
            bail!("Unexpected Result from Finalise: {}", output);
        }
        Ok(())
    }

    fn poll_finalise_firmware_upload(&mut self) -> Result<(bool, u32, u32)> {
        let result = self.request_data(
            Command::ExecuteFirmwareUpdateCommand(FirmwareCommand::POLL),
            &[],
        )?;

        let mut cursor = Cursor::new(result);
        let op = cursor.read_u32::<LittleEndian>()?;
        let stage = cursor.read_u32::<LittleEndian>()?;
        let state = cursor.read_u32::<LittleEndian>()?;
        let error_code = cursor.read_u32::<LittleEndian>()?;

        let read_total = cursor.read_u32::<LittleEndian>()?;
        let read_done = cursor.read_u32::<LittleEndian>()?;

        if op != 4 {
            if op == 3 {
                if stage == 1 {
                    bail!("Validation Failure, {}", error_code);
                }
            }

            // Something has gone wrong here with the (assumed) validation phase..
            bail!("Invalid Command Response: {}", op);
        }

        if stage != 1 {
            bail!("Unknown Stage: {}", stage);
        }

        if state == 1 {
            return Ok((false, read_total, read_done));
        }

        if state == 2 {
            return Ok((true, read_total, read_done));
        }

        bail!("Unknown Packet: {} {} {} {}", op, stage, state, error_code);

        // Seems to be similar to verify..
        // 4 1 1 0 - More Data..
        // 4 1 2 0 - Complete..

        // 3 1 3 0d on Failure.. Likely to indicate that something failed during verification (3-1)

        /*
        Ok, First integer is likely operation..
        Second Integer is also operation, but stage..
         */
    }

    fn abort_firmware_update(&mut self) -> Result<u32> {
        let result = self.request_data(
            Command::ExecuteFirmwareUpdateCommand(FirmwareCommand::ABORT),
            &[],
        )?;
        let value = LittleEndian::read_u32(&result);
        Ok(value)
    }

    fn reboot_after_firmware_upload(&mut self) -> Result<()> {
        let result = self.request_data(
            Command::ExecuteFirmwareUpdateCommand(FirmwareCommand::REBOOT),
            &[],
        )?;

        let output = LittleEndian::read_u32(&result);
        if output != 0 {
            bail!("Unexpected Result from Reboot: {}", output);
        }
        Ok(())
    }
}

// We primarily need the bus number, and address for comparison..
#[derive(Debug, Clone)]
pub struct GoXLRDevice {
    pub(crate) bus_number: u8,
    pub(crate) address: u8,
    pub(crate) identifier: Option<String>,
}

impl GoXLRDevice {
    pub fn bus_number(&self) -> u8 {
        self.bus_number
    }
    pub fn address(&self) -> u8 {
        self.address
    }

    pub fn identifier(&self) -> &Option<String> {
        &self.identifier
    }
}

pub struct UsbData {
    pub(crate) vendor_id: u16,
    pub(crate) product_id: u16,
    pub(crate) device_version: (u8, u8, u8),
    pub(crate) device_manufacturer: String,
    pub(crate) product_name: String,
}

impl UsbData {
    pub fn vendor_id(&self) -> u16 {
        self.vendor_id
    }
    pub fn product_id(&self) -> u16 {
        self.product_id
    }
    pub fn device_version(&self) -> (u8, u8, u8) {
        self.device_version
    }
    pub fn device_manufacturer(&self) -> String {
        self.device_manufacturer.clone()
    }
    pub fn product_name(&self) -> String {
        self.product_name.clone()
    }
}
