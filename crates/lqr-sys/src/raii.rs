use std::{
    marker::PhantomData,
    ptr::{self, NonNull},
};

use crate::{
    lqr_carver_destroy, lqr_carver_get_channels, lqr_carver_get_height, lqr_carver_get_width,
    lqr_carver_init, lqr_carver_new_ext, lqr_carver_resize, lqr_carver_scan_by_row,
    lqr_carver_scan_line_ext, lqr_carver_scan_reset, lqr_carver_set_image_type,
    lqr_carver_set_preserve_input_image, Error, LqrCarver, _LqrColDepth,
    _LqrColDepth_LQR_COLDEPTH_16I, _LqrColDepth_LQR_COLDEPTH_32F, _LqrColDepth_LQR_COLDEPTH_64F,
    _LqrColDepth_LQR_COLDEPTH_8I, _LqrImageType_LQR_CMYKA_IMAGE, _LqrImageType_LQR_CMYK_IMAGE,
    _LqrImageType_LQR_CMY_IMAGE, _LqrImageType_LQR_CUSTOM_IMAGE, _LqrImageType_LQR_GREYA_IMAGE,
    _LqrImageType_LQR_GREY_IMAGE, _LqrImageType_LQR_RGBA_IMAGE, _LqrImageType_LQR_RGB_IMAGE,
};

mod imp {
    pub trait ChannelSealed {}

    impl ChannelSealed for u8 {}
    impl ChannelSealed for u16 {}
    impl ChannelSealed for f32 {}
    impl ChannelSealed for f64 {}
}

pub trait Channel: Copy + imp::ChannelSealed {
    const DEPTH: _LqrColDepth;
}

impl Channel for u8 {
    const DEPTH: _LqrColDepth = _LqrColDepth_LQR_COLDEPTH_8I;
}

impl Channel for u16 {
    const DEPTH: _LqrColDepth = _LqrColDepth_LQR_COLDEPTH_16I;
}

impl Channel for f32 {
    const DEPTH: _LqrColDepth = _LqrColDepth_LQR_COLDEPTH_32F;
}

impl Channel for f64 {
    const DEPTH: _LqrColDepth = _LqrColDepth_LQR_COLDEPTH_64F;
}

pub struct Carver<'a, C>(NonNull<LqrCarver>, PhantomData<&'a [C]>);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorType {
    Luma,
    LumaA,
    Rgb,
    Rgba,
    Cmy,
    Cmyk,
    CmykA,
    Custom(u8),
}

impl ColorType {
    pub fn channels(self) -> u8 {
        match self {
            Self::Luma => 1,
            Self::LumaA => 2,
            Self::Rgb => 3,
            Self::Rgba => 4,
            Self::Cmy => 3,
            Self::Cmyk => 4,
            Self::CmykA => 5,
            Self::Custom(c) => c,
        }
    }
}

impl<'a, C: Channel> Carver<'a, C> {
    pub fn new(
        buffer: &'a [C],
        width: u32,
        height: u32,
        color: ColorType,
        delta_x: u16,
        rigidity: f32,
    ) -> Result<Self, Error> {
        assert!(
            buffer.len()
                == usize::try_from(width).unwrap()
                    * usize::try_from(height).unwrap()
                    * usize::from(color.channels())
        );

        let ptr = unsafe {
            lqr_carver_new_ext(
                buffer.as_ptr().cast_mut().cast(),
                width.try_into()?,
                height.try_into()?,
                color.channels().into(),
                C::DEPTH,
            )
        };

        let ptr = NonNull::new(ptr).ok_or(Error::Other)?;

        unsafe {
            lqr_carver_set_preserve_input_image(ptr.as_ptr());
            Error::from_code(lqr_carver_init(ptr.as_ptr(), delta_x.into(), rigidity))?;
            Error::from_code(lqr_carver_set_image_type(ptr.as_ptr(), match color {
                ColorType::Luma => _LqrImageType_LQR_GREY_IMAGE,
                ColorType::LumaA => _LqrImageType_LQR_GREYA_IMAGE,
                ColorType::Rgb => _LqrImageType_LQR_RGB_IMAGE,
                ColorType::Rgba => _LqrImageType_LQR_RGBA_IMAGE,
                ColorType::Cmy => _LqrImageType_LQR_CMY_IMAGE,
                ColorType::Cmyk => _LqrImageType_LQR_CMYK_IMAGE,
                ColorType::CmykA => _LqrImageType_LQR_CMYKA_IMAGE,
                ColorType::Custom(_) => _LqrImageType_LQR_CUSTOM_IMAGE,
            }))?;
        }

        Ok(Self(ptr, PhantomData))
    }

    pub fn resize(&mut self, nwidth: u32, nheight: u32) -> Result<(), Error> {
        unsafe {
            Error::from_code(lqr_carver_resize(
                self.0.as_ptr(),
                nwidth.try_into()?,
                nheight.try_into()?,
            ))
        }
    }

    pub fn read(&mut self, buf: &mut [C]) -> Result<(), Error> {
        let by_row;
        let width;
        let height;
        let channels;

        unsafe {
            lqr_carver_scan_reset(self.0.as_ptr());
            by_row = lqr_carver_scan_by_row(self.0.as_ptr()) != 0;
            width = usize::try_from(lqr_carver_get_width(self.0.as_ptr()))?;
            height = usize::try_from(lqr_carver_get_height(self.0.as_ptr()))?;
            channels = usize::try_from(lqr_carver_get_channels(self.0.as_ptr()))?;
        }

        assert!(buf.len() == width * height * channels);

        let stride = width * channels;

        let mut n = 0;
        let mut line = ptr::null_mut();

        while unsafe { lqr_carver_scan_line_ext(self.0.as_ptr(), &raw mut n, &raw mut line) != 0 } {
            if line.is_null() || !line.is_aligned() {
                return Err(Error::Other);
            }

            let n = usize::try_from(n).unwrap();
            if by_row {
                let offs = n * stride;

                unsafe {
                    buf.get_unchecked_mut(offs..offs + stride)
                        .copy_from_slice(&*ptr::slice_from_raw_parts(line.cast::<C>(), stride));
                }
            } else {
                for y in 0..height {
                    let offs = (y * width + n) * channels;

                    unsafe {
                        buf.get_unchecked_mut(offs..offs + channels)
                            .copy_from_slice(&*ptr::slice_from_raw_parts(
                                line.cast::<C>().add(y * channels),
                                channels,
                            ));
                    }
                }
            }
        }

        Ok(())
    }
}

impl<'a, C> Drop for Carver<'a, C> {
    fn drop(&mut self) {
        unsafe {
            lqr_carver_destroy(self.0.as_ptr());
        }
    }
}
