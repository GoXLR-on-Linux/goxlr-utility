mod channels;
mod cli;
mod client;
mod faders;
mod microphone;

use crate::channels::{apply_channel_states, apply_channel_volumes};
use crate::client::Client;
use crate::faders::apply_fader_controls;
use crate::microphone::apply_microphone_controls;
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use cli::Cli;
use goxlr_ipc::Socket;
use goxlr_ipc::{DaemonRequest, DaemonResponse, DeviceType, MixerStatus, UsbProductInformation};
use goxlr_types::{ChannelName, FaderName, InputDevice, MicrophoneType, OutputDevice};
use strum::IntoEnumIterator;
use tokio::net::UnixStream;

#[tokio::main]
async fn main() -> Result<()> {
    let cli: Cli = Cli::parse();
    let stream = UnixStream::connect("/tmp/goxlr.socket")
        .await
        .context("Could not connect to the GoXLR daemon process")?;
    let address = stream
        .peer_addr()
        .context("Could not get the address of the GoXLR daemon process")?;
    let socket: Socket<DaemonResponse, DaemonRequest> = Socket::new(address, stream);
    let mut client = Client::new(socket);
    client.poll_status().await?;

    let serial = if let Some(serial) = &cli.device {
        serial.to_owned()
    } else if client.status().mixers.len() == 1 {
        client.status().mixers.keys().next().unwrap().to_owned()
    } else {
        return Err(anyhow!(
            "Multiple GoXLR devices are connected, please specify which one to control"
        ));
    };

    apply_fader_controls(&cli.faders, &mut client, &serial)
        .await
        .context("Could not apply fader settings")?;

    apply_channel_volumes(&cli.channel_volumes, &mut client, &serial)
        .await
        .context("Could not apply channel volumes")?;

    apply_channel_states(&cli.channel_states, &mut client, &serial)
        .await
        .context("Could not apply channel states")?;

    apply_microphone_controls(&cli.microphone_controls, &mut client, &serial)
        .await
        .context("Could not apply microphone controls")?;

    client.poll_status().await?;
    for mixer in client.status().mixers.values() {
        print_device(mixer);
    }

    Ok(())
}

fn print_device(device: &MixerStatus) {
    println!(
        "Device type: {}",
        match device.hardware.device_type {
            DeviceType::Unknown => "Unknown",
            DeviceType::Full => "GoXLR (Full)",
            DeviceType::Mini => "GoXLR (Mini)",
        }
    );

    print_usb_info(&device.hardware.usb_device);

    print_mixer_info(device);
}

fn print_usb_info(usb: &UsbProductInformation) {
    println!(
        "USB Device version: {}.{}.{}",
        usb.version.0, usb.version.1, usb.version.2
    );
    println!("USB Device manufacturer: {}", usb.manufacturer_name);
    println!("USB Device name: {}", usb.product_name);
    println!("USB Device is claimed by Daemon: {}", usb.is_claimed);
    println!(
        "USB Device has kernel driver attached: {}",
        usb.has_kernel_driver_attached
    );
    println!(
        "USB Address: bus {}, address {}",
        usb.bus_number, usb.address
    );
}

fn print_mixer_info(mixer: &MixerStatus) {
    println!("Mixer firmware: {}", mixer.hardware.versions.firmware);
    println!("Mixer dice: {}", mixer.hardware.versions.dice);
    println!("Mixer FPGA count: {}", mixer.hardware.versions.fpga_count);
    println!("Mixer serial number: {}", mixer.hardware.serial_number);
    println!(
        "Mixer manufacture date: {}",
        mixer.hardware.manufactured_date
    );

    for fader in FaderName::iter() {
        println!(
            "Fader {} assignment: {}",
            fader,
            mixer.get_fader_assignment(fader)
        )
    }

    for channel in ChannelName::iter() {
        let pct = (mixer.get_channel_volume(channel) as f32 / 255.0) * 100.0;
        if mixer.get_channel_muted(channel) {
            println!("{} volume: {:.0}% (Muted)", channel, pct);
        } else {
            println!("{} volume: {:.0}%", channel, pct);
        }
    }

    for microphone in MicrophoneType::iter() {
        if mixer.mic_type == microphone {
            println!(
                "{} mic gain: {} dB (ACTIVE)",
                microphone, mixer.mic_gains[microphone as usize]
            );
        } else {
            println!(
                "{} mic gain: {} dB (Inactive)",
                microphone, mixer.mic_gains[microphone as usize]
            );
        }
    }

    let max_col_len = OutputDevice::iter()
        .map(|s| s.to_string().len())
        .max()
        .unwrap_or_default();
    let mut table_width = max_col_len + 1;
    print!(" {}", " ".repeat(max_col_len));
    for input in InputDevice::iter() {
        let col_name = input.to_string();
        print!(" |{}|", col_name);
        table_width += col_name.len() + 3;
    }
    println!();
    println!("{}", "-".repeat(table_width));

    for output in OutputDevice::iter() {
        let row_name = output.to_string();
        print!("|{}{}|", " ".repeat(max_col_len - row_name.len()), row_name,);
        for input in InputDevice::iter() {
            let col_name = input.to_string();
            if mixer.router[input as usize].contains(output) {
                let len = col_name.len() + 1;
                print!("{}X{} ", " ".repeat(len / 2), " ".repeat(len - (len / 2)));
            } else {
                let len = col_name.len() + 2;
                print!("{} ", " ".repeat(len));
            }
        }
        println!();
    }
    println!("{}", "-".repeat(table_width));
}
