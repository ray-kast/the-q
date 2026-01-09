//! A crate for destroying images

use image::{ColorType, ImageError};

pub mod jpeg;
pub mod liquid;

pub extern crate image;

/// An error arising from this crate
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The [`image`] crate raised an error
    #[error("Image format error")]
    Image(#[from] ImageError),
    /// The ImageMagick library raised an error
    #[error("ImageMagick error")]
    Magick(#[from] magick_sys::Errors),
    /// A [`ColorType`] was encountered that was not supported
    #[error("Unsupported color type {0:?}")]
    UnsupportedColorType(ColorType),
}
