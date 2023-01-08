use serenity::{
    builder::{
        CreateApplicationCommand, CreateInteractionResponse, CreateInteractionResponseData,
        EditInteractionResponse,
    },
    client::Context,
    model::{application::interaction::InteractionResponseType, id::GuildId},
    utils::MessageBuilder,
};

use super::{
    response::{Message, MessageBody, MessageOpts},
    visitor,
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

// TODO: session types pls
#[derive(Debug)]
pub enum Response {
    Message(Message),
    DeferMessage(
        MessageOpts,
        tokio::task::JoinHandle<DeferResult<MessageBody>>,
    ),
    UpdateMessage,
    DeferUpdateMessage(tokio::task::JoinHandle<Result<(), DeferError<()>>>),
    Modal,
    Autocomplete,
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

#[derive(Debug, thiserror::Error)]
pub enum DeferError<T> {
    #[error("Bot responded with error: {0}")]
    Response(&'static str, T),
    #[error("Unexpected error: {0}")]
    Other(#[from] anyhow::Error),
}

pub type CommandResult = Result<Response, Error>;
pub type DeferResult<T> = Result<T, DeferError<T>>;

#[async_trait]
pub trait Handler: fmt::Debug + Send + Sync {
    // TODO: returning an optional GuildId is the stupidest way to handle scope
    fn register(&self, opts: &Opts, cmd: &mut CreateApplicationCommand) -> Option<GuildId>;

    // TODO: set timeout for non-deferred commands?
    async fn respond(&self, ctx: &Context, visitor: &mut visitor::Visitor) -> CommandResult;
}
