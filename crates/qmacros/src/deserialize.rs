use syn::{
    meta::ParseNestedMeta, parse::Parse, punctuated::Punctuated, Block, FnArg, GenericArgument,
    Ident, ImplItem, ImplItemFn, Lifetime, LifetimeParam, Pat, PatType, Path, Signature, Token,
    Type, Visibility,
};

use crate::prelude::*;

mod command;
mod rpc;

pub(super) use command::Command;
pub(super) use rpc::Rpc;

fn get_one<I: IntoIterator>(it: I, err: impl FnOnce(I::Item)) -> Option<I::Item> {
    let mut it = it.into_iter();
    let i = it.next();
    it.next().map(err);
    i
}

pub(super) trait Kind {
    type Attr: Default;

    fn default_lt() -> LifetimeParam;

    fn trait_path() -> Path;

    fn parse_outer_nested_meta(
        attr: &mut Self::Attr,
        meta: ParseNestedMeta<'_>,
    ) -> syn::Result<bool>;

    fn validate_outer_meta(attr: &Self::Attr) -> syn::Result<()>;

    fn emit_trait_generics(
        attr: &Self::Attr,
        deser_lt: &Lifetime,
        cx_ty: &Type,
    ) -> Punctuated<GenericArgument, Token![,]>;

    fn emit_register_items<F: FnMut(&str, Punctuated<FnArg, Token![,]>, Pat, Type, Block)>(
        attr: &Self::Attr,
        f: F,
    );

    fn emit_deserialize_items<F: FnMut(&str, PatType, Type, Block)>(
        attr: &Self::Attr,
        lt: &Lifetime,
        f: F,
    );

    fn emit_extra<F: FnMut(ImplItem)>(attr: &Self::Attr, f: F);
}

#[expect(clippy::needless_pass_by_value)]
pub(super) fn run<K: Kind>(input: syn::DeriveInput, _kind: K) -> TokenStream {
    let mut diag = TokenStream::new();

    let mut cx_ty = None;
    let mut kind_attr = K::Attr::default();

    if let Some(attr) = get_one(
        input
            .attrs
            .iter()
            .filter(|a| a.path().is_ident("deserialize")),
        |a| diag.extend(a.span().error("").into_compile_error()),
    ) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("cx") {
                cx_ty = Some((meta.path.span(), Type::parse(meta.value()?)?));
            } else {
                let invalid = meta.error("Invalid argument");
                if !K::parse_outer_nested_meta(&mut kind_attr, meta)? {
                    return Err(invalid);
                }
            }

            Ok(())
        })
        .unwrap_or_else(|e| diag.extend(e.into_compile_error()));
    }

    match cx_ty
        .as_ref()
        .ok_or_else(|| Span::call_site().error("Missing #[deserialize(cx = ...)]"))
        .and_then(|_| K::validate_outer_meta(&kind_attr))
    {
        Ok(()) => (),
        Err(e) => return [diag, e.into_compile_error()].into_iter().collect(),
    }

    let (cx_span, cx_ty) = cx_ty.unwrap();

    let mut impl_generics = input.generics.clone();

    if impl_generics.lifetimes().next().is_none() {
        impl_generics.params.insert(0, K::default_lt().into());
    }

    let deser_lt = impl_generics.lifetimes().next().unwrap();

    let (impl_generics, ..) = impl_generics.split_for_impl();
    let (_, ty_generics, where_clause) = input.generics.split_for_impl();

    let mut items = vec![];

    K::emit_extra(&kind_attr, |item| items.push(item));

    K::emit_register_items(&kind_attr, |ident, mut inputs, cx_pat, ret, block| {
        inputs.push(parse_quote! { #cx_pat: &#cx_ty });
        items.push(ImplItem::from(ImplItemFn {
            attrs: vec![],
            vis: Visibility::Inherited,
            defaultness: None,
            sig: Signature {
                constness: None,
                asyncness: None,
                unsafety: None,
                abi: None,
                fn_token: parse_quote! { fn },
                ident: Ident::new(
                    &if ident.is_empty() {
                        "register".to_owned()
                    } else {
                        format!("register_{ident}")
                    },
                    Span::call_site(),
                ),
                generics: parse_quote! {},
                paren_token: syn::token::Paren::default(),
                inputs,
                variadic: None,
                output: parse_quote! { -> #ret },
            },
            block,
        }));
    });

    K::emit_deserialize_items(
        &kind_attr,
        &deser_lt.lifetime,
        |ident, visitor_arg, ret_ty, block| {
            items.push(ImplItem::from(ImplItemFn {
            attrs: vec![],
            vis: Visibility::Inherited,
            defaultness: None,
            sig: Signature {
                constness: None,
                asyncness: None,
                unsafety: None,
                abi: None,
                fn_token: parse_quote! { fn },
                ident: Ident::new(
                    &if ident.is_empty() {
                        "deserialize".to_owned()
                    } else {
                        format!("deserialize_{ident}")
                    },
                    Span::call_site(),
                ),
                generics: parse_quote! {},
                paren_token: syn::token::Paren::default(),
                inputs: parse_quote! { #visitor_arg },
                variadic: None,
                output: parse_quote! { -> Result<#ret_ty, ::paracord::interaction::visitor::Error> },
            },
            block,
        }));
        },
    );

    let ty = &input.ident;
    let trait_path = K::trait_path();
    let trait_generics = K::emit_trait_generics(&kind_attr, &deser_lt.lifetime, &cx_ty);

    quote_spanned! { input.span() =>
        #diag
        impl #impl_generics #trait_path<#trait_generics> for #ty #ty_generics
        #where_clause {
            #(#items)*
        }
    }
}
