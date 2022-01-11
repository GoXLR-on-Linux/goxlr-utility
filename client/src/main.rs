mod channels;
mod cli;
mod client;
mod faders;

use crate::channels::{apply_channel_states, apply_channel_volumes};
use crate::client::Client;
use crate::faders::apply_fader_controls;
use anyhow::{Context, Result};
use clap::Parser;
use cli::Cli;
use goxlr_ipc::{
    DaemonRequest, DaemonResponse, DeviceType, GoXLRCommand, MixerStatus, UsbProductInformation,
};
use goxlr_ipc::{DeviceStatus, Socket};
use goxlr_types::{ChannelName, FaderName};
use strum::IntoEnumIterator;
use tokio::net::UnixStream;

#[tokio::main]
async fn main() -> Result<()> {
    let cli: Cli = Cli::parse();
    let mut stream = UnixStream::connect("/tmp/goxlr.socket")
        .await
        .context("Could not connect to the GoXLR daemon process")?;
    let address = stream
        .peer_addr()
        .context("Could not get the address of the GoXLR daemon process")?;
    let socket: Socket<DaemonResponse, DaemonRequest> = Socket::new(address, &mut stream);
    let mut client = Client::new(socket);

    apply_fader_controls(&cli.faders, &mut client)
        .await
        .context("Could not apply fader settings")?;

    apply_channel_volumes(&cli.channel_volumes, &mut client)
        .await
        .context("Could not apply channel volumes")?;

    apply_channel_states(&cli.channel_states, &mut client)
        .await
        .context("Could not apply channel states")?;

    client
        .send(GoXLRCommand::GetStatus)
        .await
        .context("Could not retrieve device status")?;

    print_device(client.device());

    Ok(())
}

fn print_device(device: &DeviceStatus) {
    println!(
        "Device type: {}",
        match device.device_type {
            DeviceType::Unknown => "Unknown",
            DeviceType::Full => "GoXLR (Full)",
            DeviceType::Mini => "GoXLR (Mini)",
        }
    );

    if let Some(usb) = &device.usb_device {
        print_usb_info(usb);
    }

    if let Some(mixer) = &device.mixer {
        print_mixer_info(mixer);
    }
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
}
