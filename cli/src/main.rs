use clap::Parser;
use goxlr_usb::channels::Channel;
use goxlr_usb::faders::Fader;
use goxlr_usb::goxlr::GoXLR;
use simplelog::*;
use goxlr_usb::channelstate::ChannelState;

#[derive(Parser, Debug)]
#[clap(about, version, author)]
struct Args {
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

    let mut goxlr = GoXLR::open()?;

    goxlr.set_volume(Channel::Mic, 0xFF)?;
    goxlr.set_volume(Channel::Chat, 0xFF)?;
    goxlr.set_volume(Channel::Music, 0xFF)?;
    goxlr.set_volume(Channel::System, 0xFF)?;

    goxlr.set_fader(Fader::A, Channel::Mic)?;
    goxlr.set_fader(Fader::B, Channel::Chat)?;
    goxlr.set_fader(Fader::C, Channel::Music)?;
    goxlr.set_fader(Fader::D, Channel::System)?;

    goxlr.set_channel_state(Channel::System, ChannelState::Unmuted);

    Ok(())
}
