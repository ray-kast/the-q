//! Traits for defining the pseudo-RPC interface created by the [`Id`]
//! and [`Components`] types
//!
//! [`Id`]: super::response::id::Id
//! [`Components`]: super::response::Components

use std::fmt;

use serenity::model::application::interaction::{
    message_component::MessageComponentInteraction, modal::ModalSubmitInteraction,
};

use super::response::ModalSource;

/// A key identifying the static handler for an RPC request (i.e. the remote
/// procedure name)
pub trait Key: fmt::Debug + Copy + Eq + Ord + std::hash::Hash + 'static
where for<'a> Self: From<&'a Self::Payload>
{
    /// The payload type for this set of keys, containing enough data to produce
    /// an instance of [`Self`] as well as any arguments that should be
    /// forwarded
    type Payload: fmt::Debug;
    /// The interaction event type for which this key is valid
    type Interaction;
}

/// A helper trait intended to be implemented on a marker unit struct for
/// defining the types associated with the RPC schema for a
/// [`Registry`](super::registry::Registry)
// TODO: these associated type bounds are deranged.  fix them.
pub trait Schema: fmt::Debug {
    /// The message type for custom component IDs
    type Component: ComponentId<Key = Self::ComponentKey, Payload = Self::ComponentPayload>;
    /// The key type identifying the kind of a [`Component`](Self::Component)
    /// message
    type ComponentKey: Key<Payload = Self::ComponentPayload, Interaction = MessageComponentInteraction>;
    /// The payload of a [`Component`](Self::Component) message
    type ComponentPayload: fmt::Debug;

    /// The message type for modal custom IDs
    type Modal: ModalId<Key = Self::ModalKey, Payload = Self::ModalPayload>;
    /// The key type identifying the kind of a [`Modal`](Self::Modal) message
    type ModalKey: Key<Payload = Self::ModalPayload, Interaction = ModalSubmitInteraction>;
    /// The payload of a [`Modal`](Self::Modal) message
    type ModalPayload: fmt::Debug;
}

/// A valid message for encoding into custom component IDs
pub trait ComponentId: Default + prost::Message {
    /// The key type identifying the kind of this message
    type Key: Key<Payload = Self::Payload>;
    /// The payload type of this message
    type Payload: fmt::Debug;

    /// Construct a new ID message from its inner payload
    fn from_parts(payload: Self::Payload) -> Self;

    /// Destructure an ID message into its inner payload
    fn try_into_parts(self) -> Option<Self::Payload>;
}

/// A valid message for encoding into modal custom IDs
pub trait ModalId: Default + prost::Message {
    /// The key type identifying the kind of this message
    type Key: Key<Payload = Self::Payload>;
    /// The payload type of this message
    type Payload: fmt::Debug;

    /// Construct a new ID message from the modal source tag and its inner
    /// payload
    fn from_parts(src: ModalSource, payload: Self::Payload) -> Self;

    /// Destructure an ID message into its source tag and inner payload
    fn try_into_parts(self) -> Option<(ModalSource, Self::Payload)>;
}
