use serenity::model::application::interaction::{
    message_component::MessageComponentInteraction, modal::ModalSubmitInteraction,
};

use super::prelude::*;

#[derive(Debug)]
pub struct Schema;

impl rpc::Schema for Schema {
    type Component = component::Component;
    type ComponentKey = ComponentKey;
    type ComponentPayload = ComponentPayload;
    type Modal = modal::Modal;
    type ModalKey = ModalKey;
    type ModalPayload = ModalPayload;
}

impl rpc::ComponentId for component::Component {
    type Key = ComponentKey;
    type Payload = component::component::Payload;

    fn from_parts(payload: Self::Payload) -> Self {
        Self {
            payload: Some(payload),
        }
    }

    fn try_into_parts(self) -> Option<Self::Payload> {
        let Self { payload } = self;
        payload
    }
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

    fn try_into_parts(self) -> Option<(ModalSource, Self::Payload)> {
        let Self { source, payload } = self;
        modal::ModalSource::from_i32(source)
            .and_then(|s| match s {
                modal::ModalSource::Unknown => None,
                modal::ModalSource::Command => Some(ModalSource::Command),
                modal::ModalSource::Component => Some(ModalSource::Component),
            })
            .zip(payload)
    }
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
