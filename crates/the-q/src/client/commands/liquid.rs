use jpeggr::{image::DynamicImage, liquid};

use super::prelude::*;
use crate::util;

const MIN_PERCENT: f64 = 1.0;
const MAX_PERCENT: f64 = 300.0;

const MIN_WANDER: u16 = 0;
const MAX_WANDER: u16 = 5;

const MIN_RIGIDITY: f64 = -100.0;
const MAX_RIGIDITY: f64 = 100.0;

#[derive(Clone, Copy)]
struct Params {
    x_percent: Option<f64>,
    y_percent: Option<f64>,
    seam_wander: u16,
    seam_rigidity: f32,
    resize_output: ResizeOutput,
}

const DEFAULT_PARAMS: Params = Params {
    x_percent: None,
    y_percent: None,
    seam_wander: 2,
    seam_rigidity: -50.0,
    resize_output: ResizeOutput::Upsample,
};

fn liquid(image: DynamicImage, params: Params) -> Result<DynamicImage, jpeggr::Error> {
    const DEFAULT_PERCENT: f64 = 55.0;

    let Params {
        x_percent,
        y_percent,
        seam_wander,
        seam_rigidity,
        resize_output,
    } = params;

    let (x_percent, y_percent) = (
        x_percent.or(y_percent).unwrap_or(DEFAULT_PERCENT),
        y_percent.or(x_percent).unwrap_or(DEFAULT_PERCENT),
    );

    liquid::liquid_dynamic_image(image, liquid::LiquidArgs {
        max_input_size: 640,
        x_fac: x_percent / 100.0,
        y_fac: y_percent / 100.0,
        seam_wander,
        seam_rigidity,
        resize_output: resize_output.into(),
    })
}

#[derive(Debug, Default)]
pub struct LiquidCommand;

// #[derive(DeserializeCommand)]
// #[deserialize(cx = HandlerCx)]
pub struct LiquidArgs<'a> {
    image: &'a Attachment,
    x_percent: Option<f64>,
    y_percent: Option<f64>,
    seam_wander: u16,
    seam_rigidity: f32,
    resize_output: ResizeOutput,
}

impl<'a> DeserializeCommand<'a, HandlerCx> for LiquidArgs<'a> {
    type Completion = NoCompletion;

    fn register_global(cx: &HandlerCx) -> CommandInfo {
        CommandInfo::build_slash(
            cx.opts.command_name("liquid"),
            "Content-aware scales an image",
            |a| {
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
                    .int(
                        "seam-wander",
                        "Maximum allowed slope in the carved seams (0 is straight \
                         horizontal/vertical)",
                        false,
                        MIN_WANDER.into()..=MAX_WANDER.into(),
                    )
                    .real(
                        "seam-rigidity",
                        "Apply a bias toward (or against, if negative) more rigid seams",
                        false,
                        MIN_RIGIDITY..=MAX_RIGIDITY,
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
            },
        )
        .unwrap()
    }

    fn deserialize(visitor: &mut CommandVisitor<'a>) -> Result<Self, visitor::Error> {
        Ok(Self {
            image: visitor.visit_attachment("image")?.required()?,
            x_percent: visitor.visit_number("x-percent")?.optional(),
            y_percent: visitor.visit_number("y-percent")?.optional(),
            seam_wander: visitor
                .visit_i64("seam-wander")?
                .optional()
                .map_or(DEFAULT_PARAMS.seam_wander, |i| i.try_into().unwrap()),
            seam_rigidity: visitor.visit_number("seam-rigidity")?.optional().map_or(
                DEFAULT_PARAMS.seam_rigidity,
                #[expect(clippy::cast_possible_truncation)]
                |i| i as f32,
            ),
            resize_output: visitor.visit_i64("resize-output")?.optional().map_or(
                Ok(DEFAULT_PARAMS.resize_output),
                |i| {
                    Ok(match i {
                        0 => ResizeOutput::OutputSize,
                        1 => ResizeOutput::FitToInput,
                        2 => ResizeOutput::StretchToInput,
                        _ => {
                            return Err(visitor::Error::Malformed(
                                "Invalid value for resize-output",
                            ))
                        },
                    })
                },
            )?,
        })
    }
}

#[derive(Debug, Clone, Copy /* , DeserializeCommand */)]
// #[deserialize(cx = HandlerCx)]
pub enum ResizeOutput {
    OutputSize,
    FitToInput,
    StretchToInput,
    Upsample,
}

impl From<ResizeOutput> for liquid::ResizeOutput {
    fn from(value: ResizeOutput) -> Self {
        match value {
            ResizeOutput::OutputSize => Self::OutputSize,
            ResizeOutput::FitToInput => Self::FitToInput,
            ResizeOutput::StretchToInput => Self::StretchToInput,
            ResizeOutput::Upsample => Self::Upsample,
        }
    }
}

impl From<liquid::ResizeOutput> for ResizeOutput {
    fn from(value: liquid::ResizeOutput) -> Self {
        match value {
            liquid::ResizeOutput::OutputSize => Self::OutputSize,
            liquid::ResizeOutput::FitToInput => Self::FitToInput,
            liquid::ResizeOutput::StretchToInput => Self::StretchToInput,
            liquid::ResizeOutput::Upsample => Self::Upsample,
        }
    }
}

impl CommandHandler<Schema, HandlerCx> for LiquidCommand {
    type Data<'a> = LiquidArgs<'a>;

    async fn respond<'a, 'r>(
        &'a self,
        _serenity_cx: &'a Context,
        cx: &'a HandlerCx,
        data: Self::Data<'a>,
        responder: handler::CommandResponder<'a, 'r, Schema>,
    ) -> handler::CommandResult<'r, Schema> {
        let LiquidArgs {
            image,
            x_percent,
            y_percent,
            seam_wander,
            seam_rigidity,
            resize_output,
        } = data;

        util::image::respond_slash(
            &cx.opts.image_rate_limit,
            &cx.redis,
            image,
            responder,
            false,
            move |i| {
                liquid(i, Params {
                    x_percent,
                    y_percent,
                    seam_wander,
                    seam_rigidity,
                    resize_output,
                })
            },
        )
        .await
    }
}

#[derive(Debug, Default)]
pub struct LiquidMessageCommand;

// #[derive(DeserializeCommand)]
// #[deserialize(cx = HandlerCx)]
pub struct LiquidMessageArgs<'a> {
    message: &'a MessageBase,
}

impl<'a> DeserializeCommand<'a, HandlerCx> for LiquidMessageArgs<'a> {
    type Completion = NoCompletion;

    fn register_global(cx: &HandlerCx) -> CommandInfo {
        CommandInfo::message(cx.opts.menu_name("Liquefy This"))
    }

    fn deserialize(visitor: &mut CommandVisitor<'a>) -> Result<Self, visitor::Error> {
        Ok(Self {
            message: visitor.target().message()?,
        })
    }
}

impl CommandHandler<Schema, HandlerCx> for LiquidMessageCommand {
    type Data<'a> = LiquidMessageArgs<'a>;

    async fn respond<'a, 'r>(
        &'a self,
        _serenity_cx: &'a Context,
        cx: &'a HandlerCx,
        data: Self::Data<'a>,
        responder: handler::CommandResponder<'a, 'r, Schema>,
    ) -> handler::CommandResult<'r, Schema> {
        let LiquidMessageArgs { message } = data;

        util::image::respond_msg(
            &cx.opts.image_rate_limit,
            &cx.redis,
            message,
            responder,
            false,
            |i| liquid(i, DEFAULT_PARAMS),
        )
        .await
    }
}

#[derive(Debug, Default)]
pub struct LiquidUserCommand;

// #[derive(DeserializeCommand)]
// #[deserialize(cx = HandlerCx)]
pub struct LiquidUserArgs<'a> {
    user: &'a User,
}

impl<'a> DeserializeCommand<'a, HandlerCx> for LiquidUserArgs<'a> {
    type Completion = NoCompletion;

    fn register_global(cx: &HandlerCx) -> CommandInfo {
        CommandInfo::user(cx.opts.menu_name("Liquefy This User"))
    }

    fn deserialize(visitor: &mut CommandVisitor<'a>) -> Result<Self, visitor::Error> {
        Ok(Self {
            user: visitor.target().user()?.0,
        })
    }
}

impl CommandHandler<Schema, HandlerCx> for LiquidUserCommand {
    type Data<'a> = LiquidUserArgs<'a>;

    async fn respond<'a, 'r>(
        &'a self,
        _serenity_cx: &'a Context,
        cx: &'a HandlerCx,
        data: Self::Data<'a>,
        responder: handler::CommandResponder<'a, 'r, Schema>,
    ) -> handler::CommandResult<'r, Schema> {
        let LiquidUserArgs { user } = data;

        util::image::respond_user(
            &cx.opts.image_rate_limit,
            &cx.redis,
            user,
            responder,
            false,
            |i| liquid(i, DEFAULT_PARAMS),
        )
        .await
    }
}
