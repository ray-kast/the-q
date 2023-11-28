use wide::u32x8;

#[derive(Debug, Default)]
#[repr(C, align(32))]
pub struct ShortArray(pub [u32; Self::WIDTH]);

impl ShortArray {
    // Number of bytes intended to be stored in ShortArray, NOT its byte length
    pub const BYTE_WIDTH: usize = ShortArray::WIDTH * 2;
    pub const WIDTH: usize = 8;

    /// Bithackery to remap the stored values such that the UTF-16 surrogate
    /// pair range is avoided.  The valid scalar input range for this function
    /// is `0x00..(0x110000 - 0x800)`
    pub fn encode(&self) -> u32x8 {
        (u32x8::from(self.0) + u32x8::splat(0x800)) ^ u32x8::splat(0xd800)
    }

    /// Inverse of [`encode`](Self::encode)
    pub fn decode(&self) -> u32x8 {
        (u32x8::from(self.0) ^ u32x8::splat(0xd800)) - u32x8::splat(0x800)
    }
}
