//! Procedural macros for the `qcore` crate

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

use proc_macro::TokenStream as TokenStream1;

mod borrow;
mod builder;

pub(crate) mod prelude {
    pub use proc_macro2::{Span, TokenStream};
    pub use proc_macro2_diagnostics::SpanDiagnosticExt;
    pub use quote::{quote_spanned, ToTokens, TokenStreamExt};
    pub use syn::spanned::Spanned;
}

/// Implement [`std::borrow::Borrow`] or [`std::borrow::BorrowMut`] by way of a
/// particular field, decorated with `#[borrow]` or `#[borrow(mut)]`.
#[proc_macro_derive(Borrow, attributes(borrow))]
pub fn borrow(input: TokenStream1) -> TokenStream1 {
    borrow::run(syn::parse_macro_input!(input)).into()
}

/// Lift an impl block for a builder struct into a helper trait
#[proc_macro_attribute]
pub fn builder(args: TokenStream1, body: TokenStream1) -> TokenStream1 {
    builder::run(
        &syn::parse_macro_input!(args),
        syn::parse_macro_input!(body),
    )
    .into()
}
