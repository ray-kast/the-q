use syn::{
    meta::ParseNestedMeta, punctuated::Punctuated, Block, FnArg, ImplItem, Lifetime, LifetimeParam,
    Pat, PatType, Path, Token, Type,
};

use crate::{deserialize::Kind, prelude::*};

pub(crate) struct Command;

#[derive(Default)]
pub(crate) struct CommandAttr {}

impl Kind for Command {
    type Attr = CommandAttr;

    fn default_lt() -> LifetimeParam {
        parse_quote! { '__deserialize_command }
    }

    fn trait_path() -> Path {
        parse_quote! { ::paracord::interaction::handler::DeserializeCommand }
    }

    fn parse_outer_nested_meta(
        _attr: &mut Self::Attr,
        _meta: ParseNestedMeta<'_>,
    ) -> syn::Result<bool> {
        Ok(false)
    }

    fn validate_outer_meta(_attr: &Self::Attr) -> syn::Result<()> { Ok(()) }

    fn emit_trait_generics(
        _attr: &Self::Attr,
        deser_lt: &Lifetime,
        cx_ty: &Type,
    ) -> Punctuated<syn::GenericArgument, Token![,]> {
        parse_quote! { #deser_lt, #cx_ty }
    }

    fn emit_register_items<F: FnMut(&str, Punctuated<FnArg, Token![,]>, Pat, Type, Block)>(
        _attr: &Self::Attr,
        mut f: F,
    ) {
        f(
            "global",
            Punctuated::new(),
            parse_quote! { __deserialize_cx },
            parse_quote! { ::paracord::interaction::command::CommandInfo },
            parse_quote! {{
                todo!()
            }},
        );

        f(
            "guild",
            Some::<FnArg>(parse_quote! { __guild_id: GuildId })
                .into_iter()
                .collect(),
            parse_quote! { __deserialize_cx },
            parse_quote! { Option<::paracord::interaction::command::CommandInfo> },
            parse_quote! {{
                todo!()
            }},
        );
    }

    fn emit_deserialize_items<F: FnMut(&str, PatType, Type, Block)>(
        _attr: &Self::Attr,
        lt: &Lifetime,
        mut f: F,
    ) {
        f(
            "completion",
            parse_quote! {
                __visitor: &mut ::paracord::interaction::handler::CommandVisitor<#lt>
            },
            parse_quote! { Self::Completion },
            parse_quote! {{
                todo!()
            }},
        );

        f(
            "",
            parse_quote! {
                __visitor: &mut ::paracord::interaction::handler::CommandVisitor<#lt>
            },
            parse_quote! { Self },
            parse_quote! {{
                todo!()
            }},
        );
    }

    fn emit_extra<F: FnMut(syn::ImplItem)>(_attr: &Self::Attr, mut f: F) {
        f(ImplItem::Type(parse_quote! {
            type Completion = ::paracord::interaction::handler::NoCompletion;
        }));
    }
}
