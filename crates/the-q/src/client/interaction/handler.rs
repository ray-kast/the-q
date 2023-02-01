use serenity::{
    client::Context,
    model::{
        application::interaction::{
            application_command::ApplicationCommandInteraction,
            autocomplete::AutocompleteInteraction, message_component::MessageComponentInteraction,
            modal::ModalSubmitInteraction,
        },
        id::GuildId,
    },
};

use super::{command::CommandInfo, completion::Completion, response, rpc, visitor};
use crate::prelude::*;

pub trait IntoErr<E> {
    fn into_err(self, msg: &'static str) -> E;
}

#[derive(Debug)]
pub struct Handlers<S: rpc::Schema> {
    pub commands: Vec<Arc<dyn CommandHandler<S>>>,
    pub components: Vec<Arc<dyn RpcHandler<S, S::ComponentKey>>>,
    pub modals: Vec<Arc<dyn RpcHandler<S, S::ModalKey>>>,
}

// TODO: Component and Modal should have dedicated visitors
pub type CommandVisitor<'a> = visitor::CommandVisitor<'a, ApplicationCommandInteraction>;
pub type ComponentVisitor<'a> = visitor::BasicVisitor<'a, MessageComponentInteraction>;
pub type CompletionVisitor<'a> = visitor::CommandVisitor<'a, AutocompleteInteraction>;
pub type ModalVisitor<'a> = visitor::BasicVisitor<'a, ModalSubmitInteraction>;

#[derive(Debug, thiserror::Error)]
pub enum HandlerError<'a, S, I> {
    #[error("Error parsing command: {0}")]
    Parse(#[from] visitor::Error),
    #[error("Bot responded with error: {0}")]
    User(&'static str, response::AckedResponder<'a, S, I>),
    #[error("Unexpected error: {0}")]
    Other(#[from] anyhow::Error),
}

impl<'a, S, I> IntoErr<HandlerError<'a, S, I>> for response::CreatedResponder<'a, S, I> {
    fn into_err(self, msg: &'static str) -> HandlerError<'a, S, I> {
        HandlerError::User(msg, self.into())
    }
}

pub type ResponseResult<'a, S, I> =
    Result<response::AckedResponder<'a, S, I>, HandlerError<'a, S, I>>;

pub type CommandError<'a, S> = HandlerError<'a, S, ApplicationCommandInteraction>;
pub type CommandResult<'a, S> = ResponseResult<'a, S, ApplicationCommandInteraction>;
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

#[async_trait]
pub trait CommandHandler<S>: fmt::Debug + Send + Sync {
    fn register_global(&self) -> CommandInfo;

    #[inline]
    fn register_guild(&self, id: GuildId) -> Option<CommandInfo> {
        // Use the variables to give the trait args a nice name without getting
        // dead code warnings
        #[allow(let_underscore_drop)]
        let _ = (id,);
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
        visitor: &mut CommandVisitor<'_>,
        responder: CommandResponder<'_, 'a, S>,
    ) -> CommandResult<'a, S>;
}

pub type ComponentError<'a, S> = HandlerError<'a, S, MessageComponentInteraction>;
pub type ModalError<'a, S> = HandlerError<'a, S, ModalSubmitInteraction>;
pub type ComponentResult<'a, S> = ResponseResult<'a, S, MessageComponentInteraction>;
pub type ModalResult<'a, S> = ResponseResult<'a, S, ModalSubmitInteraction>;
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
        visitor: &mut visitor::BasicVisitor<'_, K::Interaction>,
        responder: response::BorrowingResponder<'_, 'a, S, K::Interaction>,
    ) -> ResponseResult<'a, S, K::Interaction>;
}
