use jpeggr::{
    image::{imageops::FilterType, DynamicImage},
    jpeg,
};

use super::prelude::*;
use crate::util;

fn jpeg(
    image: DynamicImage,
    iterations: Option<i64>,
    quality: Option<i64>,
    size: Option<i64>,
) -> Result<DynamicImage, jpeggr::Error> {
    let iterations @ 0..=10 = iterations.unwrap_or(1) else {
        unreachable!()
    };
    let iterations = usize::try_from(iterations).unwrap_or_else(|_| unreachable!());

    let quality @ 0..=100 = quality.unwrap_or(1) else {
        unreachable!()
    };
    let quality = u8::try_from(quality).unwrap_or_else(|_| unreachable!());

    let size @ 1..=512 = size.unwrap_or(227) else {
        unreachable!()
    };
    let size = u32::try_from(size).unwrap_or_else(|_| unreachable!());

    jpeg::jpeg_dynamic_image(image, jpeg::JpegArgs {
        iterations,
        quality,
        size,
        down_filter: FilterType::Nearest,
        up_filter: FilterType::Lanczos3,
    })
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
            a.attachment("image", "The input image", true)
                .int(
                    "iterations",
                    "Number of times to JPEG the image",
                    false,
                    1..=10,
                )
                .int("quality", "The compression quality", false, 1..=100)
                .int("size", "Maximum output size", false, 1..=512)
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
        let iterations = visitor.visit_i64("iterations")?.optional();
        let quality = visitor.visit_i64("quality")?.optional();
        let size = visitor.visit_i64("size")?.optional();

        util::image::respond_slash(attachment, responder, false, move |i| {
            jpeg(i, iterations, quality, size)
        })
        .await
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
        util::image::respond_msg(visitor, responder, false, |i| jpeg(i, None, None, None)).await
    }
}
