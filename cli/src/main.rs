use std::str::FromStr;
use clap::Parser;
use goxlr_usb::buttonstate;
use goxlr_usb::buttonstate::{ButtonStates, Buttons};
use goxlr_usb::channels::Channel;
use goxlr_usb::channelstate::ChannelState;
use goxlr_usb::commands::Command::SetButtonStates;
use goxlr_usb::error::ConnectError;
use goxlr_usb::faders::Fader;
use goxlr_usb::goxlr::GoXLR;
use goxlr_usb::microphone::MicrophoneType;
use goxlr_usb::routing::{InputDevice, OutputDevice};
use goxlr_usb::rusb::GlobalContext;
use simplelog::*;

#[derive(Parser, Debug)]
#[clap(about, version, author)]
struct Args {
    /// Assign fader A
    #[clap(long, default_value = "Mic")]
    fader_a: String,

    /// Assign fader B
    #[clap(long, default_value = "Chat")]
    fader_b: String,

    /// Assign fader C
    #[clap(long, default_value = "Music")]
    fader_c: String,

    /// Assign fader D
    #[clap(long, default_value = "System")]
    fader_d: String,

    /// How verbose should the output be (can be repeated for super verbosity!)
    #[clap(short, long, parse(from_occurrences))]
    verbose: u8,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Args::parse();

    let (log_level, usb_debug) = match cli.verbose {
        0 => (LevelFilter::Warn, false),
        1 => (LevelFilter::Info, false),
        2 => (LevelFilter::Debug, false),
        3 => (LevelFilter::Debug, true),
        _ => (LevelFilter::Trace, true),
    };

    CombinedLogger::init(vec![TermLogger::new(
        log_level,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )])
    .unwrap();

    if usb_debug {
        goxlr_usb::rusb::set_log_level(goxlr_usb::rusb::LogLevel::Debug);
    }

    let mut goxlr = match GoXLR::open() {
        Ok(goxlr) => goxlr,
        Err(ConnectError::DeviceNotFound) => {
            return Err("No GoXLR device (full or mini) was found.".into())
        }
        Err(ConnectError::UsbError(goxlr_usb::rusb::Error::Access)) => {
            return Err("A GoXLR device was found but this application has insufficient permissions to connect to it. (Have you checked the udev config?)".into())
        }
        Err(e) => return Err(e.into()),
    };

    goxlr.set_fader(Fader::A, Channel::from_str(&cli.fader_a).unwrap())?;
    goxlr.set_fader(Fader::B, Channel::from_str(&cli.fader_b).unwrap())?;
    goxlr.set_fader(Fader::C, Channel::from_str(&cli.fader_c).unwrap())?;
    goxlr.set_fader(Fader::D, Channel::from_str(&cli.fader_d).unwrap())?;

    goxlr.set_volume(Channel::Mic, 0xFF)?;
    goxlr.set_volume(Channel::Game, 0xFF)?;
    goxlr.set_volume(Channel::Chat, 0xFF)?;
    goxlr.set_volume(Channel::System, 0xFF)?;

    goxlr.set_channel_state(Channel::System, ChannelState::Unmuted);

    // So this is a complex one, there's no direct way to retrieve the button colour states
    // directly from the GoXLR, it's all managed by the app.. So for testing, all we're going
    // to do here, is a simple example of managing the buttons.

    // Define our buttons, set them all to a Dimmed State..
    let mut button_states: [u8; 24] = [ButtonStates::Dimmed.id(); 24];

    // Now set 'Mute' to a lit state..
    button_states[Buttons::Fader1Mute.position()] = ButtonStates::On.id();

    // Apply the state.
    goxlr.set_button_states(button_states);

    /*
    Ok, this is awkward as hell, this *WILL* need improving, but proof-of-concept currently..

    Left and Right channels for both sources and destinations appear to be configured separately by
    the GoXLR, but it's essentially handled with a list of 'on' or 'off' for channels in the
    correct order. The defined 'on' value is 8192 as a u16, which as bytes and endiand is
    [0x00, 0x20], so I'm just slapping the 0x20 into the correct byte slot of the list, and
    sending it run through (correct byte position being provided by an enum for convenience)
     */

    /*
        let mut gameRoutingStateLeft: [u8;22] = [0; 22];
        gameRoutingStateLeft[OutputDevice::HeadphonesLeft.position()] = 0x20;
        gameRoutingStateLeft[OutputDevice::BroadcastMixLeft.position()] = 0x20;
        goxlr.set_routing(InputDevice::GameLeft, gameRoutingStateLeft);


        let mut gameRoutingStateRight : [u8;22] = [0; 22];
        gameRoutingStateRight[OutputDevice::HeadphonesRight.position()] = 0x20;
        gameRoutingStateRight[OutputDevice::BroadcastMixRight.position()] = 0x20;
        goxlr.set_routing(InputDevice::GameRight, gameRoutingStateRight);
    */

    // Enables Phantom Mode..
    // goxlr.set_microphone_type(MicrophoneType::Phantom, 40);

    Ok(())
}
