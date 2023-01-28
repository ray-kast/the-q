use serenity::builder::CreateInteractionResponseData;

use super::{id, Components, ResponseData, TextInput};
use crate::{prelude::*, proto::modal};

#[derive(Debug, Clone, Copy)]
pub struct ModalSource(pub(super) Source);

#[derive(Debug, Clone, Copy)]
#[doc(hidden)]
pub enum Source {
    Command,
    Component,
}

impl From<Source> for modal::ModalSource {
    fn from(val: Source) -> Self {
        match val {
            Source::Command => modal::ModalSource::Command,
            Source::Component => modal::ModalSource::Component,
        }
    }
}

#[derive(Debug, qcore::Borrow)]
pub struct Modal<E> {
    id: Result<id::Id<'static>, E>,
    title: String,
    #[borrow(mut)]
    components: Components<TextInput, E>,
}

impl Modal<id::Error> {
    #[inline]
    pub fn new(
        source: ModalSource,
        payload: modal::modal::Payload,
        title: impl Into<String>,
    ) -> Self {
        Self {
            id: id::write(&modal::Modal {
                source: modal::ModalSource::from(source.0) as i32,
                payload: Some(payload),
            }),
            title: title.into(),
            components: Components::default(),
        }
    }
}

impl<E> Modal<E> {
    #[inline]
    pub fn prepare(self) -> Result<Modal<Infallible>, E> {
        let Self {
            id,
            title,
            components,
        } = self;
        Ok(Modal {
            id: Ok(id?),
            title,
            components: components.prepare()?,
        })
    }
}

impl<'a> ResponseData<'a> for Modal<Infallible> {
    #[inline]
    fn build_response_data<'b>(
        self,
        data: &'b mut CreateInteractionResponseData<'a>,
    ) -> &'b mut CreateInteractionResponseData<'a> {
        let Self {
            id,
            title,
            components,
        } = self;
        components.build_response_data(
            // TODO: use into_ok
            data.custom_id(id.unwrap_or_else(|_| unreachable!()))
                .title(title),
        )
    }
}
