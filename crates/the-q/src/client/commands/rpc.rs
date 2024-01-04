use serenity::model::application::{ComponentInteraction, ModalInteraction};

use super::prelude::*;

#[derive(Debug)]
pub enum Schema {}

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
    Soundboard,
}

impl From<&ComponentPayload> for ComponentKey {
    fn from(value: &ComponentPayload) -> Self {
        match value {
            ComponentPayload::Role(_) => Self::Role,
            ComponentPayload::Soundboard(_) => Self::Soundboard,
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
        source
            .try_into()
            .ok()
            .and_then(|s| match s {
                modal::ModalSource::Unknown => None,
                modal::ModalSource::Command => Some(ModalSource::Command),
                modal::ModalSource::Component => Some(ModalSource::Component),
            })
            .zip(payload)
    }
}

impl rpc::Key for ComponentKey {
    type Interaction = ComponentInteraction;
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
    type Interaction = ModalInteraction;
    type Payload = ModalPayload;
}
