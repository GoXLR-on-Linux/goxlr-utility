use anyhow::{bail, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use goxlr_ipc::FirmwareInfo;
use goxlr_types::{DeviceType, VersionNumber};
use std::io;
use std::io::Cursor;
use std::path::PathBuf;

pub fn check_firmware(path: &PathBuf) -> Result<FirmwareInfo> {
    load_firmware_file(path)
}

fn load_firmware_file(file: &PathBuf) -> Result<FirmwareInfo> {
    if let Ok(firmware) = std::fs::read(file) {
        // I'm going to assume that if the firmware is < 64 bytes, it doesn't contain the
        // full firmware header.
        if firmware.len() < 64 {
            bail!("Invalid GoXLR Firmware File");
        }

        // Is this a Mini, or a full?
        let device_name = get_firmware_name(&firmware[0..16]);
        let device_type = if device_name == "GoXLR Firmware" {
            DeviceType::Full
        } else if device_name == "GoXLR-Mini" {
            DeviceType::Mini
        } else {
            bail!("Unknown Device in Firmware Headers");
        };

        // Next, grab the version for this firmware..
        let device_version = if let Ok(version) = get_firmware_version(&firmware[24..32]) {
            version
        } else {
            bail!("Unable to extract Firmware Version");
        };

        Ok(FirmwareInfo {
            path: file.clone(),
            device_type,
            version: device_version,
        })
    } else {
        bail!("Unable to open Firmware File");
    }
}

fn get_firmware_name(src: &[u8]) -> String {
    let mut end_index = 0;
    for byte in src {
        if *byte == 0x00 {
            break;
        }
        end_index += 1;
    }
    String::from_utf8_lossy(&src[0..end_index]).to_string()
}

fn get_firmware_version(src: &[u8]) -> Result<VersionNumber, io::Error> {
    // Unpack the firmware version..
    let mut cursor = Cursor::new(src);
    let firmware_packed = cursor.read_u32::<LittleEndian>()?;
    let firmware_build = cursor.read_u32::<LittleEndian>()?;
    let firmware = VersionNumber(
        firmware_packed >> 12,
        (firmware_packed >> 8) & 0xF,
        Some(firmware_packed & 0xFF),
        Some(firmware_build),
    );

    Ok(firmware)
}
