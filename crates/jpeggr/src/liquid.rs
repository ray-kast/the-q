//! Apply content-aware scale to an image

use image::{imageops, DynamicImage, ImageBuffer, Rgba};

use crate::Error;

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
    /// Maximum transverse width of seams (i.e. curliness)
    pub curly_seams: f64,
    /// Bias for non-straight seams
    pub bias_curly: f64,
}

type Quantum = magick_sys::Quantum;

/// Apply content-aware scale to the given image buffer
///
/// # Errors
/// This function returns an error if applying the resiza with `MagickCore`
/// fails
///
/// # Panics
/// This function panics if `MagickCore` produces an invalid buffer or is using
/// an incompatible static configuration
pub fn liquid_buffer(
    mut image: ImageBuffer<Rgba<Quantum>, Vec<Quantum>>,
    args: LiquidArgs,
) -> Result<ImageBuffer<Rgba<Quantum>, Vec<Quantum>>, Error> {
    #![expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

    let LiquidArgs {
        max_input_size,
        x_fac,
        y_fac,
        curly_seams,
        bias_curly,
    } = args;

    let (nwidth, nheight) = {
        let width = image.width();
        let height = image.height();

        if width.max(height) > max_input_size {
            if width > height {
                (
                    max_input_size,
                    (f64::from(height) * f64::from(max_input_size) / f64::from(width)).round()
                        as u32,
                )
            } else {
                (
                    (f64::from(width) * f64::from(max_input_size) / f64::from(height)).round()
                        as u32,
                    max_input_size,
                )
            }
        } else {
            (width, height)
        }
    };

    image = imageops::resize(&image, nwidth, nheight, imageops::FilterType::CatmullRom);

    unsafe {
        let mut exc = magick_sys::Exceptions::new();

        let width = usize::try_from(image.width()).unwrap();
        let height = usize::try_from(image.height()).unwrap();

        let pixels = &mut *image;
        assert!(pixels.len() == width * height * 4);

        let mut image = exc.catch(|e| {
            magick_sys::ImageHandle::from_raw(magick_sys::ConstituteImage(
                width,
                height,
                c"RGBA".as_ptr(),
                magick_sys::StorageType_FloatPixel,
                pixels.as_ptr().cast(),
                e,
            ))
        })?;

        let width = (f64::from(nwidth) * x_fac).round() as u32;
        let height = (f64::from(nheight) * y_fac).round() as u32;

        let swidth = usize::try_from(width).unwrap();
        let sheight = usize::try_from(height).unwrap();

        let iwidth = isize::try_from(width).unwrap();
        let iheight = isize::try_from(height).unwrap();

        image = exc.catch(|e| {
            magick_sys::ImageHandle::from_raw(magick_sys::LiquidRescaleImage(
                image.as_ptr(),
                swidth,
                sheight,
                curly_seams,
                bias_curly,
                e,
            ))
        })?;

        let mut out_pixels = vec![];
        let mut buf = [0.0_f32; magick_sys::MaxPixelChannels as usize];

        assert_eq!(magick_sys::MagickQuantumRange, b"65535\0");

        for y in 0..iheight {
            for x in 0..iwidth {
                exc.catch(|e| {
                    magick_sys::GetOneVirtualPixel(
                        image.as_ptr(),
                        x,
                        y,
                        magick_sys::pack_quanta(buf.as_mut_ptr()),
                        e,
                    )
                })?;

                out_pixels.extend_from_slice(&[
                    buf[magick_sys::PixelChannel_RedPixelChannel as usize] / 65535.0,
                    buf[magick_sys::PixelChannel_GreenPixelChannel as usize] / 65535.0,
                    buf[magick_sys::PixelChannel_BluePixelChannel as usize] / 65535.0,
                    buf[magick_sys::PixelChannel_AlphaPixelChannel as usize] / 65535.0,
                ]);
            }
        }

        Ok(ImageBuffer::from_raw(width, height, out_pixels).expect("Wrong buffer size?"))
    }
}

/// Apply content-aware scale to the given image buffer
///
/// # Errors
/// This function returns an error if applying the resiza with `MagickCore`
/// fails
///
/// # Panics
/// This function panics if `MagickCore` produces an invalid buffer or is using
/// an incompatible static configuration
pub fn liquid_dynamic_image(image: DynamicImage, args: LiquidArgs) -> Result<DynamicImage, Error> {
    Ok(DynamicImage::ImageRgba32F(liquid_buffer(
        image.into_rgba32f(),
        args,
    )?))
}
