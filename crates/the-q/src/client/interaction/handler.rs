use serenity::{
    client::Context,
    model::{
        application::interaction::{
            application_command::ApplicationCommandInteraction,
            message_component::MessageComponentInteraction, modal::ModalSubmitInteraction,
        },
        id::GuildId,
    },
};

use super::{command::CommandInfo, completion::Completion, response, rpc, visitor};
use crate::prelude::*;

// TODO: make this type generic
#[derive(Debug, clap::Args)]
#[group(skip)] // hate hate hate clap please let me rename groups
pub struct Opts {
    /// Base command name to prefix all slash commands with
    #[arg(long, env, default_value = "q")]
    pub command_base: String,
}

pub type Visitor<'a> = visitor::Visitor<
    'a,
    serenity::model::application::interaction::application_command::ApplicationCommandInteraction,
>;
pub type CompletionVisitor<'a> = visitor::Visitor<
    'a,
    serenity::model::application::interaction::autocomplete::AutocompleteInteraction,
>;

#[derive(Debug, thiserror::Error)]
pub enum CommandError<'a, S> {
    #[error("Error parsing command: {0}")]
    Parse(#[from] visitor::Error),
    #[error("Bot responded with error: {0}")]
    User(&'static str, AckedCommandResponder<'a, S>),
    #[error("Unexpected error: {0}")]
    Other(#[from] anyhow::Error),
}

pub type CommandResult<'a, S> = Result<AckedCommandResponder<'a, S>, CommandError<'a, S>>;
pub type CommandResponder<'a, 'b, S> =
    response::BorrowingResponder<'a, 'b, S, ApplicationCommandInteraction>;
pub type AckedCommandResponder<'a, S> =
    response::AckedResponder<'a, S, ApplicationCommandInteraction>;

#[derive(Debug, thiserror::Error)]
pub enum CompletionError {
    #[error("Error parsing command: {0}")]
    Parse(#[from] visitor::Error),
    #[error("Bot responded with error: {0}")]
    User(&'static str),
    #[error("Unexpected error: {0}")]
    Other(#[from] anyhow::Error),
}

pub type CompletionResult = Result<Vec<Completion>, CompletionError>;

pub trait IntoErr<E> {
    fn into_err(self, msg: &'static str) -> E;
}

impl<'a, S> IntoErr<CommandError<'a, S>>
    for response::CreatedResponder<'a, S, ApplicationCommandInteraction>
{
    fn into_err(self, msg: &'static str) -> CommandError<'a, S> {
        CommandError::User(msg, self.into())
    }
}

#[async_trait]
pub trait CommandHandler<S>: fmt::Debug + Send + Sync {
    fn register_global(&self, opts: &Opts) -> CommandInfo;

    #[inline]
    fn register_guild(&self, opts: &Opts, id: GuildId) -> Option<CommandInfo> {
        // Use the variables to give the trait args a nice name without getting
        // dead code warnings
        #[allow(let_underscore_drop)]
        let _ = (opts, id);
        None
    }

    #[inline]
    async fn complete(
        &self,
        ctx: &Context,
        visitor: &mut CompletionVisitor<'_>,
    ) -> CompletionResult {
        #[allow(let_underscore_drop)]
        let _ = (ctx, visitor);
        Ok(vec![])
    }

    // TODO: set timeout for non-deferred commands?
    async fn respond<'a>(
        &self,
        ctx: &Context,
        visitor: &mut Visitor<'_>,
        responder: CommandResponder<'_, 'a, S>,
    ) -> CommandResult<'a, S>;
}

#[derive(Debug, thiserror::Error)]
pub enum RpcError<'a, S, I> {
    #[error("Bot responded with error: {0}")]
    User(&'static str, response::AckedResponder<'a, S, I>),
    #[error("Unexpected error: {0}")]
    Other(#[from] anyhow::Error),
}

pub type RpcResult<'a, S, I> = Result<(), RpcError<'a, S, I>>;
pub type ComponentResult<'a, S> = RpcResult<'a, S, MessageComponentInteraction>;
pub type ModalResult<'a, S> = RpcResult<'a, S, ModalSubmitInteraction>;
pub type ComponentResponder<'a, 'b, S> =
    response::BorrowingResponder<'a, 'b, S, MessageComponentInteraction>;
pub type AckedComponentResponder<'a, S> =
    response::AckedResponder<'a, S, MessageComponentInteraction>;
pub type ModalResponder<'a, 'b, S> =
    response::BorrowingResponder<'a, 'b, S, ModalSubmitInteraction>;
pub type AckedModalResponder<'a, S> = response::AckedResponder<'a, S, ModalSubmitInteraction>;

#[async_trait]
pub trait RpcHandler<S, K: rpc::Key>: fmt::Debug + Send + Sync {
    fn register_keys(&self) -> &'static [K];

    async fn respond<'a>(
        &self,
        ctx: &Context,
        payload: K::Payload,
        responder: response::BorrowingResponder<'_, 'a, S, K::Interaction>,
    ) -> RpcResult<S, K::Interaction>;
}
