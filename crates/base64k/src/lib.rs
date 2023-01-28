//! A utility crate for converting 16-bit binary data to Unicode code points.

#![deny(
    clippy::disallowed_methods,
    clippy::suspicious,
    clippy::style,
    clippy::clone_on_ref_ptr,
    missing_debug_implementations,
    missing_copy_implementations
)]
#![warn(clippy::pedantic, missing_docs)]
#![allow(clippy::module_name_repetitions)]

mod dec;
mod enc;

pub(self) const TRAIL_MASK: u32 = 0x0001_0000;

pub use dec::Decoder;
pub use enc::Encoder;

#[cfg(test)]
pub(self) mod test {
    use std::io::prelude::*;

    use proptest::prelude::*;

    use super::{Decoder, Encoder};

    pub fn encode1(a: u8) -> char {
        let combined = u32::from(a) | 0x0001_0000;
        let safe = (combined + 0x800) ^ 0xd800;

        char::from_u32(safe).unwrap()
    }

    pub fn encode2(a: u8, b: u8) -> char {
        let a = u16::from(a);
        let b = u16::from(b);
        let combined: u16 = a | (b << 8);
        let combined = u32::from(combined);
        let safe = (combined + 0x800) ^ 0xd800;

        char::from_u32(safe).unwrap()
    }

    fn assert_roundtrip(inp: &[u8]) {
        let mut enc = Encoder::<String>::default();
        enc.write_all(inp).unwrap();
        let s = enc.finish();
        let mut dec = Decoder::new(s.chars());
        let mut out = vec![];
        dec.read_to_end(&mut out).unwrap();
        assert_eq!(inp, out);
    }

    proptest::proptest! {
        #[test]
        fn test_roundtrip_small(v in prop::collection::vec(0_u8..255, 0..256)) {
            assert_roundtrip(&v);
        }

        #[test]
        fn test_roundtrip_kib(v in prop::collection::vec(0_u8..255, 0..(10 * 1024))) {
            assert_roundtrip(&v);
        }
    }

    proptest::proptest! {
        #![proptest_config(ProptestConfig {
            cases: 5,
            ..ProptestConfig::default()
        })]

        #[cfg(not(debug_assertions))]
        #[test]
        fn test_roundtrip_mib(v in prop::collection::vec(0_u8..255, (4 * 1024)..(10 * 1024 * 1024))) {
            assert_roundtrip(&v);
        }
    }
}
