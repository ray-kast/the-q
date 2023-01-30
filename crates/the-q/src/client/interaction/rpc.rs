use serenity::model::application::interaction::{
    message_component::MessageComponentInteraction, modal::ModalSubmitInteraction,
};

use super::response::ModalSource;
use crate::prelude::*;

pub trait Key: fmt::Debug + Copy + Eq + Ord + Hash + 'static
where for<'a> Self: From<&'a Self::Payload>
{
    type Payload: fmt::Debug;
    type Interaction;
}

// TODO: these associated type bounds are deranged.  fix them.
pub trait Schema: fmt::Debug {
    type Component: ComponentId<Key = Self::ComponentKey, Payload = Self::ComponentPayload>;
    type ComponentKey: Key<Payload = Self::ComponentPayload, Interaction = MessageComponentInteraction>;
    type ComponentPayload: fmt::Debug;

    type Modal: ModalId<Key = Self::ModalKey, Payload = Self::ModalPayload>;
    type ModalKey: Key<Payload = Self::ModalPayload, Interaction = ModalSubmitInteraction>;
    type ModalPayload: fmt::Debug;
}

pub trait ComponentId: Default + prost::Message {
    type Key: Key<Payload = Self::Payload>;
    type Payload: fmt::Debug;

    fn from_parts(payload: Self::Payload) -> Self;

    fn try_into_parts(self) -> Option<Self::Payload>;
}

pub trait ModalId: Default + prost::Message {
    type Key: Key<Payload = Self::Payload>;
    type Payload: fmt::Debug;

    fn from_parts(src: ModalSource, payload: Self::Payload) -> Self;

    fn try_into_parts(self) -> Option<(ModalSource, Self::Payload)>;
}
