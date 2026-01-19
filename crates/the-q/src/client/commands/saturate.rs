use jpeggr::image::{self, DynamicImage};

use super::prelude::*;
use crate::util;

const MIN_PERCENT: f64 = 0.0;
const MAX_PERCENT: f64 = 1_000.0;

#[expect(clippy::unnecessary_wraps, reason = "Expected for closure signature")]
fn saturate(image: DynamicImage, percent: Option<f64>) -> Result<DynamicImage, jpeggr::Error> {
    let percent @ MIN_PERCENT..=MAX_PERCENT = percent.unwrap_or(300.0) else {
        unreachable!();
    };

    #[expect(clippy::cast_possible_truncation)]
    let factor = (percent / 100.0) as f32;

    let mut buf = image.into_rgba32f();

    for image::Rgba([r, g, b, _a]) in buf.pixels_mut() {
        let mut lab = oklab::srgb_f32_to_oklab(oklab::Rgb {
            r: *r,
            g: *g,
            b: *b,
        });
        lab.a *= factor;
        lab.b *= factor;
        oklab::Rgb {
            r: *r,
            g: *g,
            b: *b,
        } = oklab::oklab_to_srgb_f32(lab);
    }

    Ok(buf.into())
}

#[derive(Debug)]
pub struct SaturateCommand {
    name: String,
}

impl From<&CommandOpts> for SaturateCommand {
    fn from(opts: &CommandOpts) -> Self {
        Self {
            name: format!("{}saturate", opts.command_base),
        }
    }
}

#[async_trait]
impl CommandHandler<Schema> for SaturateCommand {
    fn register_global(&self) -> CommandInfo {
        CommandInfo::build_slash(&self.name, "Content-aware scales an image", |a| {
            a.attachment("image", "The input image", true).real(
                "x-percent",
                "X resize percentage (defaults to Y percent or 50%)",
                false,
                MIN_PERCENT..=MAX_PERCENT,
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
        let percent = visitor.visit_number("percent")?.optional();

        util::image::respond_slash(attachment, responder, false, move |i| saturate(i, percent))
            .await
    }
}

#[derive(Debug)]
pub struct SaturateMessageCommand {
    name: String,
}

impl From<&CommandOpts> for SaturateMessageCommand {
    fn from(opts: &CommandOpts) -> Self {
        Self {
            name: format!("{}Saturate This", opts.context_menu_base),
        }
    }
}

#[async_trait]
impl CommandHandler<Schema> for SaturateMessageCommand {
    fn register_global(&self) -> CommandInfo { CommandInfo::message(&self.name) }

    async fn respond<'a>(
        &self,
        _ctx: &Context,
        visitor: &mut CommandVisitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        util::image::respond_msg(visitor, responder, false, |i| saturate(i, None)).await
    }
}

#[derive(Debug)]
pub struct SaturateUserCommand {
    name: String,
}

impl From<&CommandOpts> for SaturateUserCommand {
    fn from(opts: &CommandOpts) -> Self {
        Self {
            name: format!("{}Saturate This User", opts.context_menu_base),
        }
    }
}

#[async_trait]
impl CommandHandler<Schema> for SaturateUserCommand {
    fn register_global(&self) -> CommandInfo { CommandInfo::user(&self.name) }

    async fn respond<'a>(
        &self,
        _ctx: &Context,
        visitor: &mut CommandVisitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        util::image::respond_user(visitor, responder, false, |i| saturate(i, None)).await
    }
}
