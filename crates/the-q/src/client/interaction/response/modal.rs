use serenity::builder::CreateInteractionResponseData;

use super::{id, ResponseData};
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
        })
    }
}

impl ResponseData for Modal {
    #[inline]
    fn build_response_data<'a, 'b>(
        self,
        data: &'a mut CreateInteractionResponseData<'b>,
    ) -> &'a mut CreateInteractionResponseData<'b> {
        let Self { id, title } = self;
        data.custom_id(id).title(title).components(|c| {
            c.create_action_row(|r| {
                r.create_input_text(|t| {
                    t.custom_id("todo")
                        .style(serenity::model::prelude::component::InputTextStyle::Paragraph)
                        .label("todo")
                })
            })
        })
    }
}
