use wide::u32x8;

use crate::TRAIL_MASK;

#[derive(Debug, Default)]
#[repr(C, align(32))]
pub struct ShortArray(pub [u32; Self::WIDTH]);

#[cfg(not(miri))]
#[repr(transparent)]
pub struct Vector(u32x8);

#[cfg(miri)]
#[repr(transparent)]
pub struct Vector([u32; ShortArray::WIDTH]);

impl ShortArray {
    // Number of bytes intended to be stored in ShortArray, NOT its byte length
    pub const BYTE_WIDTH: usize = ShortArray::WIDTH * 2;
    pub const WIDTH: usize = 8;

    /// Bithackery to remap the stored values such that the UTF-16 surrogate
    /// pair range is avoided.  The valid scalar input range for this function
    /// is `0x00..(0x110000 - 0x800)`
    #[cfg(not(miri))]
    #[inline]
    pub fn encode(&self) -> Vector {
        Vector((u32x8::from(self.0) + u32x8::splat(0x800)) ^ u32x8::splat(0xd800))
    }

    /// Inverse of [`encode`](Self::encode)
    #[cfg(not(miri))]
    #[inline]
    pub fn decode(&self) -> Vector {
        Vector((u32x8::from(self.0) ^ u32x8::splat(0xd800)) - u32x8::splat(0x800))
    }

    #[cfg(any(miri, test))]
    fn encode_miri(&self) -> [u32; ShortArray::WIDTH] {
        let mut arr = self.0;
        for i in &mut arr {
            *i = (*i + 0x800) ^ 0xd800;
        }
        arr
    }

    #[cfg(any(miri, test))]
    fn decode_miri(&self) -> [u32; ShortArray::WIDTH] {
        let mut arr = self.0;
        for i in &mut arr {
            *i = (*i ^ 0xd800) - 0x800;
        }
        arr
    }

    #[cfg(miri)]
    pub fn encode(&self) -> Vector { Vector(self.encode_miri()) }

    #[cfg(miri)]
    pub fn decode(&self) -> Vector { Vector(self.decode_miri()) }
}

impl Vector {
    #[cfg(not(miri))]
    #[inline]
    pub fn trail_mask_hint(&self) -> bool { self.0 & u32x8::splat(TRAIL_MASK) != u32x8::ZERO }

    #[cfg(not(miri))]
    #[inline]
    pub fn to_array(&self) -> [u32; ShortArray::WIDTH] { self.0.to_array() }

    #[cfg(miri)]
    pub fn trail_mask_hint(&self) -> bool { self.0.iter().any(|i| i & TRAIL_MASK != 0) }

    #[cfg(miri)]
    pub fn to_array(&self) -> [u32; ShortArray::WIDTH] { self.0 }
}

#[cfg(test)]
mod test {
    use super::ShortArray;

    // Sanity check for MIRI-friendly non-vectorized encode
    #[cfg(not(miri))]
    #[test]
    fn test_encode_miri() {
        for i in 0..0xffff {
            let arr = ShortArray([i; 8]);
            assert_eq!(arr.encode().to_array(), arr.encode_miri());
        }
    }

    // Sanity check for MIRI-friendly non-vectorized decode
    #[cfg(not(miri))]
    #[test]
    fn test_decode_miri() {
        for i in 0..0xffff {
            let arr = ShortArray([i; 8]);
            let arr = ShortArray(arr.encode().to_array());
            assert_eq!(arr.decode().to_array(), arr.decode_miri());
        }
    }
}
