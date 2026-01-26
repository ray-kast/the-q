use syn::{
    meta::ParseNestedMeta, parse::Parse, punctuated::Punctuated, Block, FnArg, Lifetime,
    LifetimeParam, Pat, PatType, Path, Token, Type,
};

use crate::{deserialize::Kind, prelude::*};

pub(crate) struct Rpc;

#[derive(Default)]
pub(crate) struct RpcAttr {
    kind: Option<RpcKind>,
    schema: Option<Type>,
}

impl RpcAttr {
    fn key(&self) -> Type {
        let schema = self.schema.as_ref().unwrap();
        match self.kind.unwrap() {
            RpcKind::Component => parse_quote! {
                <#schema as ::paracord::interaction::rpc::Schema>::ComponentKey
            },
            RpcKind::Modal => parse_quote! {
                <#schema as ::paracord::interaction::rpc::Schema>::ModalKey
            },
        }
    }
}

#[derive(Clone, Copy)]
enum RpcKind {
    Component,
    Modal,
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
        if meta.path.is_ident("component") {
            attr.kind = Some(RpcKind::Component);
        } else if meta.path.is_ident("modal") {
            attr.kind = Some(RpcKind::Modal);
        } else if meta.path.is_ident("schema") {
            attr.schema = Some(Type::parse(meta.value()?)?);
        } else {
            return Ok(false);
        }

        Ok(true)
    }

    fn validate_outer_meta(attr: &Self::Attr) -> syn::Result<()> {
        attr.kind.ok_or_else(|| {
            Span::call_site().error("Missing #[deserialize(component)] or #[deserialize(modal)]")
        })?;

        attr.schema
            .as_ref()
            .ok_or_else(|| Span::call_site().error("Missing #[deserialize(schema = ...)]"))?;

        Ok(())
    }

    fn emit_trait_generics(
        attr: &Self::Attr,
        deser_lt: &Lifetime,
        cx_ty: &Type,
    ) -> Punctuated<syn::GenericArgument, Token![,]> {
        let key_ty = attr.key();
        parse_quote! { #deser_lt, #key_ty, #cx_ty }
    }

    fn emit_register_items<F: FnMut(&str, Punctuated<FnArg, Token![,]>, Pat, Type, Block)>(
        attr: &Self::Attr,
        mut f: F,
    ) {
        let key_ty = attr.key();
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
        let kind = attr.kind.unwrap();

        f(
            "",
            match kind {
                RpcKind::Component => parse_quote! {
                    __visitor: &mut ::paracord::interaction::handler::ComponentVisitor<#lt>
                },
                RpcKind::Modal => parse_quote! {
                    __visitor: &mut ::paracord::interaction::handler::ModalVisitor<#lt>
                },
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
