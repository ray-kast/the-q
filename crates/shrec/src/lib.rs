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
#![allow(missing_docs)] // TODO

pub mod dfa;
pub mod free;
pub mod nfa;
pub mod re;
pub mod union_find;
