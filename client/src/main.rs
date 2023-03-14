mod cli;
mod microphone;

use crate::cli::{
    ButtonGroupLightingCommands, ButtonLightingCommands, CompressorCommands, CoughButtonBehaviours,
    Echo, EffectsCommands, EqualiserCommands, EqualiserMiniCommands, FaderCommands,
    FaderLightingCommands, FadersAllLightingCommands, Gender, HardTune, LightingCommands,
    Megaphone, MicrophoneCommands, NoiseGateCommands, Pitch, ProfileAction, ProfileType, Reverb,
    Robot, SamplerCommands, Scribbles, SubCommands,
};
use crate::microphone::apply_microphone_controls;
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use cli::Cli;
use goxlr_ipc::client::Client;
use goxlr_ipc::clients::ipc::ipc_client::IPCClient;
use goxlr_ipc::clients::ipc::ipc_socket::Socket;
use goxlr_ipc::clients::web::web_client::WebClient;
use goxlr_ipc::GoXLRCommand;
use goxlr_ipc::{DaemonRequest, DaemonResponse, DeviceType, MixerStatus, UsbProductInformation};
use goxlr_types::{ChannelName, FaderName, InputDevice, MicrophoneType, OutputDevice};
use interprocess::local_socket::tokio::LocalSocketStream;
use interprocess::local_socket::NameTypeSupport;
use strum::IntoEnumIterator;

static SOCKET_PATH: &str = "/tmp/goxlr.socket";
static NAMED_PIPE: &str = "@goxlr.socket";

#[tokio::main]
async fn main() -> Result<()> {
    let cli: Cli = Cli::parse();

    let mut client: Box<dyn Client>;

    if let Some(url) = cli.use_http {
        client = Box::new(WebClient::new(format!("{}/api/command", url)));
    } else {
        let connection = LocalSocketStream::connect(match NameTypeSupport::query() {
            NameTypeSupport::OnlyPaths | NameTypeSupport::Both => SOCKET_PATH,
            NameTypeSupport::OnlyNamespaced => NAMED_PIPE,
        })
        .await
        .context("Unable to connect to the GoXLR daemon Process")?;

        let socket: Socket<DaemonResponse, DaemonRequest> = Socket::new(connection);
        client = Box::new(IPCClient::new(socket));
    }

    client.poll_status().await?;
    client.poll_http_status().await?;

    let serial = if let Some(serial) = &cli.device {
        serial.to_owned()
    } else if client.status().mixers.is_empty() {
        return Err(anyhow!("No GoXLR Devices are Connected."));
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
                    FaderCommands::MuteState { fader, state } => {
                        client
                            .command(&serial, GoXLRCommand::SetFaderMuteState(*fader, *state))
                            .await?;
                    }
                    FaderCommands::Scribbles { command } => match command {
                        Scribbles::Icon { fader, name } => {
                            client
                                .command(
                                    &serial,
                                    GoXLRCommand::SetScribbleIcon(*fader, name.clone()),
                                )
                                .await?;
                        }
                        Scribbles::Text { fader, text } => {
                            client
                                .command(
                                    &serial,
                                    GoXLRCommand::SetScribbleText(*fader, text.clone()),
                                )
                                .await?;
                        }
                        Scribbles::Number { fader, text } => {
                            client
                                .command(
                                    &serial,
                                    GoXLRCommand::SetScribbleNumber(*fader, text.clone()),
                                )
                                .await?;
                        }
                        Scribbles::Invert { fader, inverted } => {
                            client
                                .command(
                                    &serial,
                                    GoXLRCommand::SetScribbleInvert(*fader, *inverted),
                                )
                                .await?;
                        }
                    },
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
                            GoXLRCommand::SetSwearButtonVolume(value as i8 - 34),
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
                    LightingCommands::SimpleColour { target, colour } => {
                        client
                            .command(
                                &serial,
                                GoXLRCommand::SetSimpleColour(*target, colour.clone()),
                            )
                            .await?;
                    }
                    LightingCommands::EncoderColour {
                        target,
                        colour_one,
                        colour_two,
                        colour_three,
                    } => {
                        client
                            .command(
                                &serial,
                                GoXLRCommand::SetEncoderColour(
                                    *target,
                                    colour_one.clone(),
                                    colour_two.clone(),
                                    colour_three.clone(),
                                ),
                            )
                            .await?;
                    }
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
                        ProfileAction::LoadColours { profile_name } => {
                            client
                                .command(
                                    &serial,
                                    GoXLRCommand::LoadProfileColours(profile_name.to_string()),
                                )
                                .await
                                .context("Unable to load Profile Colours")?;
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
                        ProfileAction::LoadColours { .. } => {
                            return Err(anyhow!("Not supported for Microphone"));
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
                SubCommands::Effects { command } => match command {
                    EffectsCommands::LoadEffectPreset { name } => {
                        client
                            .command(&serial, GoXLRCommand::LoadEffectPreset(name.to_string()))
                            .await
                            .context("Unable to Load Preset")?;
                    }

                    EffectsCommands::SetActivePreset { preset } => {
                        client
                            .command(&serial, GoXLRCommand::SetActiveEffectPreset(*preset))
                            .await
                            .context("Unable to set the Active Preset")?;
                    }

                    EffectsCommands::RenameActivePreset { name } => {
                        client
                            .command(&serial, GoXLRCommand::RenameActivePreset(name.to_string()))
                            .await
                            .context("Unable to Rename Preset")?;
                    }

                    EffectsCommands::SaveActivePreset => {
                        client
                            .command(&serial, GoXLRCommand::SaveActivePreset())
                            .await
                            .context("Unable to Save Preset")?;
                    }

                    EffectsCommands::Reverb { command } => match command {
                        Reverb::Style { style } => {
                            client
                                .command(&serial, GoXLRCommand::SetReverbStyle(*style))
                                .await
                                .context("Unable to Set Reverb Style")?;
                        }
                        Reverb::Amount { amount } => {
                            client
                                .command(&serial, GoXLRCommand::SetReverbAmount(*amount))
                                .await
                                .context("Unable to Set Reverb Amount")?;
                        }
                        Reverb::Decay { decay } => {
                            client
                                .command(&serial, GoXLRCommand::SetReverbDecay(*decay))
                                .await
                                .context("Unable to Set Reverb Amount")?;
                        }
                        Reverb::EarlyLevel { level } => {
                            client
                                .command(&serial, GoXLRCommand::SetReverbEarlyLevel(*level))
                                .await
                                .context("Unable to Set Reverb Early Level")?;
                        }
                        Reverb::TailLevel { level } => {
                            client
                                .command(&serial, GoXLRCommand::SetReverbTailLevel(*level))
                                .await
                                .context("Unable to Set Reverb Tail Level")?;
                        }
                        Reverb::PreDelay { delay } => {
                            client
                                .command(&serial, GoXLRCommand::SetReverbPreDelay(*delay))
                                .await
                                .context("Unable to Set Reverb Delay")?;
                        }
                        Reverb::LowColour { colour } => {
                            client
                                .command(&serial, GoXLRCommand::SetReverbLowColour(*colour))
                                .await
                                .context("Unable to Set Reverb Low Colour")?;
                        }
                        Reverb::HighColour { colour } => {
                            client
                                .command(&serial, GoXLRCommand::SetReverbHighColour(*colour))
                                .await
                                .context("Unable to Set Reverb High Colour")?;
                        }
                        Reverb::HighFactor { factor } => {
                            client
                                .command(&serial, GoXLRCommand::SetReverbHighFactor(*factor))
                                .await
                                .context("Unable to Set Reverb High Factor")?;
                        }
                        Reverb::Diffuse { diffuse } => {
                            client
                                .command(&serial, GoXLRCommand::SetReverbDiffuse(*diffuse))
                                .await
                                .context("Unable to Set Reverb Diffuse")?;
                        }
                        Reverb::ModSpeed { speed } => {
                            client
                                .command(&serial, GoXLRCommand::SetReverbModSpeed(*speed))
                                .await
                                .context("Unable to Set Reverb Mod Speed")?;
                        }
                        Reverb::ModDepth { depth } => {
                            client
                                .command(&serial, GoXLRCommand::SetReverbModDepth(*depth))
                                .await
                                .context("Unable to Set Reverb Mod Depth")?;
                        }
                    },
                    EffectsCommands::Echo { command } => match command {
                        Echo::Style { style } => {
                            client
                                .command(&serial, GoXLRCommand::SetEchoStyle(*style))
                                .await
                                .context("Unable to Set Echo Style")?;
                        }
                        Echo::Amount { amount } => {
                            client
                                .command(&serial, GoXLRCommand::SetEchoAmount(*amount))
                                .await
                                .context("Unable to Set Echo Amount")?;
                        }
                        Echo::Feedback { feedback } => {
                            client
                                .command(&serial, GoXLRCommand::SetEchoFeedback(*feedback))
                                .await
                                .context("Unable to Set Echo Feedback")?;
                        }
                        Echo::Tempo { tempo } => {
                            client
                                .command(&serial, GoXLRCommand::SetEchoTempo(*tempo))
                                .await
                                .context("Unable to Set Echo Tempo")?;
                        }
                        Echo::DelayLeft { delay } => {
                            client
                                .command(&serial, GoXLRCommand::SetEchoDelayLeft(*delay))
                                .await
                                .context("Unable to Set Echo Delay Left")?;
                        }
                        Echo::DelayRight { delay } => {
                            client
                                .command(&serial, GoXLRCommand::SetEchoDelayRight(*delay))
                                .await
                                .context("Unable to Set Echo Delay Right")?;
                        }
                        Echo::FeedbackXFBLtoR { feedback } => {
                            client
                                .command(&serial, GoXLRCommand::SetEchoFeedbackXFBLtoR(*feedback))
                                .await
                                .context("Unable to Set Echo Feedback XFB L to R")?;
                        }
                        Echo::FeedbackXFBRtoL { feedback } => {
                            client
                                .command(&serial, GoXLRCommand::SetEchoFeedbackXFBRtoL(*feedback))
                                .await
                                .context("Unable to Set Echo Feedback XFB R to L")?;
                        }
                    },
                    EffectsCommands::Pitch { command } => match command {
                        Pitch::Style { style } => {
                            client
                                .command(&serial, GoXLRCommand::SetPitchStyle(*style))
                                .await
                                .context("Unable to Set Pitch Style")?;
                        }
                        Pitch::Amount { amount } => {
                            client
                                .command(&serial, GoXLRCommand::SetPitchAmount(*amount))
                                .await
                                .context("Unable to Set Pitch Amount")?;
                        }
                        Pitch::Character { character } => {
                            client
                                .command(&serial, GoXLRCommand::SetPitchCharacter(*character))
                                .await
                                .context("Unable to Set Pitch Character")?;
                        }
                    },
                    EffectsCommands::Gender { command } => match command {
                        Gender::Style { style } => {
                            client
                                .command(&serial, GoXLRCommand::SetGenderStyle(*style))
                                .await
                                .context("Unable to Set Gender Style")?;
                        }
                        Gender::Amount { amount } => {
                            client
                                .command(&serial, GoXLRCommand::SetGenderAmount(*amount))
                                .await
                                .context("Unable to Set Gender Amount")?;
                        }
                    },
                    EffectsCommands::Megaphone { command } => match command {
                        Megaphone::Style { style } => {
                            client
                                .command(&serial, GoXLRCommand::SetMegaphoneStyle(*style))
                                .await
                                .context("Unable to Set Megaphone Style")?;
                        }
                        Megaphone::Amount { amount } => {
                            client
                                .command(&serial, GoXLRCommand::SetMegaphoneAmount(*amount))
                                .await
                                .context("Unable to Set Megaphone Amount")?;
                        }
                        Megaphone::PostGain { gain } => {
                            client
                                .command(&serial, GoXLRCommand::SetMegaphonePostGain(*gain))
                                .await
                                .context("Unable to Set Megaphone Post-Gain")?;
                        }
                        Megaphone::Enabled { enabled } => {
                            client
                                .command(&serial, GoXLRCommand::SetMegaphoneEnabled(*enabled))
                                .await?;
                        }
                    },
                    EffectsCommands::Robot { command } => match command {
                        Robot::Style { style } => {
                            client
                                .command(&serial, GoXLRCommand::SetRobotStyle(*style))
                                .await
                                .context("Unable to set Robot Style")?;
                        }
                        Robot::Gain { range, gain } => {
                            client
                                .command(&serial, GoXLRCommand::SetRobotGain(*range, *gain))
                                .await
                                .context("Unable to set Robot Gain")?;
                        }
                        Robot::Frequency { range, frequency } => {
                            client
                                .command(&serial, GoXLRCommand::SetRobotFreq(*range, *frequency))
                                .await
                                .context("Unable to set Robot Frequency")?;
                        }
                        Robot::Bandwidth { range, bandwidth } => {
                            client
                                .command(&serial, GoXLRCommand::SetRobotWidth(*range, *bandwidth))
                                .await
                                .context("Unable to set Robot Bandwidth")?;
                        }
                        Robot::WaveForm { waveform } => {
                            client
                                .command(&serial, GoXLRCommand::SetRobotWaveform(*waveform))
                                .await
                                .context("Unable to set Robot Wave Form")?;
                        }
                        Robot::PulseWidth { width } => {
                            client
                                .command(&serial, GoXLRCommand::SetRobotPulseWidth(*width))
                                .await
                                .context("Unable to set Robot Pulse Width")?;
                        }
                        Robot::Threshold { threshold } => {
                            client
                                .command(&serial, GoXLRCommand::SetRobotThreshold(*threshold))
                                .await
                                .context("Unable to set Robot Threshold")?;
                        }
                        Robot::DryMix { dry_mix } => {
                            client
                                .command(&serial, GoXLRCommand::SetRobotDryMix(*dry_mix))
                                .await
                                .context("Unable to set Robot Dry Mix")?;
                        }
                        Robot::Enabled { enabled } => {
                            client
                                .command(&serial, GoXLRCommand::SetRobotEnabled(*enabled))
                                .await?;
                        }
                    },
                    EffectsCommands::HardTune { command } => match command {
                        HardTune::Style { style } => {
                            client
                                .command(&serial, GoXLRCommand::SetHardTuneStyle(*style))
                                .await
                                .context("Unable to set HardTune Style")?;
                        }
                        HardTune::Amount { amount } => {
                            client
                                .command(&serial, GoXLRCommand::SetHardTuneAmount(*amount))
                                .await
                                .context("Unable to set HardTune Amount")?;
                        }
                        HardTune::Rate { rate } => {
                            client
                                .command(&serial, GoXLRCommand::SetHardTuneRate(*rate))
                                .await
                                .context("Unable to set HardTune Rate")?;
                        }
                        HardTune::Window { window } => {
                            client
                                .command(&serial, GoXLRCommand::SetHardTuneWindow(*window))
                                .await
                                .context("Unable to set HardTune Window")?;
                        }
                        HardTune::Source { source } => {
                            client
                                .command(&serial, GoXLRCommand::SetHardTuneSource(*source))
                                .await
                                .context("Unable to set HardTune Source")?;
                        }
                        HardTune::Enabled { enabled } => {
                            client
                                .command(&serial, GoXLRCommand::SetHardTuneEnabled(*enabled))
                                .await?;
                        }
                    },
                    EffectsCommands::Enabled { enabled } => {
                        client
                            .command(&serial, GoXLRCommand::SetFXEnabled(*enabled))
                            .await?;
                    }
                },
                SubCommands::Sampler { command } => match command {
                    SamplerCommands::Add { bank, button, file } => {
                        client
                            .command(
                                &serial,
                                GoXLRCommand::AddSample(*bank, *button, file.clone()),
                            )
                            .await
                            .context("Unable to add Sample File")?;
                    }
                    SamplerCommands::RemoveByIndex {
                        bank,
                        button,
                        index,
                    } => {
                        client
                            .command(
                                &serial,
                                GoXLRCommand::RemoveSampleByIndex(*bank, *button, *index),
                            )
                            .await
                            .context("Unable to Remove Sample")?;
                    }
                    SamplerCommands::PlayByIndex {
                        bank,
                        button,
                        index,
                    } => {
                        client
                            .command(
                                &serial,
                                GoXLRCommand::PlaySampleByIndex(*bank, *button, *index),
                            )
                            .await
                            .context("Unable to Play Sample")?;
                    }
                    SamplerCommands::PlayNextTrack { bank, button } => {
                        client
                            .command(&serial, GoXLRCommand::PlayNextSample(*bank, *button))
                            .await?;
                    }
                    SamplerCommands::StopPlayback { bank, button } => {
                        client
                            .command(&serial, GoXLRCommand::StopSamplePlayback(*bank, *button))
                            .await
                            .context("Unable to Stop Sample Playback")?;
                    }
                    SamplerCommands::PlaybackMode { bank, button, mode } => {
                        client
                            .command(
                                &serial,
                                GoXLRCommand::SetSamplerFunction(*bank, *button, *mode),
                            )
                            .await
                            .context("Unable to set Playback Mode")?;
                    }
                    SamplerCommands::PlaybackOrder {
                        bank,
                        button,
                        mode: order,
                    } => {
                        client
                            .command(
                                &serial,
                                GoXLRCommand::SetSamplerOrder(*bank, *button, *order),
                            )
                            .await
                            .context("Unable to set Play Order")?;
                    }
                    SamplerCommands::StartPercent {
                        bank,
                        button,
                        sample_id,
                        start_position,
                    } => {
                        client
                            .command(
                                &serial,
                                GoXLRCommand::SetSampleStartPercent(
                                    *bank,
                                    *button,
                                    *sample_id,
                                    *start_position,
                                ),
                            )
                            .await
                            .context("Unable to set Start Percent")?;
                    }
                    SamplerCommands::StopPercent {
                        bank,
                        button,
                        sample_id,
                        stop_position,
                    } => {
                        client
                            .command(
                                &serial,
                                GoXLRCommand::SetSampleStopPercent(
                                    *bank,
                                    *button,
                                    *sample_id,
                                    *stop_position,
                                ),
                            )
                            .await
                            .context("Unable to set Stop Percent")?;
                    }
                },
            }
        }
    }

    if cli.status_json {
        client.poll_status().await?;
        println!("{}", serde_json::to_string_pretty(client.status())?);
    }

    if cli.status_http {
        client.poll_http_status().await?;
        println!("{}", serde_json::to_string_pretty(client.http_status())?);
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
        println!("{channel} volume: {pct:.0}%");
    }

    for microphone in MicrophoneType::iter() {
        if mixer.mic_status.mic_type == microphone {
            println!(
                "{} mic gain: {} dB (ACTIVE)",
                microphone, mixer.mic_status.mic_gains[microphone]
            );
        } else {
            println!(
                "{} mic gain: {} dB (Inactive)",
                microphone, mixer.mic_status.mic_gains[microphone]
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
        print!(" |{col_name}|");
        table_width += col_name.len() + 3;
    }
    println!();
    println!("{}", "-".repeat(table_width));

    for output in OutputDevice::iter() {
        let row_name = output.to_string();
        print!("|{}{}|", " ".repeat(max_col_len - row_name.len()), row_name,);
        for input in InputDevice::iter() {
            let col_name = input.to_string();
            let len = col_name.len() + 1;
            print!("{}X{} ", " ".repeat(len / 2), " ".repeat(len - (len / 2)));
        }
        println!();
    }
    println!("{}", "-".repeat(table_width));
}
