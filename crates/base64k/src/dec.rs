use std::io;

use wide::u32x8;

use super::TRAIL_MASK;
use crate::arr::ShortArray;

/// Decoder for reading base64k data from a sequence of `char`s
#[derive(Debug, Default)]
pub struct Decoder<I> {
    it: I,
    // TODO: I bet there's a deranged way to use ShortArray for this
    buf: Vec<u8>,
}

impl<I: Iterator<Item = char>> Decoder<I> {
    /// Construct a new decoder from the given sequence of `char`s
    #[inline]
    pub fn new<J: IntoIterator<IntoIter = I>>(it: J) -> Self {
        Self {
            it: it.into_iter(),
            buf: Vec::default(),
        }
    }
}

#[inline]
unsafe fn split_mut<T>(arr: &mut [T], i: usize) -> (&mut [T], &mut [T]) {
    if cfg!(debug_assertions) {
        arr.split_at_mut(i)
    } else {
        let len = arr.len();
        let ptr = arr.as_mut_ptr();
        (
            std::slice::from_raw_parts_mut(ptr, i),
            std::slice::from_raw_parts_mut(ptr.add(i), len - i),
        )
    }
}

fn split_first_chunk_mut<T, const N: usize>(
    arr: &mut [T],
) -> Result<(&mut [T; N], &mut [T]), &mut [T]> {
    if arr.len() < N {
        Err(arr)
    } else {
        // SAFETY: We manually verified the bounds of the split.
        let (first, tail) = unsafe { split_mut(arr, N) };

        // SAFETY: We explicitly check for the correct number of elements,
        //   do not let the reference outlive the slice,
        //   and enforce exclusive mutability of the chunk by the split.
        Ok((unsafe { &mut *(first.as_mut_ptr().cast::<[T; N]>()) }, tail))
    }
}

impl<I: Iterator<Item = char>> io::Read for Decoder<I> {
    fn read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
        let mut nread = 0;

        if !self.buf.is_empty() {
            let len = buf.len().min(self.buf.len());

            unsafe {
                // SAFETY: len is bounds-checked against buf.len()
                let (dest, rem) = split_mut(buf, len);
                buf = rem;

                // NOTE: I'm aware <slice>.get(..i).as_ptr() is equivalent to
                //       <slice>.as_ptr(), but I'm including it for the sake of
                //       superstition and banking on the compiler to optimize
                //       it out.

                // SAFETY: This is the same operation performed by
                //         copy_from_slice after performing a length equality
                //         check.  The slice lengths here are trivially equal.
                std::ptr::copy_nonoverlapping(
                    // SAFETY: len is bounds-checked against both buf and self.buf above
                    self.buf.get_unchecked(..len).as_ptr(),
                    dest.as_mut_ptr(),
                    len,
                );
            }

            nread += len;
            self.buf.drain(..len);
        }

        let mut chunk = ShortArray::default();
        let mut chunk_len;

        'hot: loop {
            if buf.is_empty() {
                return Ok(nread);
            }

            chunk_len = 0;
            while chunk_len < ShortArray::WIDTH {
                let Some(chr) = self.it.next() else {
                    break 'hot;
                };
                // SAFETY: chunk_len is bounds-checked by the loop condition
                unsafe { *chunk.0.get_unchecked_mut(chunk_len) = chr.into() };
                chunk_len += 1;
            }

            let dec = chunk.decode();
            if dec & u32x8::splat(TRAIL_MASK) != u32x8::ZERO {
                break 'hot;
            }

            let dest;
            match split_first_chunk_mut::<_, { ShortArray::BYTE_WIDTH }>(buf) {
                Ok((d, b)) => {
                    dest = d;
                    buf = b;
                },
                Err(b) => {
                    buf = b;
                    break 'hot;
                },
            }

            let dec = dec.to_array();
            debug_assert!(dest.len() == 2 * ShortArray::WIDTH);
            // SAFETY: The resulting array length is asserted to be equal to
            //         dest.len()
            let dest: &mut [[u8; 2]; ShortArray::WIDTH] = unsafe { &mut *dest.as_mut_ptr().cast() };
            for i in 0..ShortArray::WIDTH {
                // SAFETY: dec.len() == ShortArray::WIDTH
                let dw = *unsafe { dec.get_unchecked(i) };
                // SAFETY: dest.len() trivially equals ShortArray::WIDTH
                let bytes = unsafe { dest.get_unchecked_mut(i) };
                #[allow(clippy::cast_possible_truncation)]
                {
                    *bytes = (dw as u16).to_le_bytes();
                }
            }

            nread += ShortArray::BYTE_WIDTH;
        }

        // Ensure that we have 0 < n < WIDTH chars left
        debug_assert!(
            !buf.is_empty()
                && (buf.len() < ShortArray::BYTE_WIDTH
                    || chunk_len < ShortArray::WIDTH
                    || chunk.decode() & u32x8::splat(TRAIL_MASK) != u32x8::ZERO)
        );

        let dws = chunk.decode().to_array();
        let mut chunks = buf.chunks_mut(2);
        for dw in dws[..chunk_len].iter().copied() {
            #[allow(clippy::cast_possible_truncation)]
            let [lo, hi] = (dw as u16).to_le_bytes();
            let has_hi = (dw & TRAIL_MASK) != TRAIL_MASK;

            if !has_hi && self.it.next().is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Trailing chars found after padding",
                ));
            }

            match chunks.next().unwrap_or(&mut []) {
                [a, b] => {
                    *a = lo;
                    *b = hi;
                    nread += 1 + usize::from(has_hi);
                },
                [a] => {
                    *a = lo;
                    self.buf.extend(has_hi.then_some(hi));
                    nread += 1;
                },
                [] => {
                    self.buf.push(lo);
                    self.buf.extend(has_hi.then_some(hi));
                },
                _ if cfg!(debug_assertions) => unreachable!(),
                // SAFETY: chunks does not return empty slices and the maximum
                //         chunk length is the requested 2
                _ => unsafe { std::hint::unreachable_unchecked() },
            }
        }

        debug_assert!(self.buf.len() < ShortArray::BYTE_WIDTH);

        Ok(nread)
    }
}

#[cfg(test)]
mod test {
    use std::io::prelude::*;

    use super::Decoder;
    use crate::test::{encode1, encode2};

    #[allow(clippy::read_zero_byte_vec)] // for pathological
    fn zip_eq<A: Read, B: IntoIterator<Item = u8>>(pathological: usize, mut a: A, b: B)
    where B::IntoIter: ExactSizeIterator {
        let mut buf = vec![];
        if pathological > 0 {
            let mut chunk = vec![];
            loop {
                chunk.resize(pathological, 0);
                match a.read(&mut chunk).unwrap() {
                    0 => break,
                    n => {
                        chunk.truncate(n);
                        buf.append(&mut chunk);
                    },
                }
            }
        } else {
            a.read_to_end(&mut buf).unwrap();
        }
        let b = b.into_iter();
        let buf_len = buf.len();
        let b_len = b.len();

        for (i, (a, b)) in buf.into_iter().zip(b).enumerate() {
            assert_eq!(a, b, "Mismatch at index {i}: {a:#02x} vs {b:#02x}");
        }

        assert_eq!(
            buf_len, b_len,
            "Length mismatch (pathological = {pathological})"
        );
    }

    #[test]
    fn test_small() {
        zip_eq(0, Decoder::new([]), []);
        zip_eq(0, Decoder::new([encode1(b'a')]), b"a".to_owned());
        zip_eq(0, Decoder::new([encode2(0, 1)]), [0, 1]);
    }

    #[test]
    fn test_even() {
        zip_eq(
            0,
            Decoder::new([encode2(b'e', b'v'), encode2(b'e', b'n')]),
            b"even".to_owned(),
        );
    }

    #[test]
    fn test_odd() {
        zip_eq(
            0,
            Decoder::new([encode2(b'o', b'd'), encode1(b'd')]),
            b"odd".to_owned(),
        );
    }

    #[test]
    fn test_lengths() {
        for l in 0..=128 {
            for i in 0..=256 {
                let arr = vec![255_u8; i];
                zip_eq(
                    l,
                    Decoder::new(arr.clone().chunks(2).map(|c| match *c {
                        [a, b] => encode2(a, b),
                        [a] => encode1(a),
                        _ => unreachable!(),
                    })),
                    arr,
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

        zip_eq(0, Decoder::new(odd_enc), odd.to_owned());
        zip_eq(0, Decoder::new(even_enc), even.to_owned());
    }
}
