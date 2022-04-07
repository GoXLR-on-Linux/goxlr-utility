use crate::cli::ChannelVolumes;
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
