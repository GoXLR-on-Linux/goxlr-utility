mod channels;
mod cli;
mod client;
mod faders;
mod microphone;

use crate::channels::apply_channel_volumes;
use crate::client::Client;
use crate::faders::apply_fader_controls;
use crate::microphone::apply_microphone_controls;
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use cli::Cli;
use goxlr_ipc::{DaemonRequest, DaemonResponse, DeviceType, MixerStatus, UsbProductInformation};
use goxlr_ipc::{GoXLRCommand, Socket};
use goxlr_types::{ChannelName, FaderName, InputDevice, MicrophoneType, OutputDevice};
use strum::IntoEnumIterator;
use tokio::net::UnixStream;
use crate::cli::{AllFaderCommands, CoughCommands, FaderCommands, SubCommands};

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
        for mixer in client.status().mixers.values() {
            println!(
                "{} - {} on bus {}, address {}",
                mixer.hardware.serial_number,
                match mixer.hardware.device_type {
                    DeviceType::Unknown => "Unknown device",
                    DeviceType::Full => "Regular GoXLR",
                    DeviceType::Mini => "Mini GoXLR",
                },
                mixer.hardware.usb_device.bus_number,
                mixer.hardware.usb_device.address
            );
        }
        return Err(anyhow!(
            "Multiple GoXLR devices are connected, please specify which one to control"
        ));
    };

    if let Some(profile) = &cli.profile.load_profile {
        client
            .command(&serial, GoXLRCommand::LoadProfile(profile.to_string()))
            .await
            .context("Couldn't load the specified profile")?;
    }

    if let Some(profile) = &cli.profile.load_mic_profile {
        client.
            command(&serial, GoXLRCommand::LoadMicProfile(profile.to_string()))
            .await
            .context("Couldn't load Mic Profile")?;
    }

    if cli.profile.save_profile {
        client.command(&serial, GoXLRCommand::SaveProfile())
            .await
            .context("Unable to save GoXLR Profile")?;
    }

    if cli.profile.save_mic_profile {
        client.command(&serial, GoXLRCommand::SaveMicProfile())
            .await
            .context("Unable to save Microphone Profile")?;
    }

    apply_fader_controls(&cli.faders, &mut client, &serial)
        .await
        .context("Could not apply fader settings")?;

    apply_channel_volumes(&cli.channel_volumes, &mut client, &serial)
        .await
        .context("Could not apply channel volumes")?;

    apply_microphone_controls(&cli.microphone_controls, &mut client, &serial)
        .await
        .context("Could not apply microphone controls")?;

    // These will be moved around later :)
    match &cli.subcommands {
        None => {}
        Some(_) => {
            match &cli.subcommands.unwrap() {
                SubCommands::Faders { fader } => {
                    match fader {
                        None => {}
                        Some(_) => {
                            match fader.as_ref().unwrap() {
                                FaderCommands::Channel { fader, channel } => {
                                    if let Some(value) = channel {
                                        client.command(&serial, GoXLRCommand::SetFader(
                                            *fader,
                                            *value
                                        )).await?;
                                    } else {
                                        println!("Channel Getter Not Implemented");
                                    }

                                }
                                FaderCommands::MuteBehaviour { fader, mute_behaviour } => {
                                    if let Some(value) = mute_behaviour {
                                        client.command(&serial, GoXLRCommand::SetFaderMuteFunction(
                                            *fader,
                                            *value
                                        )).await?;
                                    } else {
                                        println!("Mute behaviour Getter not Implemented");
                                    }
                                }
                                FaderCommands::Display { fader, display } => {
                                    if let Some(value) = display {
                                        client.command(&serial, GoXLRCommand::SetFaderDisplay(
                                            *fader,
                                            *value
                                        )).await?;
                                    } else {
                                        println!("Display Getter Not Implemented");
                                    }
                                }
                                FaderCommands::Colour {fader, top, bottom} => {
                                    // TODO: Error check this, should be RGBA
                                    client.command(&serial, GoXLRCommand::SetFaderColours(
                                        *fader,
                                        top.to_string(),
                                        bottom.to_string()
                                    )).await?;
                                }
                                FaderCommands::ButtonColour {fader, colour_one, off_style, colour_two} => {
                                    let mut colour_send = None;
                                    if let Some(value) = colour_two {
                                        colour_send = Some(value.to_string());
                                    }

                                    // We might end up moving colour setting to elsewhere, for now we'll do it here..
                                    client.command(&serial, GoXLRCommand::SetFaderButtonColours(
                                        *fader,
                                        colour_one.to_string(),
                                        *off_style,
                                        colour_send,
                                    )).await?;
                                }
                            }
                        }
                    }
                }
                SubCommands::FadersAll { command } => {
                    match command {
                        None => {}
                        Some(_) => {
                            match command.as_ref().unwrap() {
                                AllFaderCommands::Colour { top, bottom } => {
                                    client.command(&serial, GoXLRCommand::SetAllFaderColours(
                                        top.to_string(),
                                        bottom.to_string()
                                    )).await?;
                                },
                                AllFaderCommands::Display {display} => {
                                    // This doesn't need a 'special' command for mass apply, as the display
                                    // is set directly for each fader.
                                    for fader in FaderName::iter() {
                                        client.command(&serial, GoXLRCommand::SetFaderDisplay(
                                            fader,
                                            *display
                                        )).await?;
                                    }
                                },
                                AllFaderCommands::ButtonColour {colour_one, off_style, colour_two} => {
                                    let mut colour_send = None;
                                    if let Some(value) = colour_two {
                                        colour_send = Some(value.to_string());
                                    }

                                    // We might end up moving colour setting to elsewhere, for now we'll do it here..
                                    client.command(&serial, GoXLRCommand::SetAllFaderButtonColours(
                                        colour_one.to_string(),
                                        *off_style,
                                        colour_send,
                                    )).await?;
                                }
                            }
                        }
                    }
                }
                SubCommands::Router { input, output, enabled } => {
                    if let Some(value) = enabled {
                        client.command(&serial, GoXLRCommand::SetRouter(
                            *input,
                            *output,
                            *value
                        )).await?;
                    } else {
                        println!("Router Getter Not Implemented Yet");
                    }
                }
                SubCommands::Volume { channel, volume_percent } => {
                    if let Some(value) = volume_percent {
                        // Convert the percentage to a 'correct' value..
                        let value = (255 * *value as u16) / 100;

                        client.command(&serial, GoXLRCommand::SetVolume(
                            *channel,
                            value as u8
                        )).await?;
                    } else {
                        println!("Volume Getter Not Implemented Yet");
                    }
                }
                SubCommands::Cough { command } => {
                    match Some(command) {
                        None => {}
                        Some(_) => {
                            match command.as_ref().unwrap() {
                                CoughCommands::MuteBehaviour { mute_behaviour } => {
                                    if let Some(value) = mute_behaviour {
                                        client.command(&serial, GoXLRCommand::SetCoughMuteFunction(
                                            *value
                                        )).await?;
                                    } else {
                                        println!("Mute behaviour Getter not Implemented");
                                    }
                                }
                                CoughCommands::Colour { colour_one, off_style, colour_two } => {
                                    // Might be a cleaner way to do this?
                                    let mut colour_send = None;
                                    if let Some(value) = colour_two {
                                        colour_send = Some(value.to_string());
                                    }

                                    // We might end up moving colour setting to elsewhere, for now we'll do it here..
                                    client.command(&serial, GoXLRCommand::SetCoughColourConfiguration(
                                        colour_one.to_string(),
                                        *off_style,
                                        colour_send,
                                    )).await?;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if cli.status {
        client.poll_status().await?;
        println!(
            "Profile directory: {}",
            client.status().profile_directory.to_string_lossy()
        );
        for mixer in client.status().mixers.values() {
            print_device(mixer);
        }
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
    println!("Mixer profile: {}", mixer.profile_name);

    for fader in FaderName::iter() {
        println!(
            "Fader {} assignment: {}, Mute Behaviour: {}",
            fader,
            mixer.get_fader_assignment(fader).channel,
            mixer.get_fader_assignment(fader).mute_type
        )
    }

    for channel in ChannelName::iter() {
        let pct = (mixer.get_channel_volume(channel) as f32 / 255.0) * 100.0;
        println!("{} volume: {:.0}%", channel, pct);
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
