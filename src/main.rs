mod channels;
mod commands;
mod dcp;
mod error;
mod faders;
mod goxlr;

use crate::channels::Channel;
use crate::commands::{Command, SystemInfoCommand};
use crate::faders::Fader;
use crate::goxlr::GoXLR;
use rusb::{LogLevel, RequestType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    rusb::set_log_level(LogLevel::Debug);
    let mut goxlr = GoXLR::open()?;

    println!(
        "{:X?}",
        goxlr.read_control(RequestType::Vendor, 0, 0, 0, 24)?
    ); // ??
       /* Expected output:
       0000   73 19 06 04 66 19 10 18 02 00 00 00 01 00 00 00
       0010   00 04 00 00 00 00 00 00
            */

    println!(
        "{:X?}",
        goxlr.write_control(RequestType::Vendor, 1, 0, 0, &[])?
    ); // ??
    println!(
        "{:X?}",
        goxlr.read_control(RequestType::Vendor, 3, 0, 0, 1040)?
    ); // ??
       /* Expected output:
       0000   00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
            */

    println!(
        "{:02X?}",
        goxlr.request_data(Command::SystemInfo(SystemInfoCommand::FirmwareVersion), &[])?
    );

    goxlr.set_volume(Channel::Mic, 0xFF)?;
    goxlr.set_volume(Channel::Chat, 0xFF)?;
    goxlr.set_volume(Channel::Music, 0xFF)?;
    goxlr.set_volume(Channel::System, 0xFF)?;

    goxlr.set_fader(Fader::A, Channel::Mic)?;
    goxlr.set_fader(Fader::B, Channel::Chat)?;
    goxlr.set_fader(Fader::C, Channel::Music)?;
    goxlr.set_fader(Fader::D, Channel::System)?;

    Ok(())
}
