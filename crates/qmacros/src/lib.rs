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
mod deserialize;

pub(crate) mod prelude {
    pub use proc_macro2::{Span, TokenStream};
    pub use quote::{quote_spanned, ToTokens};
    pub use syn::{parse_quote, spanned::Spanned};

    pub trait SpanExt {
        fn error(self, msg: impl std::fmt::Display) -> syn::Error;
    }

    impl<T: Into<Span>> SpanExt for T {
        #[inline]
        fn error(self, msg: impl std::fmt::Display) -> syn::Error {
            syn::Error::new(self.into(), msg)
        }
    }
}

/// Implement [`std::borrow::Borrow`] or [`std::borrow::BorrowMut`] by way of a
/// particular field, decorated with `#[borrow]` or `#[borrow(mut)]`.
#[proc_macro_derive(Borrow, attributes(borrow))]
pub fn borrow(input: TokenStream1) -> TokenStream1 {
    borrow::run(syn::parse_macro_input!(input)).into()
}

/// Lift an impl block for a builder struct into a helper trait
#[proc_macro_attribute]
pub fn builder(arg_stream: TokenStream1, body: TokenStream1) -> TokenStream1 {
    let mut args = builder::Args::default();
    let parser = args.parser();
    syn::parse_macro_input!(arg_stream with parser);
    // TODO: get a real span
    builder::run(
        args,
        proc_macro2::Span::call_site(),
        syn::parse_macro_input!(body),
    )
    .into()
}

/// Implement `paracord::interaction::handler::DeserializeCommand` for a type
#[expect(clippy::let_and_return)]
#[proc_macro_derive(DeserializeCommand, attributes(deserialize, arg, target))]
pub fn deserialize_command(input: TokenStream1) -> TokenStream1 {
    let ret = deserialize::run(syn::parse_macro_input!(input), deserialize::Command).into();
    // eprintln!("{ret}");
    ret
}

/// Implement `paracord::interaction::handler::DeserializeRpc` for a type
#[expect(clippy::let_and_return)]
#[proc_macro_derive(DeserializeRpc, attributes(deserialize))]
pub fn deserialize_rpc(input: TokenStream1) -> TokenStream1 {
    let ret = deserialize::run(syn::parse_macro_input!(input), deserialize::Rpc).into();
    // eprintln!("{ret}");
    ret
}
