use serenity::{
    client::Context,
    model::{
        application::interaction::application_command::ApplicationCommandInteraction, id::GuildId,
    },
};

use super::{command::CommandInfo, completion::Completion, response, visitor};
use crate::prelude::*;

// TODO: can argument/config parsing be (easily) included in the Handler trait?
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
pub enum CommandError<'a> {
    #[error("Error parsing command: {0}")]
    Parse(#[from] visitor::Error),
    #[error("Bot responded with error: {0}")]
    User(&'static str, AckedCommandResponder<'a>),
    #[error("Unexpected error: {0}")]
    Other(#[from] anyhow::Error),
}

pub type CommandResult<'a> = Result<AckedCommandResponder<'a>, CommandError<'a>>;
pub type CommandResponder<'a, 'b> =
    response::BorrowingResponder<'a, 'b, ApplicationCommandInteraction>;
pub type AckedCommandResponder<'a> = response::AckedResponder<'a, ApplicationCommandInteraction>;

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

impl<'a> IntoErr<CommandError<'a>>
    for response::CreatedResponder<'a, ApplicationCommandInteraction>
{
    fn into_err(self, msg: &'static str) -> CommandError<'a> {
        CommandError::User(msg, self.into())
    }
}

#[async_trait]
pub trait CommandHandler: fmt::Debug + Send + Sync {
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
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a>;
}
