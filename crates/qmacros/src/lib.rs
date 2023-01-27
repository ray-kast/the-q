use proc_macro::TokenStream as TokenStream1;

mod builder;

pub(crate) mod prelude {
    pub use proc_macro2::{Span, TokenStream};
    pub use proc_macro2_diagnostics::SpanDiagnosticExt;
    pub use quote::{quote_spanned, ToTokens, TokenStreamExt};
    pub use syn::spanned::Spanned;
}

#[proc_macro_attribute]
pub fn builder(args: TokenStream1, body: TokenStream1) -> TokenStream1 {
    builder::run(syn::parse_macro_input!(args), syn::parse_macro_input!(body)).into()
}
