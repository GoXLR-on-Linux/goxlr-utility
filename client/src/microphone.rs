use crate::cli::MicrophoneControls;
use anyhow::Result;
use goxlr_ipc::client::Client;
use goxlr_ipc::GoXLRCommand;
use goxlr_types::MicrophoneType;

pub async fn apply_microphone_controls(
    microphone_controls: &MicrophoneControls,
    client: &mut Box<dyn Client>,
    serial: &str,
) -> Result<()> {
    if let Some(gain) = microphone_controls.condenser_gain {
        client
            .command(
                serial,
                GoXLRCommand::SetMicrophoneGain(MicrophoneType::Condenser, gain),
            )
            .await?;
    }
    if let Some(gain) = microphone_controls.dynamic_gain {
        client
            .command(
                serial,
                GoXLRCommand::SetMicrophoneGain(MicrophoneType::Dynamic, gain),
            )
            .await?;
    }
    if let Some(gain) = microphone_controls.jack_gain {
        client
            .command(
                serial,
                GoXLRCommand::SetMicrophoneGain(MicrophoneType::Jack, gain),
            )
            .await?;
    }
    Ok(())
}
