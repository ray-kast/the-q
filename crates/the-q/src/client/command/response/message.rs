use serenity::{
    builder::{CreateInteractionResponse, CreateInteractionResponseData, EditInteractionResponse},
    model::application::interaction::InteractionResponseType,
    utils::MessageBuilder,
};

use super::super::handler;

#[derive(Debug)]
pub struct MessageBody {
    content: MessageBuilder,
}

impl MessageBody {
    #[inline]
    pub fn rich(f: impl FnOnce(&mut MessageBuilder) -> &mut MessageBuilder) -> Self {
        let mut content = MessageBuilder::new();
        f(&mut content);
        Self { content }
    }

    #[inline]
    pub fn plain(c: impl Into<serenity::utils::Content>) -> Self {
        Self::rich(|mb| mb.push_safe(c))
    }

    pub fn into_err(self, msg: &'static str) -> handler::DeferError<MessageBody> {
        handler::DeferError::Response(msg, self)
    }

    pub fn build_edit_response(
        self,
        res: &mut EditInteractionResponse,
    ) -> &mut EditInteractionResponse {
        let Self { content } = self;
        res.content(content)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MessageOpts {
    tts: bool,
    ephemeral: bool,
}

impl MessageOpts {
    #[inline]
    pub fn new() -> Self { Self::default() }

    pub fn tts(self, tts: bool) -> Self { Self { tts, ..self } }

    pub fn ephemeral(self, ephemeral: bool) -> Self { Self { ephemeral, ..self } }

    pub fn build_response_data<'a, 'b>(
        self,
        data: &'a mut CreateInteractionResponseData<'b>,
    ) -> &'a mut CreateInteractionResponseData<'b> {
        let Self { tts, ephemeral } = self;
        data.tts(tts).ephemeral(ephemeral)
    }
}

#[derive(Debug)]
pub struct Message {
    body: MessageBody,
    opts: MessageOpts,
}

impl From<MessageBody> for Message {
    fn from(body: MessageBody) -> Self {
        Self {
            body,
            opts: MessageOpts::default(),
        }
    }
}

impl Message {
    #[inline]
    pub fn rich(f: impl FnOnce(&mut MessageBuilder) -> &mut MessageBuilder) -> Self {
        MessageBody::rich(f).into()
    }

    #[inline]
    pub fn plain(c: impl Into<serenity::utils::Content>) -> Self { MessageBody::plain(c).into() }

    #[inline]
    pub fn from_parts(body: MessageBody, opts: MessageOpts) -> Self { Self { body, opts } }

    #[inline]
    fn opt(self, f: impl FnOnce(MessageOpts) -> MessageOpts) -> Self {
        Self {
            opts: f(self.opts),
            ..self
        }
    }

    #[inline]
    pub fn tts(self, tts: bool) -> Self { self.opt(|o| o.tts(tts)) }

    #[inline]
    pub fn ephemeral(self, ephemeral: bool) -> Self { self.opt(|o| o.ephemeral(ephemeral)) }

    pub fn into_err(self, msg: &'static str) -> handler::Error {
        handler::Error::Response(msg, self)
    }

    pub fn build_response<'a, 'b>(
        self,
        res: &'a mut CreateInteractionResponse<'b>,
    ) -> &'a mut CreateInteractionResponse<'b> {
        let Self {
            body: MessageBody { content },
            opts: MessageOpts { tts, ephemeral },
        } = self;

        res.kind(InteractionResponseType::ChannelMessageWithSource)
            .interaction_response_data(|d| d.content(content).tts(tts).ephemeral(ephemeral))
    }
}
