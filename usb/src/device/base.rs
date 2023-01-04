use crate::buttonstate::{ButtonStates, Buttons, CurrentButtonStates};
use crate::channelstate::ChannelState;
use crate::commands::SystemInfoCommand::SupportsDCPCategory;
use crate::commands::{Command, HardwareInfoCommand, SystemInfoCommand};
use crate::dcp::DCPCategory;
use crate::routing::InputDevice;
use anyhow::Result;
use byteorder::{ByteOrder, LittleEndian, ReadBytesExt, WriteBytesExt};
use enumset::EnumSet;
use goxlr_types::{
    ChannelName, EffectKey, EncoderName, FaderName, FirmwareVersions, MicrophoneParamKey,
    MicrophoneType, VersionNumber,
};
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
