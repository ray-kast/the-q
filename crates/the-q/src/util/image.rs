use std::{io::Cursor, sync::LazyLock};

use jpeggr::image::{self, AnimationDecoder, ColorType, DynamicImage, ImageFormat};
use paracord::interaction::{
    handler::{CommandVisitor, IntoErr},
    response::{prelude::*, Message, MessageOpts},
};
use regex::Regex;
use serenity::{builder::CreateAttachment, model::channel::Attachment};

use crate::{
    prelude::*,
    util::{
        http_client,
        interaction::{CommandResponder, CommandResult, CreatedCommandResponder},
    },
};

pub mod tonemap;

enum ImageInput<'a> {
    Attachment(&'a Attachment),
    Url(Url),
    Video { url: Url, thumb: Option<Url> },
}

enum ImageType<'a> {
    Static(DynamicImage),
    Frames(image::Frames<'a>),
    WebpFrames(webp::DecodeAnimImage),
}

impl ImageType<'_> {
    fn process<F: FnMut(DynamicImage) -> FR, FR: anyhow::Context<DynamicImage, FE>, FE>(
        self,
        lossless_out: bool,
        f: F,
    ) -> Result<Vec<u8>> {
        match self {
            Self::Static(i) => {
                debug!("Processing static image");

                process_static(i, lossless_out, f)
            },
            ImageType::Frames(g) => {
                debug!("Processing animation");

                process_anim(
                    g.into_iter().scan(0_i32, |t, f| {
                        Some(f.map(|f| {
                            let (num, denom) = f.delay().numer_denom_ms();
                            *t = t.saturating_add(i32::try_from(num / denom).unwrap_or(i32::MAX));
                            (f.into_buffer().into(), *t)
                        }))
                    }),
                    [0; 4],
                    0,
                    f,
                )
            },
            ImageType::WebpFrames(w) => {
                debug!("Processing WebP-encoded animation");

                process_anim(
                    w.into_iter().map(|f| Ok(((&f).into(), f.get_time_ms()))),
                    [0; 4], // TODO: this is a u32 in w, what is the endianness there?
                    w.loop_count
                        .try_into()
                        .expect("Can't roundtrip loop_count??"),
                    f,
                )
            },
        }
    }
}

static WEBP_CONFIG: LazyLock<webp::WebPConfig> = LazyLock::new(|| webp::WebPConfig {
    quality: 90.0,
    target_size: 8 << 20,
    ..webp::WebPConfig::new().expect("Unable to create base WebP config")
});

fn prepare_webp_buffer(image: DynamicImage) -> DynamicImage {
    match image.color() {
        ColorType::L8 | ColorType::L16 => image.into_luma8().into(),
        ColorType::La8 | ColorType::La16 => image.into_luma_alpha8().into(),
        ColorType::Rgb8 | ColorType::Rgb16 | ColorType::Rgb32F => image.into_rgb8().into(),
        _ => image.into_rgba8().into(),
    }
}

fn process_static<F: FnOnce(DynamicImage) -> FR, FR: anyhow::Context<DynamicImage, FE>, FE>(
    image: DynamicImage,
    lossless_out: bool,
    f: F,
) -> Result<Vec<u8>> {
    let image = prepare_webp_buffer(f(image).context("Error processing image")?);
    let enc = webp::Encoder::from_image(&image)
        .map_err(|e| anyhow!(e.to_string()))
        .context("Error creating WebP encoder")?;

    Ok(enc
        .encode_advanced(&webp::WebPConfig {
            lossless: lossless_out.into(),
            ..*WEBP_CONFIG
        })
        .map_err(|e| anyhow!("{e:?}"))
        .context("Error encoding output image")?
        .to_vec())
}

fn process_anim<
    I: IntoIterator<Item: anyhow::Context<(DynamicImage, i32), image::ImageError>>,
    F: FnMut(DynamicImage) -> FR,
    FR: anyhow::Context<DynamicImage, FE>,
    FE,
>(
    frames: I,
    bg_color: [u8; 4],
    loop_count: i32,
    mut f: F,
) -> Result<Vec<u8>> {
    let mut outs = vec![];

    for (i, frame) in frames.into_iter().enumerate() {
        trace!(frame = i, "Processing frame");

        let (frame, ts) = frame.with_context(|| format!("Error reading frame {i}"))?;
        outs.push((
            prepare_webp_buffer(f(frame).with_context(|| format!("Error processing frame {i}"))?),
            ts,
        ));
    }

    let Some((first, _)) = outs.first() else {
        bail!("Input image contained no frames");
    };
    let mut enc = webp::AnimEncoder::new(first.width(), first.height(), &WEBP_CONFIG);
    enc.set_bgcolor(bg_color);
    enc.set_loop_count(loop_count);

    for (out, timestamp) in &outs {
        enc.add_frame(webp::AnimFrame::from_image(out, *timestamp).unwrap());
    }

    Ok(enc
        .try_encode()
        .map_err(|e| anyhow!("{e:?}"))
        .context("Error encoding animated output image")?
        .to_vec())
}

async fn process_video<F: FnMut(DynamicImage) -> FR, FR: anyhow::Context<DynamicImage, FE>, FE>(
    url: Url,
    thumb: Option<Url>,
    responder: CreatedCommandResponder<'_>,
    f: F,
) -> CommandResult<'_> {
    trace!(url = url.as_str(), "Fetching video frames over HTTP");
    let res = http_client(None)
        .get(url)
        .send()
        .await
        .context("Error fetching input URL")?;

    if res.content_length().is_none_or(|l| l > 1 << 20) {
        return Ok(responder
            .delete_and_followup(Message::plain("Video is too large!").ephemeral(true))
            .await
            .context("Couldn't post content length error")?
            .0
            .into());
    }

    let mut frames = vec![];
    let framerate = 60.0_f64;

    match res
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
    {
        Some("video/mp4") => {
            gstreamer::init().context("Error initializing GStreamer")?;
            todo!()
        },
        _ => {
            return Ok(responder
                .delete_and_followup(Message::plain("Unknown video type").ephemeral(true))
                .await
                .context("Couldn't post content type error")?
                .0
                .into());
        },
    }

    let delay = framerate.recip();
    #[expect(clippy::cast_possible_truncation)]
    let bytes = process_anim(
        frames.into_iter().scan(0.0, |t, f| {
            let t = mem::replace(t, *t + delay);
            assert!(t.abs() < i32::MAX.into());
            Some(Ok((f, t.round() as i32)))
        }),
        [0, 0, 0, 255],
        0,
        f,
    )
    .context("Error processing video frames")?;

    post_response(responder, bytes).await
}

async fn post_response(
    responder: CreatedCommandResponder<'_>,
    bytes: Vec<u8>,
) -> CommandResult<'_> {
    let attachment = CreateAttachment::bytes(bytes, "output.webp");
    responder
        .create_followup(Message::plain("").attach([attachment]))
        .await
        .context("Error sending processed image")?;

    Ok(responder.into())
}

fn open(image_data: &'_ [u8], format: ImageFormat) -> Result<ImageType<'_>> {
    Ok(match format {
        ImageFormat::Gif => ImageType::Frames(
            image::codecs::gif::GifDecoder::new(Cursor::new(image_data))
                .context("Error opening image as GIF")?
                .into_frames(),
        ),
        ImageFormat::Png => {
            let dec = image::codecs::png::PngDecoder::new(Cursor::new(image_data))
                .context("Error opening image as PNG")?;

            if dec
                .is_apng()
                .context("Error detecting APNG")
                .unwrap_or_else(|err| {
                    debug!(?err);
                    false
                })
            {
                ImageType::Frames(
                    dec.apng()
                        .context("Error opening image as APNG")?
                        .into_frames(),
                )
            } else {
                ImageType::Static(
                    DynamicImage::from_decoder(dec).context("Error opening image as static PNG")?,
                )
            }
        },
        ImageFormat::WebP => {
            let image = webp::AnimDecoder::new(image_data)
                .decode()
                .map_err(|e| anyhow!(e))
                .context("Error opening image as animated WebP")?;

            if image.has_animation() {
                ImageType::WebpFrames(image)
            } else {
                ImageType::Static(
                    image
                        .get_frame(0)
                        .as_ref()
                        .context("Static WebP contained no frames")?
                        .into(),
                )
            }
        },
        f => ImageType::Static(
            image::load_from_memory_with_format(image_data, f)
                .with_context(|| format!("Unable to load image with format {f:?}"))?,
        ),
    })
}

async fn process<
    'a,
    F: FnMut(DynamicImage) -> FR + Send + 'static,
    FR: anyhow::Context<DynamicImage, FE>,
    FE,
>(
    input: ImageInput<'_>,
    responder: CreatedCommandResponder<'a>,
    lossless_out: bool,
    f: F,
) -> CommandResult<'a> {
    let image_data;
    let content_type;
    let filename;
    match input {
        ImageInput::Attachment(a) => {
            image_data = a
                .download()
                .await
                .context("Error downloading attachment from discord")?;
            content_type = a.content_type.clone();
            filename = Some(a.filename.clone());
        },
        ImageInput::Url(u) => {
            trace!(url = u.as_str(), "Fetching image over HTTP");
            let res = http_client(None)
                .get(u)
                .send()
                .await
                .context("Error fetching input URL")?;
            content_type = res
                .headers()
                .get("Content-Type")
                .and_then(|h| h.to_str().ok())
                .map(ToOwned::to_owned);
            image_data = res
                .bytes()
                .await
                .context("Error downloading image response")?
                .to_vec();
            filename = None;
        },
        ImageInput::Video { url, thumb } => return process_video(url, thumb, responder, f).await,
    }

    let bytes = tokio::task::spawn_blocking(move || {
        let image = [
            content_type.as_ref().and_then(ImageFormat::from_mime_type),
            image::guess_format(&image_data).ok(),
            filename.and_then(|f| ImageFormat::from_path(f).ok()),
        ]
        .into_iter()
        .find_map(|f| open(&image_data, f?).map_err(|err| debug!(?err)).ok())
        .ok_or_else(|| anyhow!("Unable to determine input image format"))?;

        image.process(lossless_out, f)
    })
    .await
    .context("Error running image task")??;

    post_response(responder, bytes).await
}

pub async fn respond_slash<
    'a,
    F: FnMut(DynamicImage) -> FR + Send + 'static,
    FR: anyhow::Context<DynamicImage, FE>,
    FE,
>(
    attachment: &'_ Attachment,
    responder: CommandResponder<'_, 'a>,
    lossless_out: bool,
    f: F,
) -> CommandResult<'a> {
    let responder = responder
        .defer_message(MessageOpts::default())
        .await
        .context("Error sending deferred message")?;

    process(
        ImageInput::Attachment(attachment),
        responder,
        lossless_out,
        f,
    )
    .await
}

pub async fn respond_msg<
    'a,
    F: FnMut(DynamicImage) -> FR + Send + 'static,
    FR: anyhow::Context<DynamicImage, FE>,
    FE,
>(
    visitor: &mut CommandVisitor<'_>,
    responder: CommandResponder<'_, 'a>,
    lossless_out: bool,
    f: F,
) -> CommandResult<'a> {
    let message = visitor.target().message()?;
    trace!(payload = ?message, "Looking for images");

    let input = 'found: {
        static EMOJI_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"<[^:]*:[^:]+:\d+>").unwrap());

        if let [ref attachment] = *message.attachments {
            break 'found Some(ImageInput::Attachment(attachment));
        }

        if let [ref embed] = *message.embeds {
            if let Some(ref image) = embed.image
                && let Ok(url) = image.proxy_url.as_ref().unwrap_or(&image.url).parse()
            {
                break 'found Some(ImageInput::Url(url));
            }

            let thumb = embed
                .thumbnail
                .as_ref()
                .and_then(|t| t.proxy_url.as_ref().unwrap_or(&t.url).parse().ok());

            if let Some(video) = &embed.video
                && let Ok(url) = video.proxy_url.as_ref().unwrap_or(&video.url).parse()
            {
                break 'found Some(ImageInput::Video { url, thumb });
            }

            if let Some(thumb) = thumb {
                break 'found Some(ImageInput::Url(thumb));
            }

            if let Some(ref author) = embed.author
                && let Some(ref icon) = author.icon_url
                && let Ok(url) = icon.parse()
            {
                break 'found Some(ImageInput::Url(url));
            }

            if let Some(ref url) = embed.url
                && let Ok(url) = url.parse::<Url>()
                && ImageFormat::from_path(url.path()).is_ok()
            {
                break 'found Some(ImageInput::Url(url));
            }
        }

        if let [ref sticker] = *message.sticker_items
            && let Some(url) = sticker.image_url()
            && let Ok(url) = url.parse()
        {
            break 'found Some(ImageInput::Url(url));
        }

        let mut matches = EMOJI_RE.find_iter(&message.content).peekable();
        if let Some(emoji) = matches.peek()
            && let Some(emoji) = serenity::utils::parse_emoji(emoji.as_str())
            && matches
                .all(|m| serenity::utils::parse_emoji(m.as_str()).is_some_and(|e| e.id == emoji.id))
            && let Ok(url) = emoji.url().parse()
        {
            break 'found Some(ImageInput::Url(url));
        }

        None
    };

    let Some(input) = input else {
        return Err(responder
            .create_message(
                Message::plain("Target message must have exactly one image!").ephemeral(true),
            )
            .await
            .context("Error sending attachment count error")?
            .into_err("Target message had multiple or no attachments"));
    };

    let responder = responder
        .defer_message(MessageOpts::default())
        .await
        .context("Error sending deferred message")?;

    process(input, responder, lossless_out, f).await
}

pub async fn respond_user<
    'a,
    F: FnMut(DynamicImage) -> FR + Send + 'static,
    FR: anyhow::Context<DynamicImage, FE>,
    FE,
>(
    visitor: &mut CommandVisitor<'_>,
    responder: CommandResponder<'_, 'a>,
    lossless_out: bool,
    f: F,
) -> CommandResult<'a> {
    let (user, _) = visitor.target().user()?;

    let responder = responder
        .defer_message(MessageOpts::default())
        .await
        .context("Error sending deferred message")?;

    process(
        ImageInput::Url(
            user.static_face()
                .parse()
                .context("Error parsing avatar URL")?,
        ),
        responder,
        lossless_out,
        f,
    )
    .await
}
