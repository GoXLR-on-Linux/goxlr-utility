use crate::Client;
use anyhow::Result;
use clap::Args;
use goxlr_ipc::GoXLRCommand;
use goxlr_types::ChannelName;

#[derive(Debug, Args)]
pub struct ChannelControls {
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

impl ChannelControls {
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
