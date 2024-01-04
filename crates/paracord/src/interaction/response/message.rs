use std::{
    borrow::{Borrow, BorrowMut},
    convert::Infallible,
};

use qcore::{build_with::BuildWith, builder};
use serenity::{
    builder::{
        CreateAllowedMentions, CreateAttachment, CreateInteractionResponseFollowup,
        CreateInteractionResponseMessage, EditInteractionResponse,
    },
    model::id::{RoleId, UserId},
    utils::MessageBuilder,
};

use super::{Components, Embed, Embeds, MessageComponent, Prepare};

/// The body of a message
#[derive(Debug, qcore::Borrow)]
pub struct MessageBody<I, E = Infallible> {
    content: MessageBuilder,
    embeds: Embeds,
    ping_replied: bool,
    ping_users: Vec<UserId>,
    ping_roles: Vec<RoleId>,
    #[borrow(mut)]
    components: Components<MessageComponent<I, E>>,
}

macro_rules! build_body {
    ($self:expr, $builder:expr) => {{
        let MessageBody {
            mut content,
            embeds,
            ping_replied,
            ping_users,
            ping_roles,
            components,
        } = $self;
        $builder
            .content(content.build())
            .build_with(embeds)
            .allowed_mentions(
                CreateAllowedMentions::new()
                    .replied_user(ping_replied)
                    .users(ping_users)
                    .roles(ping_roles),
            )
            .build_with(components)
    }};
}

impl<I, E> MessageBody<I, E> {
    /// Construct a new rich-text message using the given closure
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

    /// Construct a new plaintext message
    #[inline]
    pub fn plain(c: impl Into<serenity::utils::Content>) -> Self {
        Self::rich(|mb| mb.push_safe(c))
    }
}

#[builder(trait_name = MessageBodyExt)]
/// Helper methods for mutating [`MessageBody`]
impl<I, E> MessageBody<I, E> {
    /// Set whether the replied-to user is allowed to be pinged
    pub fn ping_replied(&mut self, ping_replied: bool) { self.ping_replied = ping_replied; }

    /// Set which users are allowed to be pinged
    pub fn ping_users(&mut self, ping_users: Vec<UserId>) { self.ping_users = ping_users; }

    /// Set which guild roles are allowed to be pinged
    pub fn ping_roles(&mut self, ping_roles: Vec<RoleId>) { self.ping_roles = ping_roles; }

    /// Add an embed to this message
    pub fn embed(&mut self, embed: Embed) { self.embeds.0.push(embed); }

    /// Add an embed to this message using the given closure
    pub fn build_embed(&mut self, f: impl FnOnce(Embed) -> Embed) {
        self.embed(f(Embed::default()));
    }
}

impl<I, E> Prepare for MessageBody<I, E> {
    type Error = E;
    type Output = MessageBody<I, Infallible>;

    fn prepare(self) -> Result<Self::Output, Self::Error> {
        let Self {
            content,
            embeds,
            ping_replied,
            ping_users,
            ping_roles,
            components,
        } = self;
        Ok(MessageBody {
            content,
            embeds,
            ping_replied,
            ping_users,
            ping_roles,
            components: components.prepare()?,
        })
    }
}

impl<I> BuildWith<MessageBody<I>> for CreateInteractionResponseMessage {
    #[inline]
    fn build_with(self, value: MessageBody<I>) -> Self { build_body!(value, self) }
}

impl<I> BuildWith<MessageBody<I>> for EditInteractionResponse {
    #[inline]
    fn build_with(self, value: MessageBody<I>) -> Self { build_body!(value, self) }
}

impl<I> BuildWith<MessageBody<I>> for CreateInteractionResponseFollowup {
    #[inline]
    fn build_with(self, value: MessageBody<I>) -> Self { build_body!(value, self) }
}

/// Options to provide when creating (or deferring the creation of) a message
#[derive(Debug, Clone, Copy, Default)]
pub struct MessageOpts {
    tts: bool,
    ephemeral: bool,
}

macro_rules! build_opts {
    ($self:expr, $builder:expr) => {{
        let MessageOpts { tts, ephemeral } = $self;
        $builder.tts(tts).ephemeral(ephemeral)
    }};
}

#[builder(trait_name = MessageOptsExt)]
/// Helper methods for mutating [`MessageOpts`]
impl MessageOpts {
    /// Set whether this message should be read by screen readers
    pub fn tts(&mut self, tts: bool) { self.tts = tts; }

    /// Set whether this message should be a private temporary response
    pub fn ephemeral(&mut self, ephemeral: bool) { self.ephemeral = ephemeral; }
}

impl BuildWith<MessageOpts> for CreateInteractionResponseMessage {
    #[inline]
    fn build_with(self, value: MessageOpts) -> Self { build_opts!(value, self) }
}

impl BuildWith<MessageOpts> for CreateInteractionResponseFollowup {
    #[inline]
    fn build_with(self, value: MessageOpts) -> Self { build_opts!(value, self) }
}

/// A message
#[derive(Debug, qcore::Borrow)]
pub struct Message<I, E = Infallible> {
    #[borrow(mut)]
    body: MessageBody<I, E>,
    #[borrow(mut)]
    opts: MessageOpts,
    attachments: Vec<CreateAttachment>,
}

impl<I, E> Borrow<Components<MessageComponent<I, E>>> for Message<I, E> {
    fn borrow(&self) -> &Components<MessageComponent<I, E>> { &self.body.components }
}

impl<I, E> BorrowMut<Components<MessageComponent<I, E>>> for Message<I, E> {
    fn borrow_mut(&mut self) -> &mut Components<MessageComponent<I, E>> {
        &mut self.body.components
    }
}

macro_rules! build_msg {
    ($self:expr, $builder:expr) => {{
        let Message {
            body,
            opts,
            attachments,
        } = $self;
        $builder
            .build_with(body)
            .build_with(opts)
            .files(attachments)
    }};
}

impl<I, E> From<MessageBody<I, E>> for Message<I, E> {
    fn from(body: MessageBody<I, E>) -> Self {
        Self {
            body,
            opts: MessageOpts::default(),
            attachments: vec![],
        }
    }
}

impl<I, E> Message<I, E> {
    /// Construct a new rich-text message using the given closure
    #[inline]
    pub fn rich(f: impl FnOnce(&mut MessageBuilder) -> &mut MessageBuilder) -> Self {
        MessageBody::rich(f).into()
    }

    /// Construct a new plaintext message
    #[inline]
    pub fn plain(c: impl Into<serenity::utils::Content>) -> Self { MessageBody::plain(c).into() }

    /// Construct a new message from its constituent parts
    #[inline]
    #[must_use]
    pub fn from_parts(
        body: MessageBody<I, E>,
        opts: MessageOpts,
        attachments: Vec<CreateAttachment>,
    ) -> Self {
        Self {
            body,
            opts,
            attachments,
        }
    }
}

#[builder(trait_name = MessageExt)]
/// Helper methods for mutating [`Message`]
impl<'a, I, E> Message<I, E> {
    /// Add an attachment to this message
    pub fn attach(&mut self, attachments: impl IntoIterator<Item = CreateAttachment>) {
        self.attachments.extend(attachments);
    }
}

impl<I, E> Prepare for Message<I, E> {
    type Error = E;
    type Output = Message<I, Infallible>;

    fn prepare(self) -> Result<Self::Output, Self::Error> {
        let Self {
            body,
            opts,
            attachments,
        } = self;
        Ok(Message {
            body: body.prepare()?,
            opts,
            attachments,
        })
    }
}

impl<I> BuildWith<Message<I>> for CreateInteractionResponseMessage {
    #[inline]
    fn build_with(self, value: Message<I>) -> Self { build_msg!(value, self) }
}

impl<I> BuildWith<Message<I>> for CreateInteractionResponseFollowup {
    #[inline]
    fn build_with(self, value: Message<I>) -> Self { build_msg!(value, self) }
}
