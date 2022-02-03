use crate::cli::MicrophoneControls;
use crate::Client;
use anyhow::Result;
use goxlr_ipc::GoXLRCommand;
use goxlr_types::MicrophoneType;

pub async fn apply_microphone_controls(
    microphone_controls: &MicrophoneControls,
    client: &mut Client,
) -> Result<()> {
    if let Some(gain) = microphone_controls.condenser_gain {
        client
            .send(GoXLRCommand::SetMicrophoneGain(
                MicrophoneType::Condenser,
                gain,
            ))
            .await?;
    }
    if let Some(gain) = microphone_controls.dynamic_gain {
        client
            .send(GoXLRCommand::SetMicrophoneGain(
                MicrophoneType::Dynamic,
                gain,
            ))
            .await?;
    }
    if let Some(gain) = microphone_controls.jack_gain {
        client
            .send(GoXLRCommand::SetMicrophoneGain(MicrophoneType::Jack, gain))
            .await?;
    }
    Ok(())
}
