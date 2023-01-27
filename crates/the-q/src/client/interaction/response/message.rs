use qcore::builder;
use serenity::{
    builder::{
        CreateInteractionResponseData, CreateInteractionResponseFollowup, EditInteractionResponse,
    },
    model::id::{RoleId, UserId},
    utils::MessageBuilder,
};

use super::{Components, ResponseData};

// TODO: handle embeds and attachments
#[derive(Debug)]
pub struct MessageBody {
    content: MessageBuilder,
    ping_replied: bool,
    ping_users: Vec<UserId>,
    ping_roles: Vec<RoleId>,
    components: Components,
}

macro_rules! build_body {
    ($self:expr, $builder:expr, $fn:ident) => {{
        let Self {
            content,
            ping_replied,
            ping_users,
            ping_roles,
            components,
        } = $self;
        components.$fn($builder.content(content).allowed_mentions(|m| {
            m.replied_user(ping_replied)
                .users(ping_users)
                .roles(ping_roles)
        }))
    }};
}

#[builder(trait_name = "MessageBodyExt")]
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
            components: Components::default(),
        }
    }

    #[inline]
    pub fn plain(c: impl Into<serenity::utils::Content>) -> Self {
        Self::rich(|mb| mb.push_safe(c))
    }

    pub fn ping_replied(&mut self, ping_replied: bool) { self.ping_replied = ping_replied; }

    pub fn ping_users(&mut self, ping_users: Vec<UserId>) { self.ping_users = ping_users; }

    pub fn ping_roles(&mut self, ping_roles: Vec<RoleId>) { self.ping_roles = ping_roles; }

    #[inline]
    pub fn build_edit_response(
        self,
        res: &mut EditInteractionResponse,
    ) -> &mut EditInteractionResponse {
        build_body!(self, res, build_edit_response)
    }

    #[inline]
    pub fn build_followup<'a, 'b>(
        self,
        fup: &'a mut CreateInteractionResponseFollowup<'b>,
    ) -> &'a mut CreateInteractionResponseFollowup<'b> {
        build_body!(self, fup, build_followup)
    }
}

impl ResponseData for MessageBody {
    #[inline]
    fn build_response_data<'a, 'b>(
        self,
        data: &'a mut CreateInteractionResponseData<'b>,
    ) -> &'a mut CreateInteractionResponseData<'b> {
        build_body!(self, data, build_response_data)
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

#[builder(trait_name = "MessageOptsExt")]
impl MessageOpts {
    #[inline]
    pub fn new() -> Self { Self::default() }

    pub fn tts(&mut self, tts: bool) { self.tts = tts; }

    pub fn ephemeral(&mut self, ephemeral: bool) { self.ephemeral = ephemeral; }

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

qcore::borrow!(Message { mut body: MessageBody });
qcore::borrow!(Message { mut opts: MessageOpts });

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

#[builder(trait_name = "MessageExt")]
impl Message {
    #[inline]
    pub fn rich(f: impl FnOnce(&mut MessageBuilder) -> &mut MessageBuilder) -> Self {
        MessageBody::rich(f).into()
    }

    #[inline]
    pub fn plain(c: impl Into<serenity::utils::Content>) -> Self { MessageBody::plain(c).into() }

    #[inline]
    pub fn from_parts(body: MessageBody, opts: MessageOpts) -> Self { Self { body, opts } }

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
