use proc_macro::TokenStream as TokenStream1;

mod borrow;
mod builder;

pub(crate) mod prelude {
    pub use proc_macro2::{Span, TokenStream};
    pub use proc_macro2_diagnostics::SpanDiagnosticExt;
    pub use quote::{quote_spanned, ToTokens, TokenStreamExt};
    pub use syn::spanned::Spanned;
}

#[proc_macro_derive(Borrow, attributes(borrow))]
pub fn borrow(input: TokenStream1) -> TokenStream1 {
    borrow::run(syn::parse_macro_input!(input)).into()
}

#[proc_macro_attribute]
pub fn builder(args: TokenStream1, body: TokenStream1) -> TokenStream1 {
    builder::run(syn::parse_macro_input!(args), syn::parse_macro_input!(body)).into()
}
