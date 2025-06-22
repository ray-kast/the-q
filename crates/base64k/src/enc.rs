use std::io;

use super::TRAIL_MASK;
use crate::arr::ShortArray;

/// Encoder for storing base64k data into a sequence of `char`s
#[derive(Debug, Default)]
pub struct Encoder<C> {
    curr_byte: usize,
    arr: ShortArray,
    chars: C,
}

impl<C: Extend<char>> Encoder<C> {
    // Returns (word_index, high_byte)
    #[inline]
    fn next(&self) -> (usize, bool) {
        let high = (self.curr_byte & 1) != 0;
        let idx = self.curr_byte >> 1;
        debug_assert!(idx < ShortArray::WIDTH);
        (idx, high)
    }

    #[inline]
    unsafe fn extend_chars(&mut self, i: impl IntoIterator<Item = u32>) {
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
        debug_assert_eq!(self.curr_byte, ShortArray::BYTE_WIDTH);
        // SAFETY: ShortArray::prepare transposes all u32 values that would be
        //         invalid chars into a valid range
        unsafe { self.extend_chars(self.arr.encode().to_array()) };
        self.curr_byte = 0;
    }

    fn flush_arr_partial(&mut self) {
        if self.curr_byte == ShortArray::BYTE_WIDTH {
            return self.flush_arr_full();
        }

        let (idx, high) = self.next();
        self.curr_byte = 0;

        // The next byte would have been a high byte, thus the final word has
        // a trailing byte
        let idx = if high {
            // SAFETY: idx is checked by self.next()
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

        unsafe {
            // SAFETY: ShortArray::prepare transposes all u32 values that would
            //         be invalid chars into a valid range
            self.extend_chars(
                // SAFETY: idx is previously known to be < len, and is never
                //         incremented by more than 1
                self.arr
                    .encode()
                    .to_array()
                    .get_unchecked(0..idx)
                    .iter()
                    .copied(),
            );
        }
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
        unsafe { (arr.get_unchecked(..i), arr.get_unchecked(i..)) }
    }
}

impl<C: Extend<char>> io::Write for Encoder<C> {
    fn write(&mut self, mut buf: &[u8]) -> io::Result<usize> {
        let buf_len = buf.len();

        loop {
            debug_assert!(ShortArray::BYTE_WIDTH > self.curr_byte);
            let len = buf.len().min(ShortArray::BYTE_WIDTH - self.curr_byte);

            if len == 0 {
                break;
            }

            // SAFETY: len is computed to be at most buf.len()
            let (seg, buf2) = unsafe { split(buf, len) };
            buf = buf2;
            debug_assert!(seg.len() == len);

            for &inp in seg {
                let (idx, high) = self.next();

                // SAFETY: idx is checked by self.next()
                let out = unsafe { self.arr.0.get_unchecked_mut(idx) };

                // Branchless necromancy.
                // if (high) out |= inp << 8 else out = inp
                let inp = u32::from(inp);
                let high = u32::from(high);
                *out ^= ((high ^ 0x1) * (*out ^ inp)) | (high * (inp << 8));
                #[cfg(debug_assertions)]
                {
                    // Make sure we don't get our necromancy license revoked
                    if high == 0 {
                        assert_eq!(*out, inp);
                    } else {
                        assert_eq!(*out & 0xff00, inp << 8);
                        assert_eq!(*out & !0xffff, 0);
                    }
                }

                self.curr_byte += 1;
            }

            if self.curr_byte == ShortArray::BYTE_WIDTH {
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

    fn zip_eq<A: AsRef<[u8]>, B: IntoIterator<Item = char, IntoIter: ExactSizeIterator>>(
        pathological: usize,
        a: A,
        b: B,
    ) {
        let mut enc = Encoder::<Vec<char>>::default();
        if pathological > 0 {
            let mut slice = a.as_ref();

            loop {
                match enc.write(&slice[..pathological.min(slice.len())]).unwrap() {
                    0 => break,
                    n => slice = &slice[n..],
                }
            }
        } else {
            enc.write_all(a.as_ref()).unwrap();
        }

        let a = enc.into_iter();
        let b = b.into_iter();
        let a_len = a.len();
        let b_len = b.len();

        for (i, (a, b)) in a.map(u32::from).zip(b.map(Into::into)).enumerate() {
            assert_eq!(a, b, "Mismatch at index {i}: {a:#08x} vs {b:#08x}");
        }

        assert_eq!(
            a_len, b_len,
            "Length mismatch (pathological = {pathological})"
        );
    }

    #[test]
    fn test_small() {
        zip_eq(0, [], []);
        zip_eq(0, b"a", [encode1(b'a')]);
        zip_eq(0, [0, 1], [encode2(0, 1)]);
    }

    #[test]
    fn test_even() { zip_eq(0, b"even", [encode2(b'e', b'v'), encode2(b'e', b'n')]); }

    #[test]
    fn test_odd() { zip_eq(0, b"odd", [encode2(b'o', b'd'), encode1(b'd')]); }

    #[test]
    fn test_lengths() {
        for l in 0..=128 {
            for i in 0..=256 {
                let arr = vec![255_u8; i];
                zip_eq(
                    l,
                    &arr,
                    arr.chunks(2).map(|c| match *c {
                        [a, b] => encode2(a, b),
                        [a] => encode1(a),
                        _ => unreachable!(),
                    }),
                );
            }
        }
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

        zip_eq(0, odd, odd_enc);
        zip_eq(0, even, even_enc);
    }
}
