use std::{convert::Infallible, marker::PhantomData};

use qcore::build_with::BuildWith;
use serenity::builder::CreateModal;

use super::{
    super::rpc::{ModalId, Schema},
    id, Components, Prepare, TextInput,
};

/// A predetermined modal source, dictated by the interaction currently being
/// responded to
#[derive(Debug, Clone, Copy)]
pub struct ModalSourceHandle(pub(super) ModalSource);

#[derive(Debug, Clone, Copy)]
#[doc(hidden)]
pub enum ModalSource {
    Command,
    Component,
}

/// A modal dialog
#[derive(Debug, qcore::Borrow)]
pub struct Modal<S: Schema, E> {
    id: Result<id::Id<'static>, E>,
    title: String,
    #[borrow(mut)]
    components: Components<TextInput<S::Component, E>>,
    key: PhantomData<fn(S)>,
}

impl<S: Schema> Modal<S, id::Error> {
    /// Construct a new modal
    #[inline]
    pub fn new(
        source: ModalSourceHandle,
        payload: <S::Modal as ModalId>::Payload,
        title: impl Into<String>,
    ) -> Self {
        Self {
            id: id::write(&S::Modal::from_parts(source.0, payload)),
            title: title.into(),
            components: Components::default(),
            key: PhantomData,
        }
    }
}

impl<S: Schema, E> Prepare for Modal<S, E> {
    type Error = E;
    type Output = Modal<S, Infallible>;

    #[inline]
    fn prepare(self) -> Result<Self::Output, Self::Error> {
        let Self {
            id,
            title,
            components,
            key,
        } = self;
        Ok(Modal {
            id: Ok(id?),
            title,
            components: components.prepare()?,
            key,
        })
    }
}

impl<S: Schema> From<Modal<S, Infallible>> for CreateModal {
    #[inline]
    fn from(value: Modal<S, Infallible>) -> Self {
        let Modal {
            id,
            title,
            components,
            key: _,
        } = value;

        Self::new(id.unwrap_or_else(|_| unreachable!()).to_string(), title).build_with(components)
    }
}
