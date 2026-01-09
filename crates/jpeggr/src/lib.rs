//! A crate for destroying images

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
    /// The `ImageMagick` library raised an error
    #[error("ImageMagick error")]
    Magick(#[from] magick_sys::Errors),
    /// A [`ColorType`] was encountered that was not supported
    #[error("Unsupported color type {0:?}")]
    UnsupportedColorType(ColorType),
}
