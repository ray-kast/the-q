use serenity::builder::CreateInteractionResponseData;

use super::{id, Components, ResponseData};
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

#[derive(Debug)]
pub struct Modal {
    id: String,
    title: String,
    components: Components,
}

impl Modal {
    #[inline]
    pub fn new(
        source: ModalSource,
        payload: modal::modal::Payload,
        title: impl Into<String>,
    ) -> Result<Self, id::Error> {
        Ok(Self {
            id: id::write(&modal::Modal {
                source: modal::ModalSource::from(source.0) as i32,
                payload: Some(payload),
            })?,
            title: title.into(),
            components: Components::default(),
        })
    }
}

impl<'a> ResponseData<'a> for Modal {
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
        components.build_response_data(data.custom_id(id).title(title))
    }
}
