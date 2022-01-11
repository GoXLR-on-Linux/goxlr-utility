use crate::cli::{ChannelStates, ChannelVolumes};
use crate::Client;
use anyhow::Result;
use goxlr_ipc::GoXLRCommand;
use goxlr_types::ChannelName;

pub async fn apply_channel_volumes(
    channel_volumes: &ChannelVolumes,
    client: &mut Client<'_>,
) -> Result<()> {
    if let Some(volume) = channel_volumes.mic_volume {
        client
            .send(GoXLRCommand::SetVolume(ChannelName::Mic, volume))
            .await?;
    }
    if let Some(volume) = channel_volumes.line_in_volume {
        client
            .send(GoXLRCommand::SetVolume(ChannelName::LineIn, volume))
            .await?;
    }
    if let Some(volume) = channel_volumes.console_volume {
        client
            .send(GoXLRCommand::SetVolume(ChannelName::Console, volume))
            .await?;
    }
    if let Some(volume) = channel_volumes.system_volume {
        client
            .send(GoXLRCommand::SetVolume(ChannelName::System, volume))
            .await?;
    }
    if let Some(volume) = channel_volumes.game_volume {
        client
            .send(GoXLRCommand::SetVolume(ChannelName::Game, volume))
            .await?;
    }
    if let Some(volume) = channel_volumes.chat_volume {
        client
            .send(GoXLRCommand::SetVolume(ChannelName::Chat, volume))
            .await?;
    }
    if let Some(volume) = channel_volumes.sample_volume {
        client
            .send(GoXLRCommand::SetVolume(ChannelName::Sample, volume))
            .await?;
    }
    if let Some(volume) = channel_volumes.music_volume {
        client
            .send(GoXLRCommand::SetVolume(ChannelName::Music, volume))
            .await?;
    }
    if let Some(volume) = channel_volumes.headphones_volume {
        client
            .send(GoXLRCommand::SetVolume(ChannelName::Headphones, volume))
            .await?;
    }
    if let Some(volume) = channel_volumes.mic_monitor_volume {
        client
            .send(GoXLRCommand::SetVolume(ChannelName::MicMonitor, volume))
            .await?;
    }
    if let Some(volume) = channel_volumes.line_out_volume {
        client
            .send(GoXLRCommand::SetVolume(ChannelName::LineOut, volume))
            .await?;
    }
    Ok(())
}

pub async fn apply_channel_states(
    channel_states: &ChannelStates,
    client: &mut Client<'_>,
) -> Result<()> {
    if let Some(muted) = channel_states.mic_muted {
        client
            .send(GoXLRCommand::SetChannelMuted(ChannelName::Mic, muted))
            .await?;
    }
    if let Some(muted) = channel_states.line_in_muted {
        client
            .send(GoXLRCommand::SetChannelMuted(ChannelName::LineIn, muted))
            .await?;
    }
    if let Some(muted) = channel_states.console_muted {
        client
            .send(GoXLRCommand::SetChannelMuted(ChannelName::Console, muted))
            .await?;
    }
    if let Some(muted) = channel_states.system_muted {
        client
            .send(GoXLRCommand::SetChannelMuted(ChannelName::System, muted))
            .await?;
    }
    if let Some(muted) = channel_states.game_muted {
        client
            .send(GoXLRCommand::SetChannelMuted(ChannelName::Game, muted))
            .await?;
    }
    if let Some(muted) = channel_states.chat_muted {
        client
            .send(GoXLRCommand::SetChannelMuted(ChannelName::Chat, muted))
            .await?;
    }
    if let Some(muted) = channel_states.sample_muted {
        client
            .send(GoXLRCommand::SetChannelMuted(ChannelName::Sample, muted))
            .await?;
    }
    if let Some(muted) = channel_states.music_muted {
        client
            .send(GoXLRCommand::SetChannelMuted(ChannelName::Music, muted))
            .await?;
    }
    if let Some(muted) = channel_states.headphones_muted {
        client
            .send(GoXLRCommand::SetChannelMuted(
                ChannelName::Headphones,
                muted,
            ))
            .await?;
    }
    if let Some(muted) = channel_states.mic_monitor_muted {
        client
            .send(GoXLRCommand::SetChannelMuted(
                ChannelName::MicMonitor,
                muted,
            ))
            .await?;
    }
    if let Some(muted) = channel_states.line_out_muted {
        client
            .send(GoXLRCommand::SetChannelMuted(ChannelName::LineOut, muted))
            .await?;
    }
    Ok(())
}
