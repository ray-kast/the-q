use jpeggr::{
    image::{self, DynamicImage},
    liquid,
};
use paracord::interaction::command::Choice;

use super::prelude::*;
use crate::util;

const MIN_PERCENT: f64 = 1.0;
const MAX_PERCENT: f64 = 300.0;

const MIN_CURLY: f64 = 0.0;
const MAX_CURLY: f64 = 5.0;

const MIN_BIAS_CURLY: f64 = -10.0;
const MAX_BIAS_CURLY: f64 = 10.0;

fn liquid(
    image: DynamicImage,
    x_percent: Option<f64>,
    y_percent: Option<f64>,
    curly_seams: Option<f64>,
    bias_curly: Option<f64>,
    resize_output: Option<i64>,
) -> Result<(DynamicImage, ()), jpeggr::Error> {
    const DEFAULT_PERCENT: f64 = 43.0;

    let (x_percent, y_percent) = (x_percent.or(y_percent), y_percent.or(x_percent));

    let x_percent @ MIN_PERCENT..=MAX_PERCENT = x_percent.unwrap_or(DEFAULT_PERCENT) else {
        unreachable!()
    };
    let y_percent @ MIN_PERCENT..=MAX_PERCENT = y_percent.unwrap_or(DEFAULT_PERCENT) else {
        unreachable!()
    };

    let curly_seams @ MIN_CURLY..=MAX_CURLY = curly_seams.unwrap_or(1.5) else {
        unreachable!()
    };
    let bias_curly @ MIN_BIAS_CURLY..=MAX_BIAS_CURLY = bias_curly.unwrap_or(0.6) else {
        unreachable!()
    };

    let resize_output = match resize_output {
        None => liquid::ResizeOutput::Upsample,
        Some(0) => liquid::ResizeOutput::OutputSize,
        Some(1) => liquid::ResizeOutput::FitToInput,
        Some(2) => liquid::ResizeOutput::StretchToInput,
        _ => unreachable!(),
    };

    liquid::liquid_dynamic_image(image, liquid::LiquidArgs {
        max_input_size: 640,
        x_fac: x_percent / 100.0,
        y_fac: y_percent / 100.0,
        curly_seams,
        bias_curly,
        resize_output,
    })
    .map(|i| (i, ()))
}

fn encode(image: DynamicImage, (): (), buf: &mut Vec<u8>) -> Result<(), image::ImageError> {
    let image = image.into_rgba8();
    image::codecs::webp::WebPEncoder::new_lossless(buf).encode(
        image.as_raw(),
        image.width(),
        image.height(),
        image::ExtendedColorType::Rgba8,
    )
}

#[derive(Debug)]
pub struct LiquidCommand {
    name: String,
}

impl From<&CommandOpts> for LiquidCommand {
    fn from(opts: &CommandOpts) -> Self {
        Self {
            name: format!("{}liquid", opts.command_base),
        }
    }
}

#[async_trait]
impl CommandHandler<Schema> for LiquidCommand {
    fn register_global(&self) -> CommandInfo {
        CommandInfo::build_slash(&self.name, "Content-aware scales an image", |a| {
            a.attachment("image", "The input image", true)
                .real(
                    "x-percent",
                    "X resize percentage (defaults to Y percent or 50%)",
                    false,
                    MIN_PERCENT..=MAX_PERCENT,
                )
                .real(
                    "y-percent",
                    "Y resize percentage (defaults to X percent or 50%)",
                    false,
                    MIN_PERCENT..=MAX_PERCENT,
                )
                .real(
                    "curly-seams",
                    "Amount of deviation in the carved seams",
                    false,
                    MIN_CURLY..=MAX_CURLY,
                )
                .real(
                    "bias-curly",
                    "Apply a bias for curly seams",
                    false,
                    MIN_BIAS_CURLY..=MAX_BIAS_CURLY,
                )
                .int_choice(
                    "resize-output",
                    "Resize the output image back to the original size",
                    false,
                    [
                        Choice::new("off", 0),
                        Choice::new("on", 1),
                        Choice::new("stretch", 2),
                    ],
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
        let x_percent = visitor.visit_number("x-percent")?.optional();
        let y_percent = visitor.visit_number("y-percent")?.optional();
        let curly_seams = visitor.visit_number("curly-seams")?.optional();
        let bias_curly = visitor.visit_number("bias-curly")?.optional();
        let resize_output = visitor.visit_i64("resize-output")?.optional();

        util::image::respond_slash(
            attachment,
            responder,
            move |i| {
                liquid(
                    i,
                    x_percent,
                    y_percent,
                    curly_seams,
                    bias_curly,
                    resize_output,
                )
            },
            encode,
        )
        .await
    }
}

#[derive(Debug)]
pub struct LiquidMessageCommand {
    name: String,
}

impl From<&CommandOpts> for LiquidMessageCommand {
    fn from(opts: &CommandOpts) -> Self {
        Self {
            name: format!("{}Liquefy This", opts.context_menu_base),
        }
    }
}

#[async_trait]
impl CommandHandler<Schema> for LiquidMessageCommand {
    fn register_global(&self) -> CommandInfo { CommandInfo::message(&self.name) }

    // TODO: simplify error handling
    async fn respond<'a>(
        &self,
        _ctx: &Context,
        visitor: &mut CommandVisitor<'_>,
        responder: CommandResponder<'_, 'a>,
    ) -> CommandResult<'a> {
        util::image::respond_msg(
            visitor,
            responder,
            |i| liquid(i, None, None, None, None, None),
            encode,
        )
        .await
    }
}
