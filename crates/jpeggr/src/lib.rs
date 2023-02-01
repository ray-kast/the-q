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
#![allow(missing_docs)] // TODO

use std::fmt::Display;

pub use image;
use image::{
    buffer::ConvertBuffer,
    codecs::jpeg::{JpegDecoder, JpegEncoder},
    ColorType, DynamicImage, ImageBuffer, ImageDecoder, ImageError, ImageResult, Pixel,
    PixelWithColorType,
};

pub fn jpeg_pixels(
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    color_type: ColorType,
    iterations: usize,
    quality: u8,
) -> ImageResult<Vec<u8>> {
    let mut decoded_data = pixels;
    let mut encoded_data = Vec::new();

    for _ in 0..iterations {
        encoded_data.clear();
        let mut encoder = JpegEncoder::new_with_quality(&mut encoded_data, quality);
        encoder.encode(&decoded_data, width, height, color_type)?;
        decoded_data.clear();
        let decoder = JpegDecoder::new(&*encoded_data)?;
        #[allow(clippy::cast_possible_truncation)]
        decoded_data.resize_with(decoder.total_bytes() as usize, Default::default);
        decoder.read_image(&mut decoded_data)?;
    }

    Ok(decoded_data)
}

pub fn jpeg_buffer<P>(
    image: ImageBuffer<P, Vec<u8>>,
    iterations: usize,
    quality: u8,
) -> ImageResult<ImageBuffer<P, Vec<u8>>>
where
    P: PixelWithColorType + Pixel<Subpixel = u8>,
{
    let (width, height, color_type) = (image.width(), image.height(), P::COLOR_TYPE);
    let data = jpeg_pixels(
        image.into_raw(),
        width,
        height,
        color_type,
        iterations,
        quality,
    )?;
    Ok(ImageBuffer::from_vec(width, height, data).expect("Wrong buffer size?"))
}

#[derive(Debug)]
pub enum Error {
    Image(ImageError),
    UnsupportedColorType(ColorType),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Image(err) => err.fmt(f),
            Error::UnsupportedColorType(color_type) => {
                write!(f, "Unsupported color type {color_type:?}")
            },
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> { None }
}

impl From<ImageError> for Error {
    fn from(err: ImageError) -> Error { Error::Image(err) }
}

pub fn jpeg_dynamic_image(
    image: DynamicImage,
    iterations: usize,
    quality: u8,
) -> Result<DynamicImage, Error> {
    use DynamicImage::{ImageLuma8, ImageLumaA8, ImageRgb8, ImageRgba8};
    Ok(match image {
        ImageLuma8(image) => ImageLuma8(jpeg_buffer(image, iterations, quality)?),
        ImageLumaA8(image) => ImageLuma8(jpeg_buffer(image.convert(), iterations, quality)?),
        ImageRgb8(image) => ImageRgb8(jpeg_buffer(image, iterations, quality)?),
        ImageRgba8(image) => ImageRgb8(jpeg_buffer(image.convert(), iterations, quality)?),
        image => return Err(Error::UnsupportedColorType(image.color())),
    })
}
