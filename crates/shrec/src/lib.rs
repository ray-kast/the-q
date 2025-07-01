//! Toolkit for constructing syntactic and static analyses

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
#![allow(
    missing_docs,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    reason = "TODO: document everything"
)]

pub mod autom;
pub mod bijection;
pub mod closure_builder;
pub mod dfa;
pub mod dot;
pub mod egraph;
pub mod free;
pub mod lex_cmp;
pub mod memoize;
pub mod nfa;
pub mod partition_map;
pub mod range_map;
pub mod range_set;
pub mod re;
pub mod term;
pub mod union_find;

#[cfg(any(test, feature = "proptest"))]
pub mod prop {
    use std::ops::RangeInclusive;

    use proptest::prelude::*;

    const RANGES: [RangeInclusive<char>; 26] = [
        '!'..='~',
        '\u{a1}'..='\u{ac}',
        '\u{ae}'..='\u{b7}',
        '\u{b9}'..='\u{1bf}',
        '\u{1c1}'..='\u{2af}',
        '\u{370}'..='\u{373}',
        '\u{375}'..='\u{377}',
        '\u{37b}'..='\u{37f}',
        '\u{386}'..='\u{38a}',
        '\u{38c}'..='\u{38c}',
        '\u{38e}'..='\u{3a1}',
        '\u{3a3}'..='\u{3e1}',
        '\u{400}'..='\u{482}',
        '\u{48a}'..='\u{52f}',
        '\u{531}'..='\u{556}',
        '\u{561}'..='\u{587}',
        '\u{1e00}'..='\u{1f15}',
        '\u{1f18}'..='\u{1f1d}',
        '\u{1f20}'..='\u{1f45}',
        '\u{1f48}'..='\u{1f4d}',
        '\u{1f50}'..='\u{1f57}',
        '\u{1f59}'..='\u{1f59}',
        '\u{1f5b}'..='\u{1f5b}',
        '\u{1f5d}'..='\u{1f5d}',
        '\u{1f5f}'..='\u{1f7d}',
        '\u{1f80}'..='\u{1faf}',
    ];

    pub fn symbol() -> impl Strategy<Value = char> + Clone {
        prop::char::ranges(RANGES.as_slice().into())
    }

    const SAFER_RANGES: [RangeInclusive<char>; 28] = [
        '0'..='9',
        'A'..='Z',
        'a'..='z',
        '\u{a1}'..='\u{ac}',
        '\u{ae}'..='\u{b7}',
        '\u{b9}'..='\u{1bf}',
        '\u{1c1}'..='\u{2af}',
        '\u{370}'..='\u{373}',
        '\u{375}'..='\u{377}',
        '\u{37b}'..='\u{37f}',
        '\u{386}'..='\u{38a}',
        '\u{38c}'..='\u{38c}',
        '\u{38e}'..='\u{3a1}',
        '\u{3a3}'..='\u{3e1}',
        '\u{400}'..='\u{482}',
        '\u{48a}'..='\u{52f}',
        '\u{531}'..='\u{556}',
        '\u{561}'..='\u{587}',
        '\u{1e00}'..='\u{1f15}',
        '\u{1f18}'..='\u{1f1d}',
        '\u{1f20}'..='\u{1f45}',
        '\u{1f48}'..='\u{1f4d}',
        '\u{1f50}'..='\u{1f57}',
        '\u{1f59}'..='\u{1f59}',
        '\u{1f5b}'..='\u{1f5b}',
        '\u{1f5d}'..='\u{1f5d}',
        '\u{1f5f}'..='\u{1f7d}',
        '\u{1f80}'..='\u{1faf}',
    ];

    /// Excludes non-alphanumeric ASCII symbols likely to be used in parsed
    /// syntax
    pub fn symbol_safe() -> impl Strategy<Value = char> + Clone {
        prop::char::ranges(SAFER_RANGES.as_slice().into())
    }
}
