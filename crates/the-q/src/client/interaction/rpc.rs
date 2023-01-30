use super::response::ModalSource;
use crate::prelude::*;

pub trait Key: fmt::Debug + Copy + Eq + Ord + Hash + 'static
where for<'a> Self: From<&'a Self::Payload>
{
    type Payload;
    type Interaction;
}

pub trait Schema: fmt::Debug {
    type Component: ComponentId;
    type Modal: ModalId;
}

pub trait ComponentId: prost::Message {
    type Key: Key<Payload = Self::Payload>;
    type Payload;

    fn from_parts(payload: Self::Payload) -> Self;

    fn into_parts(self) -> Self::Payload;
}

pub trait ModalId: prost::Message {
    type Key: Key<Payload = Self::Payload>;
    type Payload;

    fn from_parts(src: ModalSource, payload: Self::Payload) -> Self;

    fn into_parts(self) -> (ModalSource, Self::Payload);
}
