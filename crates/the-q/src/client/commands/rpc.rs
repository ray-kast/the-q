use serenity::model::application::interaction::{
    message_component::MessageComponentInteraction, modal::ModalSubmitInteraction,
};

use super::prelude::*;

#[derive(Debug)]
pub struct Schema;

impl rpc::Schema for Schema {
    type Component = component::Component;
    type Modal = modal::Modal;
}

impl rpc::ComponentId for component::Component {
    type Key = ComponentKey;
    type Payload = component::component::Payload;

    fn from_parts(payload: Self::Payload) -> Self {
        Self {
            payload: Some(payload),
        }
    }

    fn into_parts(self) -> Self::Payload { todo!() }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ComponentKey {
    Role,
}

impl From<&ComponentPayload> for ComponentKey {
    fn from(value: &ComponentPayload) -> Self {
        match value {
            ComponentPayload::Role(_) => Self::Role,
        }
    }
}

impl rpc::ModalId for modal::Modal {
    type Key = ModalKey;
    type Payload = modal::modal::Payload;

    fn from_parts(src: ModalSource, payload: Self::Payload) -> Self {
        let src = match src {
            ModalSource::Command => modal::ModalSource::Command,
            ModalSource::Component => modal::ModalSource::Component,
        };
        Self {
            source: src as i32,
            payload: Some(payload),
        }
    }

    fn into_parts(self) -> (ModalSource, Self::Payload) { todo!() }
}

impl rpc::Key for ComponentKey {
    type Interaction = MessageComponentInteraction;
    type Payload = ComponentPayload;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ModalKey {
    Rename,
}

impl From<&ModalPayload> for ModalKey {
    fn from(value: &ModalPayload) -> Self {
        match value {
            ModalPayload::Rename(_) => Self::Rename,
        }
    }
}

impl rpc::Key for ModalKey {
    type Interaction = ModalSubmitInteraction;
    type Payload = ModalPayload;
}
