use serenity::{
    builder::{CreateApplicationCommand, CreateInteractionResponseData},
    client::Context,
    model::id::GuildId,
    utils::MessageBuilder,
};

use super::visitor;
use crate::prelude::*;

// TODO: can argument/config parsing be (easily) included in the Handler trait?
#[derive(Debug, clap::Args)]
#[group(skip)] // hate hate hate clap please let me rename groups
pub struct Opts {
    /// Base command name to prefix all slash commands with
    #[arg(long, env, default_value = "q")]
    pub command_base: String,
}

#[derive(Debug)]
pub struct Message {
    content: String,
    ephemeral: bool,
}

impl Message {
    pub(super) fn apply<'a, 'b>(
        self,
        msg: &'b mut CreateInteractionResponseData<'a>,
    ) -> &'b mut CreateInteractionResponseData<'a> {
        let Self { content, ephemeral } = self;
        msg.content(content).ephemeral(ephemeral)
    }

    #[inline]
    pub fn rich(f: impl FnOnce(&mut MessageBuilder) -> &mut MessageBuilder) -> Self {
        let mut mb = MessageBuilder::new();
        f(&mut mb);
        Self {
            content: mb.build(),
            ephemeral: false,
        }
    }

    #[inline]
    pub fn plain(c: impl Into<serenity::utils::Content>) -> Self {
        Self::rich(|mb| mb.push_safe(c))
    }

    pub fn ephemeral(self, ephemeral: bool) -> Self { Self { ephemeral, ..self } }
}

#[derive(Debug)]
pub enum Response {
    Message(Message),
    Modal,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Error parsing command: {0}")]
    Parse(#[from] visitor::Error),
    #[error("Bot responded with error: {0}")]
    Response(&'static str, Message),
    #[error("Unexpected error: {0}")]
    Other(#[from] anyhow::Error),
}

pub type Result = std::result::Result<Response, Error>;

#[async_trait]
pub trait Handler: fmt::Debug + Send + Sync {
    // TODO: returning an optional GuildId is the stupidest way to handle scope
    fn register(&self, opts: &Opts, cmd: &mut CreateApplicationCommand) -> Option<GuildId>;

    // TODO: remove async and force the use of Deferred?
    // TODO: '_?
    async fn respond<'a>(&self, ctx: &Context, visitor: visitor::Visitor<'a>) -> Result;
}
