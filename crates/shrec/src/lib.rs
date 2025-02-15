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
