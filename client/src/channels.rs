use crate::Client;
use anyhow::Result;
use clap::Args;
use goxlr_ipc::GoXLRCommand;
use goxlr_types::ChannelName;

#[derive(Debug, Args)]
pub struct ChannelVolumes {
    /// Set Mic volume (0-255)
    #[clap(long)]
    mic_volume: Option<u8>,

    /// Set Line-In volume (0-255)
    #[clap(long)]
    line_in_volume: Option<u8>,

    /// Set Console volume (0-255)
    #[clap(long)]
    console_volume: Option<u8>,

    /// Set System volume (0-255)
    #[clap(long)]
    system_volume: Option<u8>,

    /// Set Game volume (0-255)
    #[clap(long)]
    game_volume: Option<u8>,

    /// Set Chat volume (0-255)
    #[clap(long)]
    chat_volume: Option<u8>,

    /// Set Sample volume (0-255)
    #[clap(long)]
    sample_volume: Option<u8>,

    /// Set Music volume (0-255)
    #[clap(long)]
    music_volume: Option<u8>,

    /// Set Headphones volume (0-255)
    #[clap(long)]
    headphones_volume: Option<u8>,

    /// Set Mic-Monitor volume (0-255)
    #[clap(long)]
    mic_monitor_volume: Option<u8>,

    /// Set Line-Out volume (0-255)
    #[clap(long)]
    line_out_volume: Option<u8>,
}

impl ChannelVolumes {
    pub async fn apply(&self, client: &mut Client<'_>) -> Result<()> {
        if let Some(volume) = self.mic_volume {
            client
                .send(GoXLRCommand::SetVolume(ChannelName::Mic, volume))
                .await?;
        }
        if let Some(volume) = self.line_in_volume {
            client
                .send(GoXLRCommand::SetVolume(ChannelName::LineIn, volume))
                .await?;
        }
        if let Some(volume) = self.console_volume {
            client
                .send(GoXLRCommand::SetVolume(ChannelName::Console, volume))
                .await?;
        }
        if let Some(volume) = self.system_volume {
            client
                .send(GoXLRCommand::SetVolume(ChannelName::System, volume))
                .await?;
        }
        if let Some(volume) = self.game_volume {
            client
                .send(GoXLRCommand::SetVolume(ChannelName::Game, volume))
                .await?;
        }
        if let Some(volume) = self.chat_volume {
            client
                .send(GoXLRCommand::SetVolume(ChannelName::Chat, volume))
                .await?;
        }
        if let Some(volume) = self.sample_volume {
            client
                .send(GoXLRCommand::SetVolume(ChannelName::Sample, volume))
                .await?;
        }
        if let Some(volume) = self.music_volume {
            client
                .send(GoXLRCommand::SetVolume(ChannelName::Music, volume))
                .await?;
        }
        if let Some(volume) = self.headphones_volume {
            client
                .send(GoXLRCommand::SetVolume(ChannelName::Headphones, volume))
                .await?;
        }
        if let Some(volume) = self.mic_monitor_volume {
            client
                .send(GoXLRCommand::SetVolume(ChannelName::MicMonitor, volume))
                .await?;
        }
        if let Some(volume) = self.line_out_volume {
            client
                .send(GoXLRCommand::SetVolume(ChannelName::LineOut, volume))
                .await?;
        }
        Ok(())
    }
}

#[derive(Debug, Args)]
pub struct ChannelStates {
    /// Set Mic muted status
    #[clap(long)]
    mic_muted: Option<bool>,

    /// Set Line-In muted status
    #[clap(long)]
    line_in_muted: Option<bool>,

    /// Set Console muted status
    #[clap(long)]
    console_muted: Option<bool>,

    /// Set System muted status
    #[clap(long)]
    system_muted: Option<bool>,

    /// Set Game muted status
    #[clap(long)]
    game_muted: Option<bool>,

    /// Set Chat muted status
    #[clap(long)]
    chat_muted: Option<bool>,

    /// Set Sample muted status
    #[clap(long)]
    sample_muted: Option<bool>,

    /// Set Music muted status
    #[clap(long)]
    music_muted: Option<bool>,

    /// Set Headphones muted status
    #[clap(long)]
    headphones_muted: Option<bool>,

    /// Set Mic-Monitor muted status
    #[clap(long)]
    mic_monitor_muted: Option<bool>,

    /// Set Line-Out muted status
    #[clap(long)]
    line_out_muted: Option<bool>,
}

impl ChannelStates {
    pub async fn apply(&self, client: &mut Client<'_>) -> Result<()> {
        if let Some(muted) = self.mic_muted {
            client
                .send(GoXLRCommand::SetChannelMuted(ChannelName::Mic, muted))
                .await?;
        }
        if let Some(muted) = self.line_in_muted {
            client
                .send(GoXLRCommand::SetChannelMuted(ChannelName::LineIn, muted))
                .await?;
        }
        if let Some(muted) = self.console_muted {
            client
                .send(GoXLRCommand::SetChannelMuted(ChannelName::Console, muted))
                .await?;
        }
        if let Some(muted) = self.system_muted {
            client
                .send(GoXLRCommand::SetChannelMuted(ChannelName::System, muted))
                .await?;
        }
        if let Some(muted) = self.game_muted {
            client
                .send(GoXLRCommand::SetChannelMuted(ChannelName::Game, muted))
                .await?;
        }
        if let Some(muted) = self.chat_muted {
            client
                .send(GoXLRCommand::SetChannelMuted(ChannelName::Chat, muted))
                .await?;
        }
        if let Some(muted) = self.sample_muted {
            client
                .send(GoXLRCommand::SetChannelMuted(ChannelName::Sample, muted))
                .await?;
        }
        if let Some(muted) = self.music_muted {
            client
                .send(GoXLRCommand::SetChannelMuted(ChannelName::Music, muted))
                .await?;
        }
        if let Some(muted) = self.headphones_muted {
            client
                .send(GoXLRCommand::SetChannelMuted(
                    ChannelName::Headphones,
                    muted,
                ))
                .await?;
        }
        if let Some(muted) = self.mic_monitor_muted {
            client
                .send(GoXLRCommand::SetChannelMuted(
                    ChannelName::MicMonitor,
                    muted,
                ))
                .await?;
        }
        if let Some(muted) = self.line_out_muted {
            client
                .send(GoXLRCommand::SetChannelMuted(ChannelName::LineOut, muted))
                .await?;
        }
        Ok(())
    }
}
