use serenity::{
    builder::CreateApplicationCommand,
    client::Context,
    model::{
        application::interaction::application_command::ApplicationCommandInteraction, id::GuildId,
    },
};

use crate::prelude::*;

// TODO: can argument/config parsing be (easily) included in the Handler trait?
#[derive(Debug, clap::Args)]
#[group(skip)] // hate hate hate clap please let me rename groups
pub struct Opts {
    /// Base command name to prefix all slash commands with
    #[arg(long, env, default_value = "q")]
    pub command_base: String,
}

pub enum Response {
    Message,
    Modal,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Bot responded with error: {0}")]
    Responded(&'static str),
    #[error("Unexpected error: {0}")]
    NoResponse(#[from] anyhow::Error),
}

pub type Result = std::result::Result<Response, Error>;

#[async_trait]
pub trait Handler: fmt::Debug + Send + Sync {
    fn register(&self, opts: &Opts, cmd: &mut CreateApplicationCommand) -> Option<GuildId>;

    async fn respond(&self, ctx: &Context, cmd: &ApplicationCommandInteraction) -> Result;
}
