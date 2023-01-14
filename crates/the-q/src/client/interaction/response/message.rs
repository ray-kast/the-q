use serenity::{
    builder::{
        CreateInteractionResponseData, CreateInteractionResponseFollowup, EditInteractionResponse,
    },
    model::id::{RoleId, UserId},
    utils::MessageBuilder,
};

use super::ResponseData;

// TODO: handle embeds and attachments
#[derive(Debug)]
pub struct MessageBody {
    content: MessageBuilder,
    ping_replied: bool,
    ping_users: Vec<UserId>,
    ping_roles: Vec<RoleId>,
}

macro_rules! build_body {
    ($self:expr, $builder:expr) => {{
        let Self {
            content,
            ping_replied,
            ping_users,
            ping_roles,
        } = $self;
        $builder.content(content).allowed_mentions(|m| {
            m.replied_user(ping_replied)
                .users(ping_users)
                .roles(ping_roles)
        })
    }};
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

    #[inline]
    pub fn build_edit_response(
        self,
        res: &mut EditInteractionResponse,
    ) -> &mut EditInteractionResponse {
        build_body!(self, res)
    }

    #[inline]
    pub fn build_followup<'a, 'b>(
        self,
        fup: &'a mut CreateInteractionResponseFollowup<'b>,
    ) -> &'a mut CreateInteractionResponseFollowup<'b> {
        build_body!(self, fup)
    }
}

impl ResponseData for MessageBody {
    #[inline]
    fn build_response_data<'a, 'b>(
        self,
        data: &'a mut CreateInteractionResponseData<'b>,
    ) -> &'a mut CreateInteractionResponseData<'b> {
        build_body!(self, data)
    }
}

#[derive(Debug, Default)]
pub struct MessageOpts {
    tts: bool,
    ephemeral: bool,
}

macro_rules! build_opts {
    ($self:expr, $builder:expr) => {{
        let Self { tts, ephemeral } = $self;
        $builder.tts(tts).ephemeral(ephemeral)
    }};
}

impl MessageOpts {
    #[inline]
    pub fn new() -> Self { Self::default() }

    pub fn tts(self, tts: bool) -> Self { Self { tts, ..self } }

    pub fn ephemeral(self, ephemeral: bool) -> Self { Self { ephemeral, ..self } }

    #[inline]
    fn build_followup<'a, 'b>(
        self,
        fup: &'a mut CreateInteractionResponseFollowup<'b>,
    ) -> &'a mut CreateInteractionResponseFollowup<'b> {
        build_opts!(self, fup)
    }
}

impl ResponseData for MessageOpts {
    #[inline]
    fn build_response_data<'a, 'b>(
        self,
        data: &'a mut CreateInteractionResponseData<'b>,
    ) -> &'a mut CreateInteractionResponseData<'b> {
        build_opts!(self, data)
    }
}

#[derive(Debug)]
pub struct Message {
    body: MessageBody,
    opts: MessageOpts,
}

macro_rules! build_msg {
    ($self:expr, $builder:expr, $fn:ident) => {{
        let Self { body, opts } = $self;
        opts.$fn(body.$fn($builder))
    }};
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

    pub fn build_followup<'a, 'b>(
        self,
        fup: &'a mut CreateInteractionResponseFollowup<'b>,
    ) -> &'a mut CreateInteractionResponseFollowup<'b> {
        build_msg!(self, fup, build_followup)
    }
}

impl ResponseData for Message {
    #[inline]
    fn build_response_data<'a, 'b>(
        self,
        data: &'a mut CreateInteractionResponseData<'b>,
    ) -> &'a mut CreateInteractionResponseData<'b> {
        build_msg!(self, data, build_response_data)
    }
}
