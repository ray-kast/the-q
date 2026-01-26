use jpeggr::{
    image::{imageops::FilterType, DynamicImage},
    jpeg,
};

use super::prelude::*;
use crate::util;

#[derive(Clone, Copy)]
struct Params {
    iterations: usize,
    quality: u8,
    size: u32,
}

const DEFAULT_PARAMS: Params = Params {
    iterations: 1,
    quality: 1,
    size: 227,
};

fn jpeg(image: DynamicImage, params: Params) -> Result<DynamicImage, jpeggr::Error> {
    let Params {
        iterations,
        quality,
        size,
    } = params;

    jpeg::jpeg_dynamic_image(image, jpeg::JpegArgs {
        iterations,
        quality,
        size,
        down_filter: FilterType::Nearest,
        up_filter: FilterType::Lanczos3,
    })
}

#[derive(Debug, Default)]
pub struct JpegCommand;

#[derive(DeserializeCommand)]
#[deserialize(cx = HandlerCx)]
pub struct JpegArgs<'a> {
    image: &'a Attachment,
    iterations: usize,
    quality: u8,
    size: u32,
}

impl CommandHandler<Schema, HandlerCx> for JpegCommand {
    type Data<'a> = JpegArgs<'a>;

    // fn register_global(&self, cx: &HandlerCx) -> CommandInfo {
    //     CommandInfo::build_slash(
    //         cx.opts.command_name("jpeg"),
    //         "Applies a JPEG effect to an image",
    //         |a| {
    //             a.attachment("image", "The input image", true)
    //                 .int(
    //                     "iterations",
    //                     "Number of times to JPEG the image",
    //                     false,
    //                     1..=10,
    //                 )
    //                 .int("quality", "The compression quality", false, 1..=100)
    //                 .int("size", "Maximum output size", false, 1..=512)
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
        let JpegArgs {
            image,
            iterations,
            quality,
            size,
        } = data;

        util::image::respond_slash(
            &cx.opts.image_rate_limit,
            &cx.redis,
            image,
            responder,
            false,
            move |i| {
                jpeg(i, Params {
                    iterations,
                    quality,
                    size,
                })
            },
        )
        .await
    }
}

#[derive(Debug, Default)]
pub struct JpegMessageCommand;

#[derive(DeserializeCommand)]
#[deserialize(cx = HandlerCx)]
pub struct JpegMessageData<'a> {
    message: &'a MessageBase,
}

impl CommandHandler<Schema, HandlerCx> for JpegMessageCommand {
    type Data<'a> = JpegMessageData<'a>;

    // fn register_global(&self, cx: &HandlerCx) -> CommandInfo {
    //     CommandInfo::message(cx.opts.menu_name("JPEG This"))
    // }

    async fn respond<'a, 'r>(
        &'a self,
        _serenity_cx: &'a Context,
        cx: &'a HandlerCx,
        data: Self::Data<'a>,
        responder: handler::CommandResponder<'a, 'r, Schema>,
    ) -> handler::CommandResult<'r, Schema> {
        let JpegMessageData { message } = data;

        util::image::respond_msg(
            &cx.opts.image_rate_limit,
            &cx.redis,
            message,
            responder,
            false,
            |i| jpeg(i, DEFAULT_PARAMS),
        )
        .await
    }
}

#[derive(Debug, Default)]
pub struct JpegUserCommand;

#[derive(DeserializeCommand)]
#[deserialize(cx = HandlerCx)]
pub struct JpegUserData<'a> {
    user: &'a User,
}

impl CommandHandler<Schema, HandlerCx> for JpegUserCommand {
    type Data<'a> = JpegUserData<'a>;

    // fn register_global(&self, cx: &HandlerCx) -> CommandInfo {
    //     CommandInfo::user(cx.opts.menu_name("JPEG This User"))
    // }

    async fn respond<'a, 'r>(
        &'a self,
        _serenity_cx: &'a Context,
        cx: &'a HandlerCx,
        data: Self::Data<'a>,
        responder: handler::CommandResponder<'a, 'r, Schema>,
    ) -> handler::CommandResult<'r, Schema> {
        let JpegUserData { user } = data;

        util::image::respond_user(
            &cx.opts.image_rate_limit,
            &cx.redis,
            user,
            responder,
            false,
            |i| jpeg(i, DEFAULT_PARAMS),
        )
        .await
    }
}
