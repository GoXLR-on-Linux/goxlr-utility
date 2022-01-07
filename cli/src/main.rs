use clap::Parser;

use goxlr_usb::buttonstate::{ButtonStates, Buttons};
use goxlr_usb::channels::Channel;
use goxlr_usb::channelstate::ChannelState;


use goxlr_usb::error::ConnectError;
use goxlr_usb::faders::Fader;
use goxlr_usb::goxlr::GoXLR;



use simplelog::*;
use std::str::FromStr;

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

    goxlr.set_volume(Channel::from_str(&cli.fader_a).unwrap(), 0xFF)?;
    goxlr.set_volume(Channel::from_str(&cli.fader_b).unwrap(), 0xFF)?;
    goxlr.set_volume(Channel::from_str(&cli.fader_c).unwrap(), 0xFF)?;
    goxlr.set_volume(Channel::from_str(&cli.fader_d).unwrap(), 0xFF)?;

    goxlr.set_channel_state(Channel::System, ChannelState::Unmuted);

    // So this is a complex one, there's no direct way to retrieve the button colour states
    // directly from the GoXLR, it's all managed by the app.. So for testing, all we're going
    // to do here, is a simple example of managing the buttons.

    // Define our buttons, set them all to a Dimmed State..
    let mut button_states: [ButtonStates; 24] = [ButtonStates::DimmedColour1; 24];

    // Now set 'Mute' to a lit state..
    button_states[Buttons::Fader2Mute as usize] = ButtonStates::Colour1;

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

    /*
    Again, this will need some level of improvement and structuring, more proof-of-concept..

    Colour setting is 328 bytes, which are a repeated reversed RGBX bytes (TODO: Find out X), so
    for now, we're just going to create an array, use helpers to determine the correct array
    positions, manually set colours on buttons, then ship it.
     */

    /*
    // First, create an array (this will default to all lights being off)
    let mut colourSettings: [u8;328] = [0; 328];

    // Turn on the Microphone Button, and set it's 'on' colour to Red, and 'Off' colour to blue.
    // Note that the Colour Type (Coming Soon) needs to be set to 'Colour2' and not 'Dimmed'
    // Todo: possibly change 'position' so it accepts '1' as the first colour!
    let startColour1 = ColourTargets::MicrophoneMute.position(0);
    colourSettings[startColour1] = 0x00;      // Blue
    colourSettings[startColour1 + 1] = 0x00;  // Green
    colourSettings[startColour1 + 2] = 0xff;  // Red
    colourSettings[startColour1 + 3] = 0x00;  // 'X'

    let startColour2 = ColourTargets::MicrophoneMute.position(1);
    colourSettings[startColour2] = 0xff;      // Blue
    colourSettings[startColour2 + 1] = 0x00;  // Green
    colourSettings[startColour2 + 2] = 0x00;  // Red
    colourSettings[startColour2 + 3] = 0xff;  // 'X'

    // Ship it!
    goxlr.set_button_colours(colourSettings);
    */

    // TIME TO SCRIBBLE!

    /*
    Notes:
    I recognise binary when I see it..

    The display is 128px across, and each byte represents a 1x8px vertical column.
    The display is 64px high, so 8 rows total.

    Data in each vertical column is a simple binary 'on / off' flag for each bit, with 0 being
    'dark', and 1 being 'light'. The bit order goes from Bottom -> Top

    So to colour the top and bottom pixel of the bar, the binary would be 01111110, in hex 0x7E

    The following code will draw a 2px border around the scribble area, leaving a one pixel 'safe
    zone' around the edge.
     */

    /*
    let mut scribble: [u8;1024] = [0xff; 1024];

    // scribble[0] is in the safe zone, don't draw it.
    // For 1 and 2, we need to simply draw everything *EXCEPT* the first pixel (safe zone)
    // 00000001 = 0x01
    scribble[1] = 0x01;
    scribble[2] = 0x01;

    // For the next 124 pixels, we only need to draw the top two pixels, leaving a pixel safe.
    for n in 3 .. 125 {
        // 11111001 = 0xF9 (Top 2 pixels and safe zone)
        scribble[n] = 0xF9;
    }

    // As above, fill except safe zone.
    scribble[125] = 0x01;
    scribble[126] = 0x01;
    // scribble[127] is in the safe zone, don't draw it.

    // For the next 6 rows, we need to full fill columns 1, 2, 125 and 126..
    for n in 1 .. 7 {
        scribble[1 + (n * 128)] = 0x00;
        scribble[2 + (n * 128)] = 0x00;

        scribble[125 + (n * 128)] = 0x00;
        scribble[126 + (n * 128)] = 0x00;
    }

    // And for the final row, the reverse of the first set..
    // 10000000 = 0x80
    scribble[1 + (7 * 128)] = 0x80;
    scribble[2 + (7 * 128)] = 0x80;


    for n in 3 .. 125 {
        // Only need the bottom pixels and not the safe zone..
        // 10011111 = 0x9F

        scribble[n + (7 * 128)] = 0x9F;
    }

    // Full fill the last two columns.
    scribble[125 + (7 * 128)] = 0x80;
    scribble[126 + (7 * 128)] = 0x80;

    // Send this to fader 1..
    goxlr.set_fader_scribble(Fader::A, scribble);
    */

    Ok(())
}
