use serenity::{
    builder::{CreateInteractionResponse, CreateInteractionResponseData, EditInteractionResponse},
    model::{
        application::interaction::InteractionResponseType,
        id::{RoleId, UserId},
    },
    utils::MessageBuilder,
};

use super::super::handler;

#[derive(Debug)]
pub struct MessageBody {
    content: MessageBuilder,
    ping_replied: bool,
    ping_users: Vec<UserId>,
    ping_roles: Vec<RoleId>,
}

impl MessageBody {
    #[inline]
    pub fn rich(f: impl FnOnce(&mut MessageBuilder) -> &mut MessageBuilder) -> Self {
        let mut content = MessageBuilder::new();
        f(&mut content);
        Self {
            content,
            ping_replied: false,
            ping_users: vec![],
            ping_roles: vec![],
        }
    }

    #[inline]
    pub fn plain(c: impl Into<serenity::utils::Content>) -> Self {
        Self::rich(|mb| mb.push_safe(c))
    }

    pub fn ping_replied(self, ping_replied: bool) -> Self {
        Self {
            ping_replied,
            ..self
        }
    }

    pub fn ping_users(self, ping_users: Vec<UserId>) -> Self { Self { ping_users, ..self } }

    pub fn ping_roles(self, ping_roles: Vec<RoleId>) -> Self { Self { ping_roles, ..self } }

    pub fn into_err(self, msg: &'static str) -> handler::DeferError<MessageBody> {
        handler::DeferError::Response(msg, self)
    }

    pub fn build_edit_response(
        self,
        res: &mut EditInteractionResponse,
    ) -> &mut EditInteractionResponse {
        let Self {
            content,
            ping_replied,
            ping_users,
            ping_roles,
        } = self;
        res.content(content).allowed_mentions(|m| {
            m.replied_user(ping_replied)
                .users(ping_users)
                .roles(ping_roles)
        })
    }
}

#[derive(Debug, Default)]
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
    fn body(self, f: impl FnOnce(MessageBody) -> MessageBody) -> Self {
        Self {
            body: f(self.body),
            ..self
        }
    }

    #[inline]
    pub fn ping_replied(self, ping_replied: bool) -> Self {
        self.body(|b| b.ping_replied(ping_replied))
    }

    #[inline]
    pub fn ping_users(self, ping_users: Vec<UserId>) -> Self {
        self.body(|b| b.ping_users(ping_users))
    }

    #[inline]
    pub fn ping_roles(self, ping_roles: Vec<RoleId>) -> Self {
        self.body(|b| b.ping_roles(ping_roles))
    }

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
            body:
                MessageBody {
                    content,
                    ping_replied,
                    ping_users,
                    ping_roles,
                },
            opts,
        } = self;

        res.kind(InteractionResponseType::ChannelMessageWithSource)
            .interaction_response_data(|d| {
                opts.build_response_data(d.content(content).allowed_mentions(|m| {
                    m.replied_user(ping_replied)
                        .users(ping_users)
                        .roles(ping_roles)
                }))
            })
    }
}
