use qcore::{
    build_with::{BuildWith, BuilderHelpers},
    builder,
};
use serenity::{
    builder::{
        CreateEmbed, CreateInteractionResponseFollowup, CreateInteractionResponseMessage,
        EditInteractionResponse,
    },
    model::Color,
    utils::MessageBuilder,
};
use url::Url;

use super::{prelude::*, Message, MessageBody};

#[derive(Debug, Default)]
pub(super) struct Embeds(pub(super) Vec<Embed>);

macro_rules! build_embeds {
    ($self:expr, $builder:expr) => {{
        let Embeds(embeds) = $self;
        $builder.embeds(embeds.into_iter().map(Into::into).collect())
    }};
}

impl BuildWith<Embeds> for CreateInteractionResponseMessage {
    #[inline]
    fn build_with(self, value: Embeds) -> Self { build_embeds!(value, self) }
}

impl BuildWith<Embeds> for EditInteractionResponse {
    #[inline]
    fn build_with(self, value: Embeds) -> Self { build_embeds!(value, self) }
}

impl BuildWith<Embeds> for CreateInteractionResponseFollowup {
    #[inline]
    fn build_with(self, value: Embeds) -> Self { build_embeds!(value, self) }
}

/// A message rich content embed
#[derive(Debug, Default)]
pub struct Embed {
    title: Option<String>,
    desc: Option<String>,
    url: Option<Url>,
    timestamp: Option<chrono::DateTime<chrono::Utc>>,
    color: Option<Color>,
    footer: Option<EmbedFooter>,
    image: Option<EmbedImage>,
    thumbnail: Option<EmbedThumbnail>,
    video: Option<EmbedVideo>,
    provider: Option<EmbedProvider>,
    author: Option<EmbedAuthor>,
    fields: Vec<EmbedField>,
}

impl<I, E> From<Embed> for MessageBody<I, E> {
    fn from(embed: Embed) -> Self { MessageBody::plain("").embed(embed) }
}

impl<I, E> From<Embed> for Message<I, E> {
    fn from(value: Embed) -> Self { MessageBody::from(value).into() }
}

impl From<Embed> for CreateEmbed {
    fn from(value: Embed) -> Self {
        let Embed {
            title,
            desc,
            url,
            timestamp,
            color,
            footer,
            image,
            thumbnail,
            video,
            provider,
            author,
            fields,
        } = value;

        CreateEmbed::new()
            .fold_opt(title, CreateEmbed::title)
            .fold_opt(desc, CreateEmbed::description)
            .fold_opt(url, CreateEmbed::url)
            .fold_opt(timestamp, CreateEmbed::timestamp)
            .fold_opt(color, CreateEmbed::color)
            .build_with_opt(footer)
            .build_with_opt(image)
            .build_with_opt(thumbnail)
            .build_with_opt(video)
            .build_with_opt(provider)
            .build_with_opt(author)
            .build_with_iter(fields)
    }
}

#[builder(trait_name = EmbedExt)]
/// Helper methods for mutating [`Embed`]
impl Embed {
    /// Set the title of this embed
    // TODO: does the title support markdown?
    pub fn title(&mut self, title: impl Into<String>) { self.title = Some(title.into()); }

    /// Set the description of this embed using the given closure
    pub fn desc_rich(&mut self, f: impl FnOnce(&mut MessageBuilder) -> &mut MessageBuilder) {
        let mut desc = MessageBuilder::new();
        f(&mut desc);
        self.desc = Some(desc.0);
    }

    /// Set the description of this embed to a simple string
    pub fn desc_plain(&mut self, c: impl Into<serenity::utils::Content>) {
        self.desc_rich(|mb| mb.push_safe(c));
    }

    /// Set the URL of this embed
    pub fn url(&mut self, url: impl Into<Url>) { self.url = Some(url.into()); }

    /// Set the timestamp of this embed
    pub fn timestamp(&mut self, ts: impl Into<chrono::DateTime<chrono::Utc>>) {
        self.timestamp = Some(ts.into());
    }

    /// Set the primary color of this embed
    pub fn color(&mut self, color: impl Into<Color>) { self.color = Some(color.into()); }

    /// Set (or reset) the primary color of this embed
    pub fn color_opt(&mut self, color: Option<impl Into<Color>>) {
        self.color = color.map(Into::into);
    }

    // TODO: footer, image, thumbnail, video, provider, author, fields
}

#[derive(Debug)]
struct EmbedFooter {}
impl BuildWith<EmbedFooter> for CreateEmbed {
    fn build_with(self, _: EmbedFooter) -> Self {
        self // TODO
    }
}
#[derive(Debug)]
struct EmbedImage {}
impl BuildWith<EmbedImage> for CreateEmbed {
    fn build_with(self, _: EmbedImage) -> Self { todo!() }
}
#[derive(Debug)]
struct EmbedThumbnail {}
impl BuildWith<EmbedThumbnail> for CreateEmbed {
    fn build_with(self, _: EmbedThumbnail) -> Self { todo!() }
}
#[derive(Debug)]
struct EmbedVideo {}
impl BuildWith<EmbedVideo> for CreateEmbed {
    fn build_with(self, _: EmbedVideo) -> Self { todo!() }
}
#[derive(Debug)]
struct EmbedProvider {}
impl BuildWith<EmbedProvider> for CreateEmbed {
    fn build_with(self, _: EmbedProvider) -> Self { todo!() }
}
#[derive(Debug)]
struct EmbedAuthor {}
impl BuildWith<EmbedAuthor> for CreateEmbed {
    fn build_with(self, _: EmbedAuthor) -> Self { todo!() }
}
#[derive(Debug)]
struct EmbedField {}
impl BuildWith<EmbedField> for CreateEmbed {
    fn build_with(self, _: EmbedField) -> Self { todo!() }
}
