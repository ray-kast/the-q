use std::io;

use wide::u32x8;

use super::TRAIL_MASK;

const WIDTH: usize = 8;
const BYTE_WIDTH: usize = WIDTH * 2;

#[derive(Debug, Default)]
#[repr(C, align(32))]
struct ShortArray([u32; WIDTH]);

impl ShortArray {
    /// Bithackery to remap the stored values such that the UTF-16 surrogate
    /// pair range is avoided.  The valid scalar input range for this function
    /// is `0x00..(0x110000 - 0x800)`
    fn prepare(&self) -> u32x8 {
        (u32x8::from(self.0) + u32x8::splat(0x800)) ^ u32x8::splat(0xd800)
    }
}

/// Encoder for storing base64k data into a sequence of `char`s
#[derive(Debug, Default)]
pub struct Encoder<C> {
    curr_byte: usize,
    arr: ShortArray,
    chars: C,
}

impl<C: Extend<char>> Encoder<C> {
    #[inline]
    fn next(&self) -> (usize, bool) {
        let byte = (self.curr_byte & 1) != 0;
        let idx = self.curr_byte >> 1;
        debug_assert!(idx < WIDTH);
        (idx, byte)
    }

    #[inline]
    fn extend_chars(&mut self, i: impl IntoIterator<Item = u32>) {
        self.chars.extend(i.into_iter().map(|i| {
            if cfg!(debug_assertions) {
                char::from_u32(i).unwrap_or_else(|| unreachable!())
            } else {
                unsafe { char::from_u32_unchecked(i) }
            }
        }));
    }

    #[inline]
    fn flush_arr_full(&mut self) {
        debug_assert_eq!(self.curr_byte, BYTE_WIDTH);
        self.extend_chars(self.arr.prepare().to_array());
        self.curr_byte = 0;
    }

    fn flush_arr_partial(&mut self) {
        if self.curr_byte == BYTE_WIDTH {
            return self.flush_arr_full();
        }

        let (idx, byte) = self.next();
        self.curr_byte = 0;

        let idx = if byte {
            // SAFETY: idx is checked by self.curr
            unsafe {
                let dw = self.arr.0.get_unchecked_mut(idx);
                assert_eq!(*dw & !0x00ff, 0);
                // Mark the final word as trailing
                *dw |= TRAIL_MASK;
            }
            idx + 1
        } else {
            idx
        };

        self.extend_chars(
            // SAFETY: idx is previously known to be < len, and is never
            //         incremented by more than 1
            unsafe { self.arr.prepare().to_array().get_unchecked(0..idx) }
                .iter()
                .copied(),
        );
    }

    /// Flush the internal buffer and return the encoded data
    #[inline]
    #[must_use]
    pub fn finish(mut self) -> C {
        self.flush_arr_partial();
        self.chars
    }
}

#[inline]
unsafe fn split<T>(arr: &[T], i: usize) -> (&[T], &[T]) {
    if cfg!(debug_assertions) {
        arr.split_at(i)
    } else {
        (arr.get_unchecked(..i), arr.get_unchecked(i..))
    }
}

impl<C: Extend<char>> io::Write for Encoder<C> {
    fn write(&mut self, mut buf: &[u8]) -> io::Result<usize> {
        let buf_len = buf.len();

        loop {
            debug_assert!(BYTE_WIDTH > self.curr_byte);
            let len = buf.len().min(BYTE_WIDTH - self.curr_byte);

            if len == 0 {
                break;
            }

            let (seg, buf2) = unsafe { split(buf, len) };
            buf = buf2;
            debug_assert!(seg.len() == len);

            for &inp in seg {
                let (idx, byte) = self.next();

                // SAFETY: idx is checked by self.curr
                let out = unsafe { self.arr.0.get_unchecked_mut(idx) };

                let inp = u32::from(inp);
                let byte = u32::from(byte);
                *out ^= ((byte ^ 0x1) * (*out ^ inp)) | (byte * (inp << 8));
                #[cfg(debug_assertions)]
                {
                    if byte == 0 {
                        assert_eq!(*out, inp);
                    } else {
                        assert_eq!(*out & 0xff00, inp << 8);
                        assert_eq!(*out & !0xffff, 0);
                    }
                }

                self.curr_byte += 1;
            }

            if self.curr_byte == BYTE_WIDTH {
                self.flush_arr_full();
            }
        }

        Ok(buf_len)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.flush_arr_partial();
        Ok(())
    }
}

impl<C: IntoIterator + Extend<char>> IntoIterator for Encoder<C> {
    type IntoIter = <C as IntoIterator>::IntoIter;
    type Item = <C as IntoIterator>::Item;

    #[inline]
    fn into_iter(self) -> Self::IntoIter { self.finish().into_iter() }
}

#[cfg(test)]
mod test {
    use std::io::prelude::*;

    use super::Encoder;
    use crate::test::{encode1, encode2};

    fn zip_eq<A: IntoIterator, B: IntoIterator<Item = char>>(a: A, b: B)
    where
        A::Item: Into<u32>,
        A::IntoIter: ExactSizeIterator,
        B::IntoIter: ExactSizeIterator,
    {
        let a = a.into_iter();
        let b = b.into_iter();
        assert_eq!(a.len(), b.len(), "Length mismatch");

        for (i, (a, b)) in a.map(Into::into).zip(b.map(Into::into)).enumerate() {
            assert_eq!(a, b, "Mismatch at index {i}: {a:#08x} vs {b:#08x}");
        }
    }

    #[test]
    fn test_small() {
        zip_eq(Encoder::<Vec<char>>::default(), []);
        let mut enc = Encoder::<Vec<char>>::default();
        enc.write_all(b"a").unwrap();
        zip_eq(enc, [encode1(b'a')]);
        enc = Encoder::default();
        enc.write_all(&[0, 1]).unwrap();
        zip_eq(enc, [encode2(0, 1)]);
    }

    #[test]
    fn test_even() {
        let mut enc = Encoder::<Vec<char>>::default();
        enc.write_all(b"even").unwrap();
        zip_eq(enc, [encode2(b'e', b'v'), encode2(b'e', b'n')]);
    }

    #[test]
    fn test_odd() {
        let mut enc = Encoder::<Vec<char>>::default();
        enc.write_all(b"odd").unwrap();
        zip_eq(enc, [encode2(b'o', b'd'), encode1(b'd')]);
    }

    #[test]
    fn test_long() {
        let odd = b"the quick brown fox jumps over the lazy dog";
        let even = b"the quick brown fox jumps over the lazy dog!";

        let odd_enc = odd.chunks(2).map(|c| match *c {
            [a, b] => encode2(a, b),
            [a] => encode1(a),
            _ => unreachable!(),
        });
        let even_enc = even.chunks_exact(2).map(|c| {
            let [a, b] = *c else { unreachable!() };
            encode2(a, b)
        });

        let mut enc = Encoder::<Vec<char>>::default();
        enc.write_all(odd).unwrap();
        zip_eq(enc, odd_enc);

        enc = Encoder::default();
        enc.write_all(even).unwrap();
        zip_eq(enc, even_enc);
    }
}
