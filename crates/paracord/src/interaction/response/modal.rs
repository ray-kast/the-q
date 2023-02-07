use std::{convert::Infallible, marker::PhantomData};

use serenity::builder::CreateInteractionResponseData;

use super::{
    super::rpc::{ModalId, Schema},
    id, Components, ResponseData, TextInput,
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
    components: Components<S::Component, TextInput<S::Component>, E>,
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
            key: PhantomData::default(),
        }
    }
}

impl<S: Schema, E> Modal<S, E> {
    /// Purge any validation errors caused during initialization
    ///
    /// # Errors
    /// If the modal or any of its components contain an error it will be
    /// returned.
    #[inline]
    pub fn prepare(self) -> Result<Modal<S, Infallible>, E> {
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

impl<'a, S: Schema> ResponseData<'a> for Modal<S, Infallible> {
    #[inline]
    fn build_response_data<'b>(
        self,
        data: &'b mut CreateInteractionResponseData<'a>,
    ) -> &'b mut CreateInteractionResponseData<'a> {
        let Self {
            id,
            title,
            components,
            key: _,
        } = self;
        components.build_response_data(
            // TODO: use into_ok
            data.custom_id(id.unwrap_or_else(|_| unreachable!()))
                .title(title),
        )
    }
}
