use jpeggr::image::{self, DynamicImage};

use super::prelude::*;
use crate::util;

const MIN_PERCENT: f64 = 0.0;
const MAX_PERCENT: f64 = 1_000.0;

const REINHARD_GAIN: f32 = 7.0;

fn pseudo_reinhard(x: f32) -> f32 {
    f32::from(x > 0.0) * (REINHARD_GAIN * x / ((REINHARD_GAIN - 1.0) * x + 1.0))
}

fn pseudo_reinhard_inv(x: f32) -> f32 {
    f32::from(x > 0.0) * (x / (REINHARD_GAIN - (REINHARD_GAIN - 1.0) * x))
}

const fn luma_dropoff(x: f32) -> f32 {
    const fn sigmoid(mut x: f32) -> f32 {
        x -= 1.0;
        0.5 - (x / (2.0 * x.abs() + 1.0))
    }

    sigmoid(x) / sigmoid(0.0)
}

fn saturate(image: DynamicImage, percent: Option<f64>) -> Result<DynamicImage, jpeggr::Error> {
    let percent @ MIN_PERCENT..=MAX_PERCENT = percent.unwrap_or(300.0) else {
        unreachable!();
    };

    #[expect(clippy::cast_possible_truncation)]
    let saturation = (percent / 100.0) as f32;

    let mut buf = image.into_rgba32f();
    buf.apply_color_space(
        image::metadata::Cicp::SRGB_LINEAR,
        image::ConvertColorOptions::default(),
    )?;

    let desat_amount = saturation.clamp(1.0, 2.0) - 1.0;
    for image::Rgba([r, g, b, _a]) in buf.pixels_mut() {
        let mut lab = oklab::linear_srgb_to_oklab(oklab::LinearRgb {
            r: pseudo_reinhard_inv(*r),
            g: pseudo_reinhard_inv(*g),
            b: pseudo_reinhard_inv(*b),
        });

        let desat = luma_dropoff(lab.l * 1.65);
        let factor = saturation * (1.0 + desat_amount * (desat - 1.0));
        #[cfg(debug_assertions)]
        {
            let factor_naive =
                (1.0 - desat_amount) * saturation + desat_amount * desat * saturation;
            assert!(
                (factor - factor_naive).abs() < 1e-5,
                "factor={factor} vs. factor_naive={factor_naive}"
            );
        }

        lab.a *= factor;
        lab.b *= factor;

        let out = oklab::oklab_to_linear_srgb(lab);
        *r = pseudo_reinhard(out.r);
        *g = pseudo_reinhard(out.g);
        *b = pseudo_reinhard(out.b);
    }

    buf.apply_color_space(
        image::metadata::Cicp::SRGB,
        image::ConvertColorOptions::default(),
    )?;
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
        CommandInfo::build_slash(&self.name, "Adjusts the saturation of an image", |a| {
            a.attachment("image", "The input image", true).real(
                "percent",
                "Saturation percentage (defaults to 300%)",
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
