use jpeggr::{image::DynamicImage, liquid};
use paracord::interaction::command::Choice;

use super::prelude::*;
use crate::util;

const MIN_PERCENT: f64 = 1.0;
const MAX_PERCENT: f64 = 300.0;

const MIN_WANDER: u16 = 0;
const MAX_WANDER: u16 = 5;

const MIN_RIGIDITY: f64 = -100.0;
const MAX_RIGIDITY: f64 = 100.0;

fn liquid(
    image: DynamicImage,
    x_percent: Option<f64>,
    y_percent: Option<f64>,
    seam_wander: Option<i64>,
    seam_rigidity: Option<f64>,
    resize_output: Option<i64>,
) -> Result<DynamicImage, jpeggr::Error> {
    const DEFAULT_PERCENT: f64 = 55.0;

    let (x_percent, y_percent) = (x_percent.or(y_percent), y_percent.or(x_percent));

    let x_percent @ MIN_PERCENT..=MAX_PERCENT = x_percent.unwrap_or(DEFAULT_PERCENT) else {
        unreachable!()
    };
    let y_percent @ MIN_PERCENT..=MAX_PERCENT = y_percent.unwrap_or(DEFAULT_PERCENT) else {
        unreachable!()
    };

    let Ok(seam_wander @ MIN_WANDER..=MAX_WANDER) = seam_wander.unwrap_or(2).try_into() else {
        unreachable!()
    };
    let seam_rigidity @ MIN_RIGIDITY..=MAX_RIGIDITY = seam_rigidity.unwrap_or(-50.0) else {
        unreachable!()
    };
    #[allow(clippy::cast_possible_truncation)]
    let seam_rigidity = seam_rigidity as f32;

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
        seam_wander,
        seam_rigidity,
        resize_output,
    })
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
                .int(
                    "seam-wander",
                    "Maximum allowed slope in the carved seams (0 is straight horizontal/vertical)",
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
        let seam_wander = visitor.visit_i64("seam-wander")?.optional();
        let seam_rigidity = visitor.visit_number("seam-rigidity")?.optional();
        let resize_output = visitor.visit_i64("resize-output")?.optional();

        util::image::respond_slash(attachment, responder, false, move |i| {
            liquid(
                i,
                x_percent,
                y_percent,
                seam_wander,
                seam_rigidity,
                resize_output,
            )
        })
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
        util::image::respond_msg(visitor, responder, false, |i| {
            liquid(i, None, None, None, None, None)
        })
        .await
    }
}
