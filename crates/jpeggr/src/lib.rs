//! A crate which repeatedly applies a JPEG effect to an image

#![deny(
    clippy::disallowed_methods,
    clippy::suspicious,
    clippy::style,
    clippy::clone_on_ref_ptr,
    missing_debug_implementations,
    missing_copy_implementations
)]
#![warn(clippy::pedantic, missing_docs)]
#![allow(clippy::module_name_repetitions)]

use std::io::Cursor;

pub use image;
use image::{
    buffer::ConvertBuffer,
    codecs::jpeg::{JpegDecoder, JpegEncoder},
    imageops::{self, FilterType},
    ColorType, DynamicImage, ImageBuffer, ImageDecoder, ImageError, ImageResult, Pixel,
    PixelWithColorType,
};
use tracing::trace;

/// An error arising from JPEG-ing pixels
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The [`image`] crate raised an error
    #[error("Image format error")]
    Image(#[from] ImageError),
    /// A [`ColorType`] was encountered that was not supported
    #[error("Unsupported color type {0:?}")]
    UnsupportedColorType(ColorType),
}

/// Common arguments to the jpeg functions
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JpegArgs {
    /// Number of re-encoding iterations to run
    pub iterations: usize,
    /// JPEG encoding quality per iteration
    pub quality: u8,
    /// Maximum bounding size for image downsampling
    pub size: u32,
    /// Image downsampling filter
    pub down_filter: FilterType,
    /// Image upsampling filter
    pub up_filter: FilterType,
}

/// Apply JPEG compression to the given image buffer
///
/// # Errors
/// This function returns an error if the JPEG transcoder fails
///
/// # Panics
/// This function panics if the JPEG transcoder produces an invalid buffer
pub fn jpeg_buffer<P: PixelWithColorType + Pixel<Subpixel = u8> + 'static>(
    mut image: ImageBuffer<P, Vec<u8>>,
    args: JpegArgs,
) -> ImageResult<ImageBuffer<P, Vec<u8>>> {
    #![expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

    let JpegArgs {
        iterations,
        quality,
        size,
        down_filter,
        up_filter,
    } = args;

    let (width, height) = (image.width(), image.height());
    let mut jpeg_bytes = vec![];

    for i in 0..iterations {
        let (nwidth, nheight, filter) = if i % 2 == 0 {
            if width.max(height) > size {
                if width > height {
                    (
                        size,
                        (f64::from(size) * (f64::from(height) / f64::from(width))).round() as u32,
                        down_filter,
                    )
                } else {
                    (
                        (f64::from(size) * (f64::from(width) / f64::from(height))).round() as u32,
                        size,
                        down_filter,
                    )
                }
            } else {
                (width, height, FilterType::Nearest)
            }
        } else {
            (width, height, up_filter)
        };

        trace!(
            width = image.width(),
            height = image.height(),
            nwidth,
            nheight,
            ?filter,
            "Resizing image..."
        );
        let mut pixels = imageops::resize(&image, nwidth, nheight, filter).into_raw();

        trace!(quality, "JPEGing image");
        jpeg_bytes.clear();
        let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_bytes, quality);
        encoder.encode(&pixels, nwidth, nheight, P::COLOR_TYPE)?;
        pixels.clear();
        let decoder = JpegDecoder::new(Cursor::new(&*jpeg_bytes))?;
        pixels.resize(
            decoder
                .total_bytes()
                .try_into()
                .unwrap_or_else(|_| unreachable!()),
            0,
        );
        decoder.read_image(&mut pixels)?;

        image = ImageBuffer::from_vec(nwidth, nheight, pixels).expect("Wrong buffer size?");
    }

    if iterations % 2 != 0 {
        trace!(
            width = image.width(),
            height = image.height(),
            nwidth = width,
            nheight = height,
            ?up_filter,
            "Performing final resize..."
        );
        image = imageops::resize(&image, width, height, up_filter);
    }

    Ok(image)
}

/// Apply JPEG compression to the given [`DynamicImage`]
///
/// # Errors
/// This function returns an error if the JPEG transcoder fails
pub fn jpeg_dynamic_image(image: DynamicImage, args: JpegArgs) -> Result<DynamicImage, Error> {
    use DynamicImage::{ImageLuma8, ImageLumaA8, ImageRgb8, ImageRgba8};
    Ok(match image {
        ImageLuma8(image) => ImageLuma8(jpeg_buffer(image, args)?),
        ImageLumaA8(image) => ImageLuma8(jpeg_buffer(image.convert(), args)?),
        ImageRgb8(image) => ImageRgb8(jpeg_buffer(image, args)?),
        ImageRgba8(image) => ImageRgb8(jpeg_buffer(image.convert(), args)?),
        image => return Err(Error::UnsupportedColorType(image.color())),
    })
}
