mod channels;
mod client;
mod faders;

use crate::channels::{ChannelStates, ChannelVolumes};
use crate::client::Client;
use anyhow::{Context, Result};
use clap::Parser;
use faders::FaderControls;
use goxlr_ipc::{
    DaemonRequest, DaemonResponse, DeviceType, GoXLRCommand, MixerStatus, UsbProductInformation,
};
use goxlr_ipc::{DeviceStatus, Socket};
use goxlr_types::{ChannelName, FaderName};
use strum::IntoEnumIterator;
use tokio::net::UnixStream;

#[derive(Parser, Debug)]
#[clap(about, version, author)]
struct Cli {
    #[clap(flatten, help_heading = "Fader controls")]
    faders: FaderControls,

    #[clap(flatten, help_heading = "Channel volumes")]
    channel_volumes: ChannelVolumes,

    #[clap(flatten, help_heading = "Channel states")]
    channel_states: ChannelStates,
}

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

    cli.faders
        .apply(&mut client)
        .await
        .context("Could not apply fader settings")?;

    cli.channel_volumes
        .apply(&mut client)
        .await
        .context("Could not apply channel volumes")?;

    cli.channel_states
        .apply(&mut client)
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
