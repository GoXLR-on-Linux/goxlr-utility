mod cli;
mod microphone;

use crate::cli::{
    ButtonGroupLightingCommands, ButtonLightingCommands, CompressorCommands, CoughButtonBehaviours,
    EqualiserCommands, EqualiserMiniCommands, FaderCommands, FaderLightingCommands,
    FadersAllLightingCommands, LightingCommands, MicrophoneCommands, NoiseGateCommands,
    ProfileAction, ProfileType, SubCommands,
};
use crate::microphone::apply_microphone_controls;
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use cli::Cli;
use goxlr_ipc::client::Client;
use goxlr_ipc::{DaemonRequest, DaemonResponse, DeviceType, MixerStatus, UsbProductInformation};
use goxlr_ipc::{GoXLRCommand, Socket};
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

    apply_microphone_controls(&cli.microphone_controls, &mut client, &serial)
        .await
        .context("Could not apply microphone controls")?;

    // These will be moved around later :)
    match &cli.subcommands {
        None => {}
        Some(_) => {}
    }

    match &cli.subcommands {
        None => {}
        Some(command) => {
            match command {
                SubCommands::Microphone { command } => match command {
                    MicrophoneCommands::Equaliser { command } => match command {
                        EqualiserCommands::Frequency { frequency, value } => {
                            client
                                .command(&serial, GoXLRCommand::SetEqFreq(*frequency, *value))
                                .await?;
                        }
                        EqualiserCommands::Gain { frequency, gain } => {
                            client
                                .command(&serial, GoXLRCommand::SetEqGain(*frequency, *gain))
                                .await?;
                        }
                    },
                    MicrophoneCommands::EqualiserMini { command } => match command {
                        EqualiserMiniCommands::Frequency { frequency, value } => {
                            client
                                .command(&serial, GoXLRCommand::SetEqMiniFreq(*frequency, *value))
                                .await?;
                        }
                        EqualiserMiniCommands::Gain { frequency, gain } => {
                            client
                                .command(&serial, GoXLRCommand::SetEqMiniGain(*frequency, *gain))
                                .await?;
                        }
                    },
                    MicrophoneCommands::NoiseGate { command } => match command {
                        NoiseGateCommands::Threshold { value } => {
                            client
                                .command(&serial, GoXLRCommand::SetGateThreshold(*value))
                                .await?;
                        }
                        NoiseGateCommands::Attenuation { value } => {
                            client
                                .command(&serial, GoXLRCommand::SetGateAttenuation(*value))
                                .await?;
                        }
                        NoiseGateCommands::Attack { value } => {
                            client
                                .command(&serial, GoXLRCommand::SetGateAttack(*value))
                                .await?;
                        }
                        NoiseGateCommands::Release { value } => {
                            client
                                .command(&serial, GoXLRCommand::SetGateRelease(*value))
                                .await?;
                        }
                        NoiseGateCommands::Active { enabled } => {
                            client
                                .command(&serial, GoXLRCommand::SetGateActive(*enabled))
                                .await?;
                        }
                    },
                    MicrophoneCommands::Compressor { command } => match command {
                        CompressorCommands::Threshold { value } => {
                            client
                                .command(&serial, GoXLRCommand::SetCompressorThreshold(*value))
                                .await?;
                        }
                        CompressorCommands::Ratio { value } => {
                            client
                                .command(&serial, GoXLRCommand::SetCompressorRatio(*value))
                                .await?;
                        }
                        CompressorCommands::Attack { value } => {
                            client
                                .command(&serial, GoXLRCommand::SetCompressorAttack(*value))
                                .await?;
                        }
                        CompressorCommands::Release { value } => {
                            client
                                .command(&serial, GoXLRCommand::SetCompressorReleaseTime(*value))
                                .await?;
                        }
                        CompressorCommands::MakeUp { value } => {
                            client
                                .command(&serial, GoXLRCommand::SetCompressorMakeupGain(*value))
                                .await?;
                        }
                    },
                    MicrophoneCommands::DeEss { level } => {
                        client
                            .command(&serial, GoXLRCommand::SetDeeser(*level))
                            .await?;
                    }
                },
                SubCommands::Faders { fader } => match fader {
                    FaderCommands::Channel { fader, channel } => {
                        client
                            .command(&serial, GoXLRCommand::SetFader(*fader, *channel))
                            .await?;
                    }
                    FaderCommands::MuteBehaviour {
                        fader,
                        mute_behaviour,
                    } => {
                        client
                            .command(
                                &serial,
                                GoXLRCommand::SetFaderMuteFunction(*fader, *mute_behaviour),
                            )
                            .await?;
                    }
                },
                SubCommands::Router {
                    input,
                    output,
                    enabled,
                } => {
                    client
                        .command(&serial, GoXLRCommand::SetRouter(*input, *output, *enabled))
                        .await?;
                }
                SubCommands::Volume {
                    channel,
                    volume_percent,
                } => {
                    let value = (255 * *volume_percent as u16) / 100;

                    client
                        .command(&serial, GoXLRCommand::SetVolume(*channel, value as u8))
                        .await?;
                }
                SubCommands::CoughButton { command } => match command {
                    CoughButtonBehaviours::ButtonIsHold { is_hold } => {
                        client
                            .command(&serial, GoXLRCommand::SetCoughIsHold(*is_hold))
                            .await?;
                    }
                    CoughButtonBehaviours::MuteBehaviour { mute_behaviour } => {
                        client
                            .command(&serial, GoXLRCommand::SetCoughMuteFunction(*mute_behaviour))
                            .await?;
                    }
                },
                SubCommands::BleepVolume { volume_percent } => {
                    // Ok, this is a value between -34 and 0, with 0 being loudest :D
                    let value = (34 * *volume_percent as u16) / 100;
                    client
                        .command(
                            &serial,
                            GoXLRCommand::SetSwearButtonVolume((value as i8 - 34) as i8),
                        )
                        .await?;
                }

                SubCommands::Lighting { command } => match command {
                    LightingCommands::Fader { command } => match command {
                        FaderLightingCommands::Display { fader, display } => {
                            client
                                .command(
                                    &serial,
                                    GoXLRCommand::SetFaderDisplayStyle(*fader, *display),
                                )
                                .await?;
                        }
                        FaderLightingCommands::Colour { fader, top, bottom } => {
                            client
                                .command(
                                    &serial,
                                    GoXLRCommand::SetFaderColours(
                                        *fader,
                                        top.to_string(),
                                        bottom.to_string(),
                                    ),
                                )
                                .await?;
                        }
                    },
                    LightingCommands::FadersAll { command } => match command {
                        FadersAllLightingCommands::Display { display } => {
                            client
                                .command(&serial, GoXLRCommand::SetAllFaderDisplayStyle(*display))
                                .await?;
                        }
                        FadersAllLightingCommands::Colour { top, bottom } => {
                            client
                                .command(
                                    &serial,
                                    GoXLRCommand::SetAllFaderColours(
                                        top.to_string(),
                                        bottom.to_string(),
                                    ),
                                )
                                .await?;
                        }
                    },
                    LightingCommands::Button { command } => match command {
                        ButtonLightingCommands::Colour {
                            button,
                            colour_one,
                            colour_two,
                        } => {
                            let mut colour_send = None;
                            if let Some(value) = colour_two {
                                colour_send = Some(value.to_string());
                            }

                            client
                                .command(
                                    &serial,
                                    GoXLRCommand::SetButtonColours(
                                        *button,
                                        colour_one.to_string(),
                                        colour_send,
                                    ),
                                )
                                .await?;
                        }
                        ButtonLightingCommands::OffStyle { button, off_style } => {
                            client
                                .command(
                                    &serial,
                                    GoXLRCommand::SetButtonOffStyle(*button, *off_style),
                                )
                                .await?;
                        }
                    },
                    LightingCommands::ButtonGroup { command } => match command {
                        ButtonGroupLightingCommands::Colour {
                            group,
                            colour_one,
                            colour_two,
                        } => {
                            let mut colour_send = None;
                            if let Some(value) = colour_two {
                                colour_send = Some(value.to_string());
                            }

                            client
                                .command(
                                    &serial,
                                    GoXLRCommand::SetButtonGroupColours(
                                        *group,
                                        colour_one.clone(),
                                        colour_send,
                                    ),
                                )
                                .await?;
                        }
                        ButtonGroupLightingCommands::OffStyle { group, off_style } => {
                            client
                                .command(
                                    &serial,
                                    GoXLRCommand::SetButtonGroupOffStyle(*group, *off_style),
                                )
                                .await?;
                        }
                    },
                },

                SubCommands::Profiles { command } => match command {
                    ProfileType::Device { command } => match command {
                        ProfileAction::New { profile_name } => {
                            client
                                .command(
                                    &serial,
                                    GoXLRCommand::NewProfile(profile_name.to_string()),
                                )
                                .await
                                .context("Unable to create new profile")?;
                        }
                        ProfileAction::Load { profile_name } => {
                            client
                                .command(
                                    &serial,
                                    GoXLRCommand::LoadProfile(profile_name.to_string()),
                                )
                                .await
                                .context("Unable to Load Profile")?;
                        }
                        ProfileAction::Save {} => {
                            client
                                .command(&serial, GoXLRCommand::SaveProfile())
                                .await
                                .context("Unable to Save Profile")?;
                        }
                        ProfileAction::SaveAs { profile_name } => {
                            client
                                .command(
                                    &serial,
                                    GoXLRCommand::SaveProfileAs(profile_name.to_string()),
                                )
                                .await
                                .context("Unable to Save Profile")?;
                        }
                    },
                    ProfileType::Microphone { command } => match command {
                        ProfileAction::New { profile_name } => {
                            client
                                .command(
                                    &serial,
                                    GoXLRCommand::NewMicProfile(profile_name.to_string()),
                                )
                                .await
                                .context("Unable to create new profile")?;
                        }
                        ProfileAction::Load { profile_name } => {
                            client
                                .command(
                                    &serial,
                                    GoXLRCommand::LoadMicProfile(profile_name.to_string()),
                                )
                                .await
                                .context("Unable to Load Microphone Profile")?;
                        }
                        ProfileAction::Save {} => {
                            client
                                .command(&serial, GoXLRCommand::SaveMicProfile())
                                .await
                                .context("Unable to Save Microphone Profile")?;
                        }
                        ProfileAction::SaveAs { profile_name } => {
                            client
                                .command(
                                    &serial,
                                    GoXLRCommand::SaveMicProfileAs(profile_name.to_string()),
                                )
                                .await
                                .context("Unable to Save Microphone Profile")?;
                        }
                    },
                },
            }
        }
    }

    if cli.status_json {
        client.poll_status().await?;
        println!("{}", serde_json::to_string_pretty(client.status())?);
    }

    if cli.status {
        client.poll_status().await?;
        println!(
            "Profile directory: {}",
            client.status().paths.profile_directory.to_string_lossy()
        );
        println!(
            "Mic Profile directory: {}",
            client
                .status()
                .paths
                .mic_profile_directory
                .to_string_lossy()
        );
        println!(
            "Samples directory: {}",
            client.status().paths.samples_directory.to_string_lossy()
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
            mixer.get_fader_status(fader).channel,
            mixer.get_fader_status(fader).mute_type
        )
    }

    for channel in ChannelName::iter() {
        let pct = (mixer.get_channel_volume(channel) as f32 / 255.0) * 100.0;
        println!("{} volume: {:.0}%", channel, pct);
    }

    for microphone in MicrophoneType::iter() {
        if mixer.mic_status.mic_type == microphone {
            println!(
                "{} mic gain: {} dB (ACTIVE)",
                microphone, mixer.mic_status.mic_gains[microphone as usize]
            );
        } else {
            println!(
                "{} mic gain: {} dB (Inactive)",
                microphone, mixer.mic_status.mic_gains[microphone as usize]
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
