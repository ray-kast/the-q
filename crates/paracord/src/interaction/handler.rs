//! Traits for defining handler logic for various interactions

use std::{fmt, sync::Arc};

use serenity::{
    client::Context,
    model::{
        application::{CommandInteraction, ComponentInteraction, ModalInteraction},
        id::GuildId,
    },
};

use super::{command::CommandInfo, completion::Completion, response, rpc, visitor};

/// Helper trait for constructing an error response
pub trait IntoErr<E> {
    /// Convert this value into an error, attaching the given message
    fn into_err(self, msg: &'static str) -> E;
}

/// A set of handlers from which a [`Registry`](super::registry::Registry) can
/// be created
#[derive(Debug)]
pub struct Handlers<S: rpc::Schema> {
    /// Command (and autocomplete) interaction handlers
    pub commands: Vec<Arc<dyn CommandHandler<S>>>,
    /// Component interaction handlers
    pub components: Vec<Arc<dyn RpcHandler<S, S::ComponentKey>>>,
    /// Modal-submit interaction handlers
    pub modals: Vec<Arc<dyn RpcHandler<S, S::ModalKey>>>,
}

// TODO: Component and Modal should have dedicated visitors
/// Visitor for command interactions
pub type CommandVisitor<'a> = visitor::CommandVisitor<'a, CommandInteraction>;
/// Visitor for component interactions
pub type ComponentVisitor<'a> = visitor::BasicVisitor<'a, ComponentInteraction>;
/// Visitor for autocomplete interactions
pub type CompletionVisitor<'a> = visitor::CommandVisitor<'a, CommandInteraction>;
/// Visitor for modal-submit interactions
pub type ModalVisitor<'a> = visitor::BasicVisitor<'a, ModalInteraction>;

/// An error arising from handling an interaction
#[derive(Debug, thiserror::Error)]
pub enum HandlerError<'a, S, I> {
    /// A visitor extractor returned an error
    #[error("Error parsing interaction data: {0}")]
    Parse(#[from] visitor::Error),
    /// A custom response was dispatched to the user
    #[error("Bot responded with error: {0}")]
    User(&'static str, response::AckedResponder<'a, S, I>),
    /// An unhandled error occurred
    #[error("Unexpected error: {0}")]
    Other(#[from] anyhow::Error),
}

impl<'a, S, I> IntoErr<HandlerError<'a, S, I>> for response::CreatedResponder<'a, S, I> {
    fn into_err(self, msg: &'static str) -> HandlerError<'a, S, I> {
        HandlerError::User(msg, self.into())
    }
}

/// Return type for all interaction handlers
pub type ResponseResult<'a, S, I> =
    Result<response::AckedResponder<'a, S, I>, HandlerError<'a, S, I>>;

/// An error returned from a command interaction handler
pub type CommandError<'a, S> = HandlerError<'a, S, CommandInteraction>;
/// Return type for the command interaction handler method
pub type CommandResult<'a, S> = ResponseResult<'a, S, CommandInteraction>;
/// Responder type provided to command interaction handlers
pub type CommandResponder<'a, 'b, S> = response::BorrowingResponder<'a, 'b, S, CommandInteraction>;
/// Responder type produced by creating a response in a command interaction handler
pub type CreatedCommandResponder<'a, S> = response::CreatedResponder<'a, S, CommandInteraction>;
/// Responder type to be returned by command interaction handlers
pub type AckedCommandResponder<'a, S> = response::AckedResponder<'a, S, CommandInteraction>;

/// An error arising from handling an autocomplete interaction
#[derive(Debug, thiserror::Error)]
pub enum CompletionError {
    /// A visitor extractor returned an error
    #[error("Error parsing command: {0}")]
    Parse(#[from] visitor::Error),
    /// An unhandled error occurred
    #[error("Unexpected error: {0}")]
    Other(#[from] anyhow::Error),
}

/// Return type for the autocomplete interaction handler
pub type CompletionResult = Result<Vec<Completion>, CompletionError>;

/// A handler for a command interaction and its associated autocomplete
/// interactions
#[async_trait::async_trait]
pub trait CommandHandler<S>: fmt::Debug + Send + Sync {
    /// Provide registration data for this command within the global context
    fn register_global(&self) -> CommandInfo;

    /// Provide registration data for this command within the context of a guild
    #[inline]
    fn register_guild(&self, id: GuildId) -> Option<CommandInfo> {
        let _ = (id,);
        None
    }

    /// Respond to an autocomplete interaction
    ///
    /// The default behavior of this method is to return an empty list.
    #[inline]
    async fn complete(
        &self,
        ctx: &Context,
        visitor: &mut CompletionVisitor<'_>,
    ) -> CompletionResult {
        let _ = (ctx, visitor);
        Ok(vec![])
    }

    /// Respond to a command interaction
    // TODO: set timeout for non-deferred commands?
    async fn respond<'a>(
        &self,
        ctx: &Context,
        visitor: &mut CommandVisitor<'_>,
        responder: CommandResponder<'_, 'a, S>,
    ) -> CommandResult<'a, S>;
}

/// An error returned from a component interaction handler
pub type ComponentError<'a, S> = HandlerError<'a, S, ComponentInteraction>;
/// An error returned from a modal-submit interaction handler
pub type ModalError<'a, S> = HandlerError<'a, S, ModalInteraction>;
/// Return type for the component interaction handler method
pub type ComponentResult<'a, S> = ResponseResult<'a, S, ComponentInteraction>;
/// Return type for the modal-submit interaction handler method
pub type ModalResult<'a, S> = ResponseResult<'a, S, ModalInteraction>;
/// Responder type provided to component interaction handlers
pub type ComponentResponder<'a, 'b, S> =
    response::BorrowingResponder<'a, 'b, S, ComponentInteraction>;
/// Responder type to be returned by component interaction handlers
pub type AckedComponentResponder<'a, S> = response::AckedResponder<'a, S, ComponentInteraction>;
/// Responder type provided to modal-submit interaction handlers
pub type ModalResponder<'a, 'b, S> = response::BorrowingResponder<'a, 'b, S, ModalInteraction>;
/// Responder type to be returned by modal-submit interaction handlers
pub type AckedModalResponder<'a, S> = response::AckedResponder<'a, S, ModalInteraction>;

/// A handler for an RPC (i.e. component or modal-submit) interaction
#[async_trait::async_trait]
pub trait RpcHandler<S, K: rpc::Key>: fmt::Debug + Send + Sync {
    /// Register the ID type keys to which this handler can respond
    fn register_keys(&self) -> &'static [K];

    /// Respond to an RPC interaction
    async fn respond<'a>(
        &self,
        ctx: &Context,
        payload: K::Payload,
        visitor: &mut visitor::BasicVisitor<'_, K::Interaction>,
        responder: response::BorrowingResponder<'_, 'a, S, K::Interaction>,
    ) -> ResponseResult<'a, S, K::Interaction>;
}
