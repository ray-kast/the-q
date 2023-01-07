use serenity::model::prelude::interaction::application_command::{
    CommandDataOption, CommandDataOptionValue,
};

use super::prelude::*;

#[derive(Debug)]
pub struct JpegCommand;

#[async_trait]
impl CommandHandler for JpegCommand {
    fn register(&self, opts: &CommandOpts, cmd: &mut CreateApplicationCommand) -> Option<GuildId> {
        cmd.name("jpeg")
            .description("Applies a JPEG effect to an image.")
            .kind(CommandType::User)
            .create_option(|option| {
                option
                    .name("image")
                    .description("The input image")
                    .kind(CommandOptionType::Attachment)
                    .required(true)
            })
            .create_option(|option| {
                option
                    .name("quality")
                    .description("The compression quality")
                    .kind(CommandOptionType::Integer)
                    .min_int_value(1)
                    .max_int_value(100)
                    .required(false)
            });
        None
    }

    async fn respond(&self, ctx: &Context, cmd: ApplicationCommandInteraction) -> Result {
        let mut quality = 1;
        let mut attachment = None;
        for option in cmd.data.options.iter() {
            match (option.name.as_str(), option.resolved) {
                ("quality", Some(CommandDataOptionValue::Integer(value))) => quality = value,
                ("image", Some(CommandDataOptionValue::Attachment(value))) => {
                    attachment = Some(value)
                },
            }
        }

        let Ok(quality @ 1..=100) = u8::try_from(quality) else {
            todo!();
        };

        let Some(attachment) = attachment else {
            todo!();
        };

        let image_data = attachment.download().await?;

        use jpeggr::image::{self, ImageFormat};
        let Some(format) = None
            .or_else(|| attachment.content_type.and_then(ImageFormat::from_mime_type))
            .or_else(|| ImageFormat::from_path(attachment.filename).ok())
            .or_else(|| image::guess_format(&image_data).ok())
        else {
            todo!();
        };

        let image = image::load_from_memory_with_format(&image_data, format)?; // TODO: error handling?
        let jpegged_image = jpeggr::jpeg_dynamic_image(image, 1, quality)?;

        todo!();
    }
}
