use std::{path::PathBuf, io::Cursor};

use serenity::model::prelude::AttachmentType;

use crate::client::interaction::handler::CommandHandler;

use super::prelude::*;

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
        CommandInfo::build_slash(&self.name, "Applies a JPEG effect to an image.", |a| a
            .attachment("image", "The input image", true)
            .int("quality", "The compression quality", false, 1..=100)
        ).unwrap()
    }

    async fn respond<'a>(
        &self,
        _ctx: &Context,
        visitor: &mut CommandVisitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        let attachment = visitor.visit_attachment("image")?.required()?;
        let quality = visitor.visit_i64("quality")?.optional().unwrap_or(1) as u8; // validated to be in range 1..=100
        
        let image_data = attachment.download().await.context("Error downloading attachment from discord")?;

        use jpeggr::image::{self, ImageFormat};
        let format = None
            .or_else(|| attachment.content_type.as_ref().and_then(ImageFormat::from_mime_type))
            .or_else(|| ImageFormat::from_path(&attachment.filename).ok())
            .or_else(|| image::guess_format(&image_data).ok())
            .context("Error determining format of attached image")?;

        let image = image::load_from_memory_with_format(&image_data, format).context("Error reading image data")?;
        let jpegged_image = jpeggr::jpeg_dynamic_image(image, 1, quality).context("Error applying JPEG effect to image")?;

        let mut bytes = Vec::new();
        jpegged_image.write_to(&mut Cursor::new(&mut bytes), image::ImageOutputFormat::Jpeg(quality)).context("Error encoding image")?;

        let attachment = AttachmentType::Bytes { data: bytes.into(), filename: PathBuf::from(&attachment.filename).with_extension("jpg").display().to_string() };
        let responder = responder.create_message(Message::plain("").attach([attachment])).await.context("Error sending jpegged image")?;
        
        Ok(responder.into())
    }
}
