use crate::cli::{ChannelStates, ChannelVolumes};
use crate::Client;
use anyhow::Result;
use goxlr_ipc::GoXLRCommand;
use goxlr_types::ChannelName;

pub async fn apply_channel_volumes(
    channel_volumes: &ChannelVolumes,
    client: &mut Client,
    serial: &str,
) -> Result<()> {
    if let Some(volume) = channel_volumes.mic_volume {
        client
            .command(serial, GoXLRCommand::SetVolume(ChannelName::Mic, volume))
            .await?;
    }
    if let Some(volume) = channel_volumes.line_in_volume {
        client
            .command(serial, GoXLRCommand::SetVolume(ChannelName::LineIn, volume))
            .await?;
    }
    if let Some(volume) = channel_volumes.console_volume {
        client
            .command(
                serial,
                GoXLRCommand::SetVolume(ChannelName::Console, volume),
            )
            .await?;
    }
    if let Some(volume) = channel_volumes.system_volume {
        client
            .command(serial, GoXLRCommand::SetVolume(ChannelName::System, volume))
            .await?;
    }
    if let Some(volume) = channel_volumes.game_volume {
        client
            .command(serial, GoXLRCommand::SetVolume(ChannelName::Game, volume))
            .await?;
    }
    if let Some(volume) = channel_volumes.chat_volume {
        client
            .command(serial, GoXLRCommand::SetVolume(ChannelName::Chat, volume))
            .await?;
    }
    if let Some(volume) = channel_volumes.sample_volume {
        client
            .command(serial, GoXLRCommand::SetVolume(ChannelName::Sample, volume))
            .await?;
    }
    if let Some(volume) = channel_volumes.music_volume {
        client
            .command(serial, GoXLRCommand::SetVolume(ChannelName::Music, volume))
            .await?;
    }
    if let Some(volume) = channel_volumes.headphones_volume {
        client
            .command(
                serial,
                GoXLRCommand::SetVolume(ChannelName::Headphones, volume),
            )
            .await?;
    }
    if let Some(volume) = channel_volumes.mic_monitor_volume {
        client
            .command(
                serial,
                GoXLRCommand::SetVolume(ChannelName::MicMonitor, volume),
            )
            .await?;
    }
    if let Some(volume) = channel_volumes.line_out_volume {
        client
            .command(
                serial,
                GoXLRCommand::SetVolume(ChannelName::LineOut, volume),
            )
            .await?;
    }
    Ok(())
}

pub async fn apply_channel_states(
    channel_states: &ChannelStates,
    client: &mut Client,
    serial: &str,
) -> Result<()> {
    if let Some(muted) = channel_states.mic_muted {
        client
            .command(
                serial,
                GoXLRCommand::SetChannelMuted(ChannelName::Mic, muted, true),
            )
            .await?;
    }
    if let Some(muted) = channel_states.line_in_muted {
        client
            .command(
                serial,
                GoXLRCommand::SetChannelMuted(ChannelName::LineIn, muted, true),
            )
            .await?;
    }
    if let Some(muted) = channel_states.console_muted {
        client
            .command(
                serial,
                GoXLRCommand::SetChannelMuted(ChannelName::Console, muted, true),
            )
            .await?;
    }
    if let Some(muted) = channel_states.system_muted {
        client
            .command(
                serial,
                GoXLRCommand::SetChannelMuted(ChannelName::System, muted, true),
            )
            .await?;
    }
    if let Some(muted) = channel_states.game_muted {
        client
            .command(
                serial,
                GoXLRCommand::SetChannelMuted(ChannelName::Game, muted, true),
            )
            .await?;
    }
    if let Some(muted) = channel_states.chat_muted {
        client
            .command(
                serial,
                GoXLRCommand::SetChannelMuted(ChannelName::Chat, muted, true),
            )
            .await?;
    }
    if let Some(muted) = channel_states.sample_muted {
        client
            .command(
                serial,
                GoXLRCommand::SetChannelMuted(ChannelName::Sample, muted, true),
            )
            .await?;
    }
    if let Some(muted) = channel_states.music_muted {
        client
            .command(
                serial,
                GoXLRCommand::SetChannelMuted(ChannelName::Music, muted, true),
            )
            .await?;
    }
    if let Some(muted) = channel_states.headphones_muted {
        client
            .command(
                serial,
                GoXLRCommand::SetChannelMuted(ChannelName::Headphones, muted, true),
            )
            .await?;
    }
    if let Some(muted) = channel_states.mic_monitor_muted {
        client
            .command(
                serial,
                GoXLRCommand::SetChannelMuted(ChannelName::MicMonitor, muted, true),
            )
            .await?;
    }
    if let Some(muted) = channel_states.line_out_muted {
        client
            .command(
                serial,
                GoXLRCommand::SetChannelMuted(ChannelName::LineOut, muted, true),
            )
            .await?;
    }
    Ok(())
}
