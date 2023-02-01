use std::{io::Cursor, path::PathBuf};

use jpeggr::image::{self, ImageFormat};
use serenity::model::prelude::AttachmentType;

use super::prelude::*;
use crate::client::interaction::handler::CommandHandler;

async fn jpeg(attachment: &Attachment, quality: Option<i64>) -> Result<Vec<u8>> {
    let quality @ 0..=100 = quality.unwrap_or(1) else { unreachable!() };
    let quality = u8::try_from(quality).unwrap_or_else(|_| unreachable!());
    let image_data = attachment
        .download()
        .await
        .context("Error downloading attachment from discord")?;
    let content_type = attachment.content_type.clone();
    let filename = attachment.filename.clone();

    tokio::task::spawn_blocking(move || {
        let format = None
            .or_else(|| content_type.as_ref().and_then(ImageFormat::from_mime_type))
            .or_else(|| ImageFormat::from_path(&filename).ok())
            .or_else(|| image::guess_format(&image_data).ok())
            .context("Error determining format of attached image")?;

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

        let bytes = jpeg(attachment, quality).await?;

        let attachment = AttachmentType::Bytes {
            data: bytes.into(),
            filename: PathBuf::from(&attachment.filename)
                .with_extension("jpg")
                .display()
                .to_string(),
        };
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

    async fn respond<'a>(
        &self,
        _ctx: &Context,
        visitor: &mut CommandVisitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        let message = visitor.target().message()?;

        let [ref attachment] = *message.attachments else {
            return Err(responder
                .create_message(Message::plain(
                    "Target message must have exactly one attachment!",
                ))
                .await
                .context("Error sending attachment count error")?
                .into_err("Target message had multiple or no attachments"));
        };

        let responder = responder
            .defer_message(MessageOpts::default())
            .await
            .context("Error sending deferred message")?;

        let bytes = jpeg(attachment, None).await?;

        let attachment = AttachmentType::Bytes {
            data: bytes.into(),
            filename: PathBuf::from(&attachment.filename)
                .with_extension("jpg")
                .display()
                .to_string(),
        };
        responder
            .create_followup(Message::plain("").attach([attachment]))
            .await
            .context("Error sending jpegged image")?;

        Ok(responder.into())
    }
}
