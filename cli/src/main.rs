use goxlr_usb::channels::Channel;
use goxlr_usb::faders::Fader;
use goxlr_usb::goxlr::GoXLR;
use simplelog::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Debug,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )])
    .unwrap();

    let mut goxlr = GoXLR::open()?;

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
