//! Traits for defining handler logic for various interactions

use std::{fmt, pin::Pin, sync::Arc};

use futures_util::TryFutureExt;
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
pub struct Handlers<S: rpc::Schema, C> {
    /// Command (and autocomplete) interaction handlers
    pub commands: Vec<Arc<dyn DynCommandHandler<S, C>>>,
    /// Component interaction handlers
    pub components: Vec<Arc<dyn DynRpcHandler<S, S::ComponentKey, C>>>,
    /// Modal-submit interaction handlers
    pub modals: Vec<Arc<dyn DynRpcHandler<S, S::ModalKey, C>>>,
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

/// Registration and parsing functionality for application commands
pub trait DeserializeCommand<'a, C>: Sized + Send {
    /// Deserialized type of autocomplete interactions
    type Completion: Send;

    /// Provide registration data for this command within the global context
    fn register_global(cx: &C) -> CommandInfo;

    /// Provide registration data for this command within the context of a guild
    fn register_guild(id: GuildId, cx: &C) -> Option<CommandInfo>;

    /// Deserialize payload data for autocomplete invocations of this command
    ///
    /// # Errors
    /// This method should return an error if the provided interaction data is
    /// invalid
    fn deserialize_completion(
        visitor: &mut CompletionVisitor<'a>,
    ) -> Result<Self::Completion, visitor::Error>;

    /// Deserialize payload data for invocations of this command
    ///
    /// # Errors
    /// This method should return an error if the provided interaction data is
    /// invalid
    fn deserialize(visitor: &mut CommandVisitor<'a>) -> Result<Self, visitor::Error>;
}

/// Type representing command data with no possible autocomplete interactions
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NoCompletion {}

/// A handler for a command interaction and its associated autocomplete
/// interactions
pub trait CommandHandler<S, C: Sync>: fmt::Debug + Send + Sync {
    /// Payload data for this handler
    type Data<'a>: DeserializeCommand<'a, C>
    where
        Self: 'a,
        C: 'a;

    /// Respond to an autocomplete interaction
    ///
    /// The default behavior of this method is to return an empty list.
    #[inline]
    fn complete<'a>(
        &'a self,
        serenity_cx: &'a Context,
        cx: &'a C,
        data: <Self::Data<'a> as DeserializeCommand<'a, C>>::Completion,
    ) -> impl Future<Output = CompletionResult> + Send + use<'a, Self, S, C> {
        let _ = (serenity_cx, cx, data);
        std::future::ready(Ok(vec![]))
    }

    /// Respond to a command interaction
    // TODO: set timeout for non-deferred commands?
    fn respond<'a, 'r>(
        &'a self,
        serenity_cx: &'a Context,
        cx: &'a C,
        data: Self::Data<'a>,
        responder: CommandResponder<'a, 'r, S>,
    ) -> impl Future<Output = CommandResult<'r, S>> + Send + use<'a, 'r, Self, S, C>;
}

/// Dyn-safe version of [`CommandHandler`] and [`DeserializeCommand`]
pub trait DynCommandHandler<S, C: Sync>: fmt::Debug + Send + Sync {
    /// Dyn-safe version of [`DeserializeCommand::register_global`]
    fn dyn_register_global(&self, cx: &C) -> CommandInfo;

    /// Dyn-safe version of [`DeserializeCommand::register_guild`]
    fn dyn_register_guild(&self, id: GuildId, cx: &C) -> Option<CommandInfo>;

    /// Dyn-safe version of [`DeserializeCommand::deserialize_completion`] and
    /// [`CommandHandler::complete`]
    fn dyn_complete<'a>(
        &'a self,
        serenity_cx: &'a Context,
        cx: &'a C,
        visitor: &'a mut CompletionVisitor<'a>,
    ) -> Pin<Box<dyn Future<Output = CompletionResult> + Send + 'a>>
    where
        S: 'a,
        C: 'a;

    /// Dyn-safe version of [`DeserializeCommand::deserialize`] and
    /// [`CommandHandler::respond`]
    fn dyn_respond<'a, 'r>(
        &'a self,
        serenity_cx: &'a Context,
        cx: &'a C,
        visitor: &mut CommandVisitor<'a>,
        responder: CommandResponder<'a, 'r, S>,
    ) -> Pin<Box<dyn Future<Output = CommandResult<'r, S>> + Send + 'a>>
    where
        S: 'a,
        C: 'a;
}

impl<S, C: Sync, H: CommandHandler<S, C>> DynCommandHandler<S, C> for H {
    #[inline]
    fn dyn_register_global(&self, cx: &C) -> CommandInfo { H::Data::register_global(cx) }

    #[inline]
    fn dyn_register_guild(&self, id: GuildId, cx: &C) -> Option<CommandInfo> {
        H::Data::register_guild(id, cx)
    }

    #[inline]
    fn dyn_complete<'a>(
        &'a self,
        serenity_cx: &'a Context,
        cx: &'a C,
        visitor: &'a mut CompletionVisitor<'a>,
    ) -> Pin<Box<dyn Future<Output = CompletionResult> + Send + 'a>>
    where
        S: 'a,
        C: 'a,
    {
        Box::pin(
            std::future::ready(H::Data::deserialize_completion(visitor))
                .map_err(Into::into)
                .and_then(|data| self.complete(serenity_cx, cx, data)),
        )
    }

    #[inline]
    fn dyn_respond<'a, 'r>(
        &'a self,
        serenity_cx: &'a Context,
        cx: &'a C,
        visitor: &mut CommandVisitor<'a>,
        responder: CommandResponder<'a, 'r, S>,
    ) -> Pin<Box<dyn Future<Output = CommandResult<'r, S>> + Send + 'a>>
    where
        S: 'a,
        C: 'a,
    {
        Box::pin(
            std::future::ready(H::Data::deserialize(visitor))
                .map_err(Into::into)
                .and_then(|data| self.respond(serenity_cx, cx, data, responder)),
        )
    }
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

/// Registration and parsing functionality for application components and
/// modals
pub trait DeserializeRpc<'a, K: rpc::Key, C>: Sized + Send {
    /// Register the ID type keys to which this handler can respond
    fn register_keys(cx: &C) -> &[K];

    /// Deserialize payload data for an RPC interaction
    ///
    /// # Errors
    /// This method should return an error if the provided interaction data is
    /// invalid
    fn deserialize(
        visitor: &mut visitor::BasicVisitor<'a, K::Interaction>,
    ) -> Result<Self, visitor::Error>;
}

/// A handler for an RPC (i.e. component or modal-submit) interaction
pub trait RpcHandler<S, K: rpc::Key, C: Sync>: fmt::Debug + Send + Sync {
    /// Payload data for this handler
    type Data<'a>: DeserializeRpc<'a, K, C>
    where
        Self: 'a,
        S: 'a,
        K: 'a,
        C: 'a;

    /// Respond to an RPC interaction
    fn respond<'a, 'r>(
        &'a self,
        serenity_cx: &'a Context,
        cx: &'a C,
        payload: K::Payload,
        data: Self::Data<'a>,
        responder: response::BorrowingResponder<'a, 'r, S, K::Interaction>,
    ) -> impl Future<Output = ResponseResult<'r, S, K::Interaction>> + Send + use<'a, 'r, Self, S, K, C>;
}

/// Dyn-safe version of [`RpcHandler`] and [`DeserializeRpc`]
pub trait DynRpcHandler<S, K: rpc::Key, C: Sync>: fmt::Debug + Send + Sync {
    /// Dyn-safe version of [`DeserializeRpc::register_keys`]
    fn dyn_register_keys<'k>(&self, cx: &'k C) -> &'k [K];

    /// Dyn-safe version of [`DeserializeRpc::deserialize`] and [`RpcHandler::respond`]
    fn dyn_respond<'a, 'r>(
        &'a self,
        serenity_cx: &'a Context,
        cx: &'a C,
        payload: K::Payload,
        visitor: &'a mut visitor::BasicVisitor<'a, K::Interaction>,
        responder: response::BorrowingResponder<'a, 'r, S, K::Interaction>,
    ) -> Pin<Box<dyn Future<Output = ResponseResult<'r, S, K::Interaction>> + Send + 'a>>
    where
        S: 'a,
        K: 'a,
        C: 'a;
}

impl<S, K: rpc::Key, C: Sync, H: RpcHandler<S, K, C>> DynRpcHandler<S, K, C> for H {
    #[inline]
    fn dyn_register_keys<'k>(&self, cx: &'k C) -> &'k [K] { H::Data::register_keys(cx) }

    #[inline]
    fn dyn_respond<'a, 'r>(
        &'a self,
        serenity_cx: &'a Context,
        cx: &'a C,
        payload: K::Payload,
        visitor: &'a mut visitor::BasicVisitor<'a, K::Interaction>,
        responder: response::BorrowingResponder<'a, 'r, S, K::Interaction>,
    ) -> Pin<Box<dyn Future<Output = ResponseResult<'r, S, K::Interaction>> + Send + 'a>>
    where
        S: 'a,
        K: 'a,
        C: 'a,
    {
        Box::pin(
            std::future::ready(H::Data::deserialize(visitor))
                .map_err(Into::into)
                .and_then(|data| self.respond(serenity_cx, cx, payload, data, responder)),
        )
    }
}
