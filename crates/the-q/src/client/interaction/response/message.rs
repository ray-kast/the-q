use qcore::builder;
use serenity::{
    builder::{
        CreateInteractionResponseData, CreateInteractionResponseFollowup, EditInteractionResponse,
    },
    model::{
        id::{RoleId, UserId},
        prelude::AttachmentType,
    },
    utils::MessageBuilder,
};

use super::{Components, Embed, Embeds, ResponseData};

#[derive(Debug)]
pub struct MessageBody {
    content: MessageBuilder,
    embeds: Embeds,
    ping_replied: bool,
    ping_users: Vec<UserId>,
    ping_roles: Vec<RoleId>,
    components: Components,
}

macro_rules! build_body {
    ($self:expr, $builder:expr, $fn:ident) => {{
        let Self {
            content,
            embeds,
            ping_replied,
            ping_users,
            ping_roles,
            components,
        } = $self;
        components.$fn(embeds.$fn($builder.content(content)).allowed_mentions(|m| {
            m.replied_user(ping_replied)
                .users(ping_users)
                .roles(ping_roles)
        }))
    }};
}

impl MessageBody {
    #[inline]
    pub fn rich(f: impl FnOnce(&mut MessageBuilder) -> &mut MessageBuilder) -> Self {
        let mut content = MessageBuilder::new();
        f(&mut content);
        Self {
            content,
            embeds: Embeds::default(),
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
        fup: &'b mut CreateInteractionResponseFollowup<'a>,
    ) -> &'b mut CreateInteractionResponseFollowup<'a> {
        build_body!(self, fup, build_followup)
    }
}

#[builder(trait_name = "MessageBodyExt")]
impl MessageBody {
    pub fn ping_replied(&mut self, ping_replied: bool) { self.ping_replied = ping_replied; }

    pub fn ping_users(&mut self, ping_users: Vec<UserId>) { self.ping_users = ping_users; }

    pub fn ping_roles(&mut self, ping_roles: Vec<RoleId>) { self.ping_roles = ping_roles; }

    pub fn embed(&mut self, embed: Embed) { self.embeds.0.push(embed); }

    pub fn build_embed(&mut self, f: impl FnOnce(Embed) -> Embed) {
        self.embed(f(Embed::default()));
    }
}

impl<'a> ResponseData<'a> for MessageBody {
    #[inline]
    fn build_response_data<'b>(
        self,
        data: &'b mut CreateInteractionResponseData<'a>,
    ) -> &'b mut CreateInteractionResponseData<'a> {
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

impl MessageOpts {
    #[inline]
    pub fn new() -> Self { Self::default() }

    #[inline]
    fn build_followup<'a, 'b>(
        self,
        fup: &'a mut CreateInteractionResponseFollowup<'b>,
    ) -> &'a mut CreateInteractionResponseFollowup<'b> {
        build_opts!(self, fup)
    }
}

#[builder(trait_name = "MessageOptsExt")]
impl MessageOpts {
    pub fn tts(&mut self, tts: bool) { self.tts = tts; }

    pub fn ephemeral(&mut self, ephemeral: bool) { self.ephemeral = ephemeral; }
}

impl<'a> ResponseData<'a> for MessageOpts {
    #[inline]
    fn build_response_data<'b>(
        self,
        data: &'b mut CreateInteractionResponseData<'a>,
    ) -> &'b mut CreateInteractionResponseData<'a> {
        build_opts!(self, data)
    }
}

#[derive(Debug, qcore::Borrow)]
pub struct Message<'a> {
    #[borrow(mut)]
    body: MessageBody,
    #[borrow(mut)]
    opts: MessageOpts,
    attachments: Vec<AttachmentType<'a>>,
}

macro_rules! build_msg {
    ($self:expr, $builder:expr, $fn:ident) => {{
        let Self {
            body,
            opts,
            attachments,
        } = $self;
        opts.$fn(body.$fn($builder)).files(attachments)
    }};
}

impl<'a> From<MessageBody> for Message<'a> {
    fn from(body: MessageBody) -> Self {
        Self {
            body,
            opts: MessageOpts::default(),
            attachments: vec![],
        }
    }
}

impl<'a> Message<'a> {
    #[inline]
    pub fn rich(f: impl FnOnce(&mut MessageBuilder) -> &mut MessageBuilder) -> Self {
        MessageBody::rich(f).into()
    }

    #[inline]
    pub fn plain(c: impl Into<serenity::utils::Content>) -> Self { MessageBody::plain(c).into() }

    #[inline]
    pub fn from_parts(
        body: MessageBody,
        opts: MessageOpts,
        attachments: Vec<AttachmentType<'a>>,
    ) -> Self {
        Self {
            body,
            opts,
            attachments,
        }
    }

    pub fn build_followup<'b>(
        self,
        fup: &'b mut CreateInteractionResponseFollowup<'a>,
    ) -> &'b mut CreateInteractionResponseFollowup<'a> {
        build_msg!(self, fup, build_followup)
    }
}

#[builder(trait_name = "MessageExt")]
impl<'a> Message<'a> {
    pub fn attach(&mut self, attachments: impl IntoIterator<Item = AttachmentType<'a>>) {
        self.attachments.extend(attachments);
    }
}

impl<'a> ResponseData<'a> for Message<'a> {
    #[inline]
    fn build_response_data<'b>(
        self,
        data: &'b mut CreateInteractionResponseData<'a>,
    ) -> &'b mut CreateInteractionResponseData<'a> {
        build_msg!(self, data, build_response_data)
    }
}
