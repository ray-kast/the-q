use std::path::PathBuf;

use jpeggr::image::{self, ColorType, DynamicImage, ImageFormat};
use paracord::interaction::{
    handler::{CommandVisitor, IntoErr},
    response::{prelude::*, Message, MessageOpts},
};
use serenity::{builder::CreateAttachment, model::channel::Attachment};

use crate::{
    prelude::*,
    util::{
        http_client,
        interaction::{CommandResponder, CommandResult},
    },
};

enum ImageInput<'a> {
    Attachment(&'a Attachment),
    Url(Url),
}

async fn process<
    F: FnOnce(DynamicImage) -> FR + Send + 'static,
    FR: anyhow::Context<DynamicImage, FE>,
    FE,
>(
    input: ImageInput<'_>,
    lossless_out: bool,
    f: F,
) -> Result<Vec<u8>> {
    let image_data;
    let content_type;
    let filename;
    match input {
        ImageInput::Attachment(a) => {
            image_data = a
                .download()
                .await
                .context("Error downloading attachment from discord")?;
            content_type = a.content_type.clone();
            filename = Some(a.filename.clone());
        },
        ImageInput::Url(u) => {
            let res = http_client(None)
                .get(u)
                .send()
                .await
                .context("Error fetching input URL")?;
            content_type = res
                .headers()
                .get("Content-Type")
                .and_then(|h| h.to_str().ok())
                .map(ToOwned::to_owned);
            image_data = res
                .bytes()
                .await
                .context("Error downloading image response")?
                .to_vec();
            filename = None;
        },
    }

    tokio::task::spawn_blocking(move || {
        let image = [
            content_type.as_ref().and_then(ImageFormat::from_mime_type),
            filename.and_then(|f| ImageFormat::from_path(f).ok()),
            image::guess_format(&image_data).ok(),
        ]
        .into_iter()
        .find_map(|f| {
            let f = f?;
            image::load_from_memory_with_format(&image_data, f)
                .with_context(|| format!("Unable to load image with format {f:?}"))
                .map_err(|err| trace!(?err))
                .ok()
        })
        .or_else(|| {
            webp::Decoder::new(&image_data)
                .decode()
                .map(|i| i.to_image())
        })
        .or_else(|| {
            webp::AnimDecoder::new(&image_data)
                .decode()
                .map_err(|err| trace!(%err, "Error loading image as animated WebP"))
                .ok()
                .as_ref()
                .and_then(|i| i.get_frame(0))
                .as_ref()
                .map(Into::into)
        })
        .ok_or_else(|| anyhow!("Unable to determine input image format"))?;

        let out = f(image).context("Error processing image")?;
        let bytes;

        {
            let image = match out.color() {
                ColorType::L8 | ColorType::L16 => out.into_luma8().into(),
                ColorType::La8 | ColorType::La16 => out.into_luma_alpha8().into(),
                ColorType::Rgb8 | ColorType::Rgb16 | ColorType::Rgb32F => out.into_rgb8().into(),
                _ => out.into_rgba8().into(),
            };

            let enc = webp::Encoder::from_image(&image)
                .map_err(|e| anyhow!(e.to_string()))
                .context("Error creating WebP encoder")?;

            let mem = enc
                .encode_advanced(&webp::WebPConfig {
                    lossless: lossless_out.into(),
                    quality: 90.0,
                    target_size: 8 << 20,
                    ..webp::WebPConfig::new().expect("Unable to create base WebP config")
                })
                .map_err(|e| anyhow!("{e:?}"))
                .context("Error encoding output image")?;

            bytes = mem.to_vec();
        }

        Ok(bytes)
    })
    .await
    .context("Error running image task")?
}

pub async fn respond_slash<
    'a,
    F: FnOnce(DynamicImage) -> FR + Send + 'static,
    FR: anyhow::Context<DynamicImage, FE>,
    FE,
>(
    attachment: &'_ Attachment,
    responder: CommandResponder<'_, 'a>,
    lossless_out: bool,
    f: F,
) -> CommandResult<'a> {
    let responder = responder
        .defer_message(MessageOpts::default())
        .await
        .context("Error sending deferred message")?;

    let bytes = process(ImageInput::Attachment(attachment), lossless_out, f).await?;

    let attachment = CreateAttachment::bytes(
        bytes,
        PathBuf::from(&attachment.filename)
            .with_extension("jpg")
            .display()
            .to_string(),
    );
    responder
        .create_followup(Message::plain("").attach([attachment]))
        .await
        .context("Error sending processed image")?;

    Ok(responder.into())
}

pub async fn respond_msg<
    'a,
    F: FnOnce(DynamicImage) -> FR + Send + 'static,
    FR: anyhow::Context<DynamicImage, FE>,
    FE,
>(
    visitor: &mut CommandVisitor<'_>,
    responder: CommandResponder<'_, 'a>,
    lossless_out: bool,
    f: F,
) -> CommandResult<'a> {
    let message = visitor.target().message()?;

    let input = 'found: {
        if let [ref attachment] = *message.attachments {
            break 'found Some((ImageInput::Attachment(attachment), &*attachment.filename));
        }

        if let [ref embed] = *message.embeds {
            if let Some(ref image) = embed.image
                && let Ok(url) = image.url.parse()
            {
                break 'found Some((ImageInput::Url(url), "output.jpg"));
            }

            if let Some(ref thumbnail) = embed.thumbnail
                && let Ok(url) = thumbnail.url.parse()
            {
                break 'found Some((ImageInput::Url(url), "thumb.jpg"));
            }

            if let Some(ref author) = embed.author
                && let Some(ref icon) = author.icon_url
                && let Ok(url) = icon.parse()
            {
                break 'found Some((ImageInput::Url(url), "icon.jpg"));
            }

            if let Some(ref url) = embed.url
                && let Ok(url) = url.parse::<Url>()
                && ImageFormat::from_path(url.path()).is_ok()
            {
                break 'found Some((ImageInput::Url(url), "embed.jpg"));
            }
        }

        None
    };

    let Some((input, filename)) = input else {
        return Err(responder
            .create_message(
                Message::plain("Target message must have exactly one attachment!").ephemeral(true),
            )
            .await
            .context("Error sending attachment count error")?
            .into_err("Target message had multiple or no attachments"));
    };

    let responder = responder
        .defer_message(MessageOpts::default())
        .await
        .context("Error sending deferred message")?;

    let bytes = process(input, lossless_out, f).await?;

    let attachment = CreateAttachment::bytes(
        bytes,
        PathBuf::from(filename)
            .with_extension("jpg")
            .display()
            .to_string(),
    );
    responder
        .create_followup(Message::plain("").attach([attachment]))
        .await
        .context("Error sending processed image")?;

    Ok(responder.into())
}
