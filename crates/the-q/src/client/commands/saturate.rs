use jpeggr::image::{self, DynamicImage};

use super::prelude::*;
use crate::util::{self, image::tonemap};

const MIN_PERCENT: f64 = 0.0;
const MAX_PERCENT: f64 = 1_000.0;

#[derive(Clone, Copy)]
struct Params {
    percent: f64,
}

const DEFAULT_PARAMS: Params = Params { percent: 400.0 };

#[derive(Clone, Copy)]
struct Tonemap;

impl tonemap::SigmoidParams for Tonemap {
    type Scalar = f32;

    fn gain_negative(self) -> Self::Scalar { -16.0 }

    fn gain_positive(self) -> Self::Scalar { 1.0 }

    fn inflection(self) -> Self::Scalar { 0.35 }

    fn domain_max(self) -> Self::Scalar { 16.0 }
}

#[inline]
fn tonemap(x: f32) -> f32 { tonemap::sigmoid(x, Tonemap) }

#[inline]
fn tonemap_inv(x: f32) -> f32 { tonemap::sigmoid_inv(x, Tonemap) }

const fn luma_dropoff(x: f32) -> f32 {
    const fn sigmoid(mut x: f32) -> f32 {
        x -= 1.0;
        0.5 - (x / (2.0 * x.abs() + 1.0))
    }

    sigmoid(x) / sigmoid(0.0)
}

fn saturate(image: DynamicImage, params: Params) -> Result<DynamicImage, jpeggr::Error> {
    let Params { percent } = params;

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
            r: tonemap_inv(*r),
            g: tonemap_inv(*g),
            b: tonemap_inv(*b),
        });

        let desat = luma_dropoff(lab.l);
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
        *r = tonemap(out.r);
        *g = tonemap(out.g);
        *b = tonemap(out.b);
    }

    buf.apply_color_space(
        image::metadata::Cicp::SRGB,
        image::ConvertColorOptions::default(),
    )?;
    Ok(buf.into())
}

#[derive(Debug, Default)]
pub struct SaturateCommand;

#[derive(DeserializeCommand)]
#[deserialize(cx = HandlerCx)]
pub struct SaturateArgs<'a> {
    image: &'a Attachment,
    percent: f64,
}

impl CommandHandler<Schema, HandlerCx> for SaturateCommand {
    type Data<'a> = SaturateArgs<'a>;

    // fn register_global(&self, cx: &HandlerCx) -> CommandInfo {
    //     CommandInfo::build_slash(
    //         cx.opts.command_name("saturate"),
    //         "Adjusts the saturation of an image",
    //         |a| {
    //             a.attachment("image", "The input image", true).real(
    //                 "percent",
    //                 "Saturation percentage (defaults to 400%)",
    //                 false,
    //                 MIN_PERCENT..=MAX_PERCENT,
    //             )
    //         },
    //     )
    //     .unwrap()
    // }

    async fn respond<'a, 'r>(
        &'a self,
        _serenity_cx: &'a Context,
        cx: &'a HandlerCx,
        data: Self::Data<'a>,
        responder: handler::CommandResponder<'a, 'r, Schema>,
    ) -> handler::CommandResult<'r, Schema> {
        let SaturateArgs { image, percent } = data;

        util::image::respond_slash(
            &cx.opts.image_rate_limit,
            &cx.redis,
            image,
            responder,
            false,
            move |i| saturate(i, Params { percent }),
        )
        .await
    }
}

#[derive(Debug, Default)]
pub struct SaturateMessageCommand;

#[derive(DeserializeCommand)]
#[deserialize(cx = HandlerCx)]
pub struct SaturateMessageArgs<'a> {
    message: &'a MessageBase,
}

impl CommandHandler<Schema, HandlerCx> for SaturateMessageCommand {
    type Data<'a> = SaturateMessageArgs<'a>;

    // fn register_global(&self, cx: &HandlerCx) -> CommandInfo {
    //     CommandInfo::message(cx.opts.menu_name("Saturate This"))
    // }

    async fn respond<'a, 'r>(
        &'a self,
        _serenity_cx: &'a Context,
        cx: &'a HandlerCx,
        data: Self::Data<'a>,
        responder: handler::CommandResponder<'a, 'r, Schema>,
    ) -> handler::CommandResult<'r, Schema> {
        let SaturateMessageArgs { message } = data;

        util::image::respond_msg(
            &cx.opts.image_rate_limit,
            &cx.redis,
            message,
            responder,
            false,
            |i| saturate(i, DEFAULT_PARAMS),
        )
        .await
    }
}

#[derive(Debug, Default)]
pub struct SaturateUserCommand;

#[derive(DeserializeCommand)]
#[deserialize(cx = HandlerCx)]
pub struct SaturateUserArgs<'a> {
    user: &'a User,
}

impl CommandHandler<Schema, HandlerCx> for SaturateUserCommand {
    type Data<'a> = SaturateUserArgs<'a>;

    // fn register_global(&self, cx: &HandlerCx) -> CommandInfo {
    //     CommandInfo::user(cx.opts.menu_name("Saturate This User"))
    // }

    async fn respond<'a, 'r>(
        &'a self,
        _serenity_cx: &'a Context,
        cx: &'a HandlerCx,
        data: Self::Data<'a>,
        responder: handler::CommandResponder<'a, 'r, Schema>,
    ) -> handler::CommandResult<'r, Schema> {
        let SaturateUserArgs { user } = data;

        util::image::respond_user(
            &cx.opts.image_rate_limit,
            &cx.redis,
            user,
            responder,
            false,
            |i| saturate(i, DEFAULT_PARAMS),
        )
        .await
    }
}
