use std::ops::Deref;

use image::{ImageBuffer, Rgba};
use magick_sys::{
    ConstituteImage, Exceptions, GetOneVirtualPixel, ImageHandle, MagickQuantumRange,
    MaxPixelChannels, Quantum, StorageType_FloatPixel,
};

use crate::Error;

pub unsafe fn process<
    C: Deref<Target = [Quantum]>,
    F: FnOnce(ImageHandle, &mut Exceptions) -> Result<ImageHandle, E>,
    E,
>(
    image: &ImageBuffer<Rgba<Quantum>, C>,
    f: F,
) -> Result<ImageBuffer<Rgba<Quantum>, Vec<Quantum>>, Error>
where
    Error: From<E>,
{
    let mut exc = Exceptions::new();

    let mut magick_image = {
        let width = usize::try_from(image.width()).unwrap();
        let height = usize::try_from(image.height()).unwrap();

        assert!(image.len() == width * height * 4);

        unsafe {
            exc.catch(|e| {
                ImageHandle::from_raw(ConstituteImage(
                    usize::try_from(image.width()).unwrap(),
                    usize::try_from(image.height()).unwrap(),
                    c"RGBA".as_ptr(),
                    StorageType_FloatPixel,
                    image.as_ptr().cast(),
                    e,
                ))
            })?
        }
    };

    magick_image = f(magick_image, &mut exc)?;

    let (width, height) = unsafe {
        let image = magick_image.as_ref();
        (
            u32::try_from(image.columns).expect("Image is too wide"),
            u32::try_from(image.rows).expect("Image is too tall"),
        )
    };

    let iwidth = isize::try_from(width).unwrap();
    let iheight = isize::try_from(height).unwrap();

    let mut out_pixels = vec![];
    let mut buf = [0.0_f32; MaxPixelChannels as usize];

    assert_eq!(MagickQuantumRange, b"65535\0");

    for y in 0..iheight {
        for x in 0..iwidth {
            unsafe {
                exc.catch(|e| {
                    GetOneVirtualPixel(magick_image.as_ptr(), x, y, buf.as_mut_ptr(), e)
                })?;

                out_pixels.extend_from_slice(&[
                    buf[magick_sys::PixelChannel_RedPixelChannel as usize] / 65535.0,
                    buf[magick_sys::PixelChannel_GreenPixelChannel as usize] / 65535.0,
                    buf[magick_sys::PixelChannel_BluePixelChannel as usize] / 65535.0,
                    buf[magick_sys::PixelChannel_AlphaPixelChannel as usize] / 65535.0,
                ]);
            }
        }
    }

    Ok(ImageBuffer::from_raw(width, height, out_pixels).expect("Wrong buffer size?"))
}
