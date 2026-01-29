use syn::{
    meta::ParseNestedMeta, parse::Parse, punctuated::Punctuated, Block, FnArg, Lifetime,
    LifetimeParam, Pat, PatType, Path, Token, Type,
};

use crate::{deserialize::Kind, prelude::*};

pub(crate) struct Rpc;

#[derive(Default)]
pub(crate) struct RpcAttr {
    key: Option<Type>,
}

impl Kind for Rpc {
    type Attr = RpcAttr;

    fn default_lt() -> LifetimeParam {
        parse_quote! { '__deserialize_rpc }
    }

    fn trait_path() -> Path {
        parse_quote! { ::paracord::interaction::handler::DeserializeRpc }
    }

    fn parse_outer_nested_meta(
        attr: &mut Self::Attr,
        meta: ParseNestedMeta<'_>,
    ) -> syn::Result<bool> {
        if meta.path.is_ident("key") {
            attr.key = Some(Type::parse(meta.value()?)?);
        } else {
            return Ok(false);
        }

        Ok(true)
    }

    fn validate_outer_meta(attr: &Self::Attr) -> syn::Result<()> {
        attr.key
            .as_ref()
            .ok_or_else(|| Span::call_site().error("Missing #[deserialize(key = ...)]"))?;

        Ok(())
    }

    fn emit_trait_generics(
        attr: &Self::Attr,
        deser_lt: &Lifetime,
        cx_ty: &Type,
    ) -> Punctuated<syn::GenericArgument, Token![,]> {
        let key_ty = attr.key.as_ref().unwrap();
        parse_quote! { #deser_lt, #key_ty, #cx_ty }
    }

    fn emit_register_items<F: FnMut(&str, Punctuated<FnArg, Token![,]>, Pat, Type, Block)>(
        attr: &Self::Attr,
        mut f: F,
    ) {
        let key_ty = attr.key.as_ref().unwrap();
        f(
            "keys",
            Punctuated::new(),
            parse_quote! { __deserialize_cx },
            parse_quote! { &[#key_ty] },
            parse_quote! {{
                todo!()
            }},
        );
    }

    fn emit_deserialize_items<F: FnMut(&str, PatType, Type, Block)>(
        attr: &Self::Attr,
        lt: &Lifetime,
        mut f: F,
    ) {
        let key_ty = attr.key.as_ref().unwrap();
        f(
            "",
            parse_quote! {
                __visitor: &mut ::paracord::interaction::visitor::BasicVisitor<
                    #lt,
                    <#key_ty as ::paracord::interaction::rpc::Key>::Interaction,
                >
            },
            parse_quote! { Self },
            parse_quote! {{
                todo!()
            }},
        );
    }

    #[inline]
    fn emit_extra<F: FnMut(syn::ImplItem)>(_attr: &Self::Attr, _f: F) {}
}
