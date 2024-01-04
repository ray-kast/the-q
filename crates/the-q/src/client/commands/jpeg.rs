use std::{io::Cursor, path::PathBuf};

use jpeggr::image::{self, ImageFormat};
use serenity::builder::CreateAttachment;

use super::prelude::*;

enum JpegInput<'a> {
    Attachment(&'a Attachment),
    Url(Url),
}

async fn jpeg(input: JpegInput<'_>, quality: Option<i64>) -> Result<Vec<u8>> {
    let quality @ 0..=100 = quality.unwrap_or(1) else {
        unreachable!()
    };
    let quality = u8::try_from(quality).unwrap_or_else(|_| unreachable!());

    let image_data;
    let content_type;
    let filename;
    match input {
        JpegInput::Attachment(a) => {
            image_data = a
                .download()
                .await
                .context("Error downloading attachment from discord")?;
            content_type = a.content_type.clone();
            filename = Some(a.filename.clone());
        },
        JpegInput::Url(u) => {
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
        let format = content_type
            .as_ref()
            .and_then(ImageFormat::from_mime_type)
            .or_else(|| filename.and_then(|f| ImageFormat::from_path(f).ok()))
            .or_else(|| image::guess_format(&image_data).ok())
            .context("Error determining format of input image")?;

        let image = image::load_from_memory_with_format(&image_data, format)
            .context("Error reading image data")?;
        let jpegged_image = jpeggr::jpeg_dynamic_image(image, 1, quality)
            .context("Error applying JPEG effect to image")?;

        let mut bytes = Vec::new();
        jpegged_image
            .write_to(
                &mut Cursor::new(&mut bytes),
                image::ImageOutputFormat::Jpeg(quality),
            )
            .context("Error encoding image")?;

        Ok(bytes)
    })
    .await
    .context("Error running image task")?
}

#[derive(Debug)]
pub struct JpegCommand {
    name: String,
}

impl From<&CommandOpts> for JpegCommand {
    fn from(opts: &CommandOpts) -> Self {
        Self {
            name: format!("{}jpeg", opts.command_base),
        }
    }
}

#[async_trait]
impl CommandHandler<Schema> for JpegCommand {
    fn register_global(&self) -> CommandInfo {
        CommandInfo::build_slash(&self.name, "Applies a JPEG effect to an image", |a| {
            a.attachment("image", "The input image", true).int(
                "quality",
                "The compression quality",
                false,
                1..=100,
            )
        })
        .unwrap()
    }

    async fn respond<'a>(
        &self,
        _ctx: &Context,
        visitor: &mut CommandVisitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        let attachment = visitor.visit_attachment("image")?.required()?;
        let quality = visitor.visit_i64("quality")?.optional();

        let responder = responder
            .defer_message(MessageOpts::default())
            .await
            .context("Error sending deferred message")?;

        let bytes = jpeg(JpegInput::Attachment(attachment), quality).await?;

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
            .context("Error sending jpegged image")?;

        Ok(responder.into())
    }
}

#[derive(Debug)]
pub struct JpegMessageCommand {
    name: String,
}

impl From<&CommandOpts> for JpegMessageCommand {
    fn from(opts: &CommandOpts) -> Self {
        Self {
            name: format!("{}JPEG This", opts.context_menu_base),
        }
    }
}

#[async_trait]
impl CommandHandler<Schema> for JpegMessageCommand {
    fn register_global(&self) -> CommandInfo { CommandInfo::message(&self.name) }

    // TODO: simplify error handling
    async fn respond<'a>(
        &self,
        _ctx: &Context,
        visitor: &mut CommandVisitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        let message = visitor.target().message()?;

        let input = 'found: {
            if let [ref attachment] = *message.attachments {
                break 'found Some((JpegInput::Attachment(attachment), &*attachment.filename));
            }

            // TODO: when let-chains
            if let [ref embed] = *message.embeds {
                if let Some(ref image) = embed.image {
                    if let Ok(url) = image.url.parse() {
                        break 'found Some((JpegInput::Url(url), "output.jpg"));
                    }
                }

                if let Some(ref thumbnail) = embed.thumbnail {
                    if let Ok(url) = thumbnail.url.parse() {
                        break 'found Some((JpegInput::Url(url), "thumb.jpg"));
                    }
                }

                if let Some(ref author) = embed.author {
                    if let Some(ref icon) = author.icon_url {
                        if let Ok(url) = icon.parse() {
                            break 'found Some((JpegInput::Url(url), "icon.jpg"));
                        }
                    }
                }

                if let Some(ref url) = embed.url {
                    if let Ok(url) = url.parse::<Url>() {
                        if ImageFormat::from_path(url.path()).is_ok() {
                            break 'found Some((JpegInput::Url(url), "embed.jpg"));
                        }
                    }
                }
            }

            None
        };

        let Some((input, filename)) = input else {
            return Err(responder
                .create_message(
                    Message::plain("Target message must have exactly one attachment!")
                        .ephemeral(true),
                )
                .await
                .context("Error sending attachment count error")?
                .into_err("Target message had multiple or no attachments"));
        };

        let responder = responder
            .defer_message(MessageOpts::default())
            .await
            .context("Error sending deferred message")?;

        let bytes = jpeg(input, None).await?;

        // TODO: post file size difference
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
            .context("Error sending jpegged image")?;

        Ok(responder.into())
    }
}
