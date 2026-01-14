//! Apply content-aware scale to an image

use image::{imageops, DynamicImage, ImageBuffer, Rgba};
use lqr_sys::Carver;

use crate::Error;

/// Output resizing mode for the liquid functions
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResizeOutput {
    /// Keep output size
    OutputSize,
    /// Resize to fit within input size, preserving aspect ratio
    FitToInput,
    /// Resize to exact input size
    StretchToInput,
    /// Resize to fit within input size if output is smaller
    Upsample,
}

/// Common arguments to the liquid functions
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiquidArgs {
    /// Maximum input resolution in either dimension
    ///
    /// Inputs larger than this will be downsampled for performance reasons
    pub max_input_size: u32,
    /// X resolution scale factor
    pub x_fac: f64,
    /// Y resolution scale factor
    pub y_fac: f64,
    /// Maximum transverse slope for seams (i.e. maximum curl)
    pub seam_wander: u16,
    /// Bias towards more rigid seam paths
    pub seam_rigidity: f32,
    /// Resize output image back to original size
    pub resize_output: ResizeOutput,
}

/// Apply content-aware scale to the given image buffer
///
/// # Errors
/// This function returns an error if applying the resize with `liblqr` fails
///
/// # Panics
/// This function panics if the image is an invalid size
pub fn liquid_buffer(
    mut image: ImageBuffer<Rgba<f32>, Vec<f32>>,
    args: LiquidArgs,
) -> Result<ImageBuffer<Rgba<f32>, Vec<f32>>, Error> {
    #![expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

    let LiquidArgs {
        max_input_size,
        x_fac,
        y_fac,
        seam_wander,
        seam_rigidity,
        resize_output,
    } = args;

    let orig_width = image.width();
    let orig_height = image.height();
    let (resize_output, ignore_aspect_ratio) = match resize_output {
        ResizeOutput::OutputSize => (false, false),
        ResizeOutput::FitToInput => (true, false),
        ResizeOutput::StretchToInput => (true, true),
        ResizeOutput::Upsample => (true, x_fac <= 1.0 && y_fac <= 1.0),
    };

    let (nwidth, nheight) = {
        if orig_width.max(orig_height) > max_input_size {
            if orig_width > orig_height {
                (
                    max_input_size,
                    (f64::from(orig_height) * f64::from(max_input_size) / f64::from(orig_width))
                        .round() as u32,
                )
            } else {
                (
                    (f64::from(orig_width) * f64::from(max_input_size) / f64::from(orig_height))
                        .round() as u32,
                    max_input_size,
                )
            }
        } else {
            (orig_width, orig_height)
        }
    };

    image = imageops::resize(&image, nwidth, nheight, imageops::FilterType::CatmullRom);

    let mut carver = Carver::new(
        &image,
        image.width(),
        image.height(),
        lqr_sys::ColorType::Rgba,
        seam_wander,
        seam_rigidity,
    )?;

    let nwidth = (f64::from(image.width()) * x_fac).round() as u32;
    let nheight = (f64::from(image.height()) * y_fac).round() as u32;
    carver.resize(nwidth, nheight)?;

    let mut buf =
        vec![0.0; usize::try_from(nwidth).unwrap() * usize::try_from(nheight).unwrap() * 4];
    carver.read(&mut buf)?;

    drop(carver);

    image = ImageBuffer::from_raw(nwidth, nheight, buf).expect("Wrong buffer size?");

    let width = image.width();
    let height = image.height();

    if resize_output {
        let (nwidth, nheight) = if ignore_aspect_ratio {
            (orig_width, orig_height)
        } else {
            let x_scale = f64::from(orig_width) / f64::from(width);
            let y_scale = f64::from(orig_height) / f64::from(height);

            if x_scale < y_scale {
                (orig_width, (f64::from(height) * x_scale).round() as u32)
            } else {
                ((f64::from(width) * y_scale).round() as u32, orig_height)
            }
        };

        image = imageops::resize(&image, nwidth, nheight, imageops::FilterType::CatmullRom);
    }

    Ok(image)
}

/// Apply content-aware scale to the given image buffer
///
/// # Errors
/// This function returns an error if applying the resize with `liblqr` fails
///
/// # Panics
/// This function panics if the image is an invalid size
pub fn liquid_dynamic_image(image: DynamicImage, args: LiquidArgs) -> Result<DynamicImage, Error> {
    Ok(DynamicImage::ImageRgba32F(liquid_buffer(
        image.into_rgba32f(),
        args,
    )?))
}
