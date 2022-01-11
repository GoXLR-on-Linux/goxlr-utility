use crate::Client;
use anyhow::Result;
use clap::Args;
use goxlr_ipc::GoXLRCommand;
use goxlr_types::{ChannelName, FaderName};

#[derive(Debug, Args)]
pub struct FaderControls {
    /// Assign fader A
    #[clap(arg_enum, long)]
    fader_a: Option<ChannelName>,

    /// Assign fader B
    #[clap(arg_enum, long)]
    fader_b: Option<ChannelName>,

    /// Assign fader C
    #[clap(arg_enum, long)]
    fader_c: Option<ChannelName>,

    /// Assign fader D
    #[clap(arg_enum, long)]
    fader_d: Option<ChannelName>,
}

impl FaderControls {
    pub async fn apply(&self, client: &mut Client<'_>) -> Result<()> {
        if let Some(channel) = self.fader_a {
            client
                .send(GoXLRCommand::AssignFader(FaderName::A, channel))
                .await?;
        }
        if let Some(channel) = self.fader_b {
            client
                .send(GoXLRCommand::AssignFader(FaderName::B, channel))
                .await?;
        }
        if let Some(channel) = self.fader_c {
            client
                .send(GoXLRCommand::AssignFader(FaderName::C, channel))
                .await?;
        }
        if let Some(channel) = self.fader_d {
            client
                .send(GoXLRCommand::AssignFader(FaderName::D, channel))
                .await?;
        }
        Ok(())
    }
}
