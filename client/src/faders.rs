use crate::cli::FaderControls;
use crate::Client;
use anyhow::Result;
use goxlr_ipc::GoXLRCommand;
use goxlr_types::FaderName;

pub async fn apply_fader_controls(
    fader_controls: &FaderControls,
    client: &mut Client,
    serial: &str,
) -> Result<()> {
    if let Some(channel) = fader_controls.fader_a {
        client
            .command(serial, GoXLRCommand::AssignFader(FaderName::A, channel))
            .await?;
    }
    if let Some(channel) = fader_controls.fader_b {
        client
            .command(serial, GoXLRCommand::AssignFader(FaderName::B, channel))
            .await?;
    }
    if let Some(channel) = fader_controls.fader_c {
        client
            .command(serial, GoXLRCommand::AssignFader(FaderName::C, channel))
            .await?;
    }
    if let Some(channel) = fader_controls.fader_d {
        client
            .command(serial, GoXLRCommand::AssignFader(FaderName::D, channel))
            .await?;
    }
    Ok(())
}
