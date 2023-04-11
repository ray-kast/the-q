use qcore::builder;
use serenity::{
    builder::{
        CreateEmbed, CreateInteractionResponseData, CreateInteractionResponseFollowup,
        EditInteractionResponse,
    },
    utils::{Color, MessageBuilder},
};
use url::Url;

use super::{prelude::*, Message, MessageBody, ResponseData};

#[derive(Debug, Default)]
pub(super) struct Embeds(pub(super) Vec<Embed>);

macro_rules! build_embeds {
    ($self:expr, $builder:expr) => {{
        let Self(embeds) = $self;
        embeds.into_iter().fold($builder, |b, e| {
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
            } = e;
            b.embed(|b| {
                visit(title, b, CreateEmbed::title);
                visit(desc, b, CreateEmbed::description);
                visit(url, b, CreateEmbed::url);
                visit(timestamp, b, CreateEmbed::timestamp);
                visit(color, b, CreateEmbed::color);
                visit_build(footer, b);
                visit_build(image, b);
                visit_build(thumbnail, b);
                visit_build(video, b);
                visit_build(provider, b);
                visit_build(author, b);
                visit_build(fields, b)
            })
        })
    }};
}

#[inline]
fn visit<V: IntoIterator>(
    vals: V,
    embed: &mut CreateEmbed,
    f: impl FnMut(&mut CreateEmbed, V::Item) -> &mut CreateEmbed,
) -> &mut CreateEmbed {
    vals.into_iter().fold(embed, f)
}

#[inline]
fn visit_build<V: IntoIterator>(vals: V, embed: &mut CreateEmbed) -> &mut CreateEmbed
where V::Item: BuildEmbed {
    for v in vals {
        v.build_embed(embed);
    }
    embed
}

impl Embeds {
    #[inline]
    pub(super) fn build_edit_response(
        self,
        res: &mut EditInteractionResponse,
    ) -> &mut EditInteractionResponse {
        build_embeds!(self, res)
    }

    #[inline]
    pub(super) fn build_followup<'a, 'b>(
        self,
        fup: &'b mut CreateInteractionResponseFollowup<'a>,
    ) -> &'b mut CreateInteractionResponseFollowup<'a> {
        build_embeds!(self, fup)
    }
}

impl<'a> ResponseData<'a> for Embeds {
    fn build_response_data<'b>(
        self,
        data: &'b mut CreateInteractionResponseData<'a>,
    ) -> &'b mut CreateInteractionResponseData<'a> {
        build_embeds!(self, data)
    }
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

impl<'a, I, E> From<Embed> for Message<'a, I, E> {
    fn from(value: Embed) -> Self { MessageBody::from(value).into() }
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

trait BuildEmbed {
    fn build_embed(self, embed: &mut CreateEmbed) -> &mut CreateEmbed;
}

#[derive(Debug)]
struct EmbedFooter {}
impl BuildEmbed for EmbedFooter {
    fn build_embed(self, embed: &mut CreateEmbed) -> &mut CreateEmbed {
        embed // TODO
    }
}
#[derive(Debug)]
struct EmbedImage {}
impl BuildEmbed for EmbedImage {
    fn build_embed(self, embed: &mut CreateEmbed) -> &mut CreateEmbed {
        embed // TODO
    }
}
#[derive(Debug)]
struct EmbedThumbnail {}
impl BuildEmbed for EmbedThumbnail {
    fn build_embed(self, embed: &mut CreateEmbed) -> &mut CreateEmbed {
        embed // TODO
    }
}
#[derive(Debug)]
struct EmbedVideo {}
impl BuildEmbed for EmbedVideo {
    fn build_embed(self, embed: &mut CreateEmbed) -> &mut CreateEmbed {
        embed // TODO
    }
}
#[derive(Debug)]
struct EmbedProvider {}
impl BuildEmbed for EmbedProvider {
    fn build_embed(self, embed: &mut CreateEmbed) -> &mut CreateEmbed {
        embed // TODO
    }
}
#[derive(Debug)]
struct EmbedAuthor {}
impl BuildEmbed for EmbedAuthor {
    fn build_embed(self, embed: &mut CreateEmbed) -> &mut CreateEmbed {
        embed // TODO
    }
}
#[derive(Debug)]
struct EmbedField {}
impl BuildEmbed for EmbedField {
    fn build_embed(self, embed: &mut CreateEmbed) -> &mut CreateEmbed {
        embed // TODO
    }
}
