use std::io;

use super::TRAIL_MASK;

/// Decoder for reading base64k data from a sequence of `char`s
#[derive(Debug, Default)]
pub struct Decoder<I> {
    it: I,
    // TODO: SIMD?
    buf: Option<u8>,
}

impl<I: Iterator<Item = char>> Decoder<I> {
    /// Construct a new decoder from the given sequence of `char`s
    #[inline]
    pub fn new<J: IntoIterator<IntoIter = I>>(it: J) -> Self {
        Self {
            it: it.into_iter(),
            buf: Option::default(),
        }
    }
}

impl<I: Iterator<Item = char>> io::Read for Decoder<I> {
    fn read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let mut nread = 0;
        if let Some(b) = self.buf.take() {
            // SAFETY: buf is asserted to have length >= 1 above
            unsafe {
                *buf.get_unchecked_mut(0) = b;
                buf = buf.get_unchecked_mut(1..);
            }
            nread += 1;
        }

        for chunk in buf.chunks_mut(2) {
            let Some(int) = self.it.next() else { break };
            // Undo the result of enc::ShortArray::prepare
            let int = (u32::from(int) ^ 0xd800) - 0x800;
            #[allow(clippy::cast_possible_truncation)]
            let (lo, hi) = (int as u8, (int >> 8) as u8);
            let has_hi = (int & TRAIL_MASK) != TRAIL_MASK;

            if !has_hi && self.it.next().is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Trailing chars found after padding",
                ));
            }

            match chunk {
                [a, b] => {
                    *a = lo;
                    *b = hi * u8::from(has_hi);
                    nread += 1 + usize::from(has_hi);
                },
                [a] => {
                    *a = lo;
                    self.buf = Some(hi);
                    nread += 1;
                },
                _ if cfg!(debug_assertions) => unreachable!(),
                // SAFETY: chunks does not return empty slices and the maximum
                //         chunk length is the requested 2
                _ => unsafe { std::hint::unreachable_unchecked() },
            }
        }

        Ok(nread)
    }
}

#[cfg(test)]
mod test {
    use std::io::prelude::*;

    use super::Decoder;
    use crate::test::{encode1, encode2};

    fn zip_eq<A: Read, B: IntoIterator<Item = u8>>(mut a: A, b: B)
    where B::IntoIter: ExactSizeIterator {
        let mut buf = vec![];
        a.read_to_end(&mut buf).unwrap();
        let b = b.into_iter();
        assert_eq!(buf.len(), b.len(), "Length mismatch");

        for (i, (a, b)) in buf.into_iter().zip(b).enumerate() {
            assert_eq!(a, b, "Mismatch at index {i}: {a:#02x} vs {b:#02x}");
        }
    }

    #[test]
    fn test_small() {
        zip_eq(Decoder::new([]), []);
        zip_eq(Decoder::new([encode1(b'a')]), b"a".to_owned());
        zip_eq(Decoder::new([encode2(0, 1)]), [0, 1]);
    }

    #[test]
    fn test_even() {
        zip_eq(
            Decoder::new([encode2(b'e', b'v'), encode2(b'e', b'n')]),
            b"even".to_owned(),
        );
    }

    #[test]
    fn test_odd() {
        zip_eq(
            Decoder::new([encode2(b'o', b'd'), encode1(b'd')]),
            b"odd".to_owned(),
        );
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

        zip_eq(Decoder::new(odd_enc), odd.to_owned());
        zip_eq(Decoder::new(even_enc), even.to_owned());
    }
}
