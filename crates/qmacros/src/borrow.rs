use crate::prelude::*;

pub(super) fn run(input: syn::DeriveInput) -> TokenStream {
    let span = input.span();
    match input.data {
        syn::Data::Struct(s) => {
            let mut diag = TokenStream::new();
            let toks: TokenStream = s
                .fields
                .into_iter()
                .enumerate()
                .filter_map(|(i, f)| try_impl(&input.ident, &input.generics, f, i, &mut diag))
                .collect();

            [diag, toks].into_iter().collect()
        },
        _ => span
            .error("Cannot derive Borrow on a non-struct type")
            .emit_as_item_tokens(),
    }
}

struct FieldOpts {
    span: Span,
    mutable: Option<Span>,
}

fn try_impl(
    ty: &syn::Ident,
    generics: &syn::Generics,
    field: syn::Field,
    field_id: usize,
    diag: &mut TokenStream,
) -> Option<TokenStream> {
    let mut opts = None;

    for attr in field.attrs {
        if !attr.path.is_ident("borrow") {
            continue;
        }

        let span = attr.span();

        if opts.is_some() {
            diag.extend(
                span.error("Duplicate #[borrow] attribute")
                    .emit_as_item_tokens(),
            );
            return None;
        }

        let meta = match attr.parse_meta() {
            Ok(m) => m,
            Err(e) => {
                diag.extend(e.into_compile_error());
                continue;
            },
        };

        let mut mutable = None;

        match meta {
            syn::Meta::Path(_) => (),
            syn::Meta::List(l) => {
                for nested in l.nested {
                    match nested {
                        syn::NestedMeta::Meta(syn::Meta::Path(p)) if p.is_ident("mut") => {
                            mutable = Some(p.span());
                        },
                        _ => (),
                    }
                }
            },
            syn::Meta::NameValue(nv) => diag.extend(
                nv.span()
                    .error("Invalid #[borrow] attribute")
                    .emit_as_item_tokens(),
            ),
        }

        opts = Some(FieldOpts { span, mutable });
    }

    let FieldOpts { span, mutable } = opts?;

    let args = BorrowArgs {
        ty,
        generics,
        out_ty: &field.ty,
        field: field.ident.as_ref(),
        field_id,
    };

    let mut toks = borrow(args, span, None);

    if let Some(mutbl) = mutable {
        toks.extend(borrow(args, span, Some(mutbl)));
    }

    Some(toks)
}

#[derive(Clone, Copy)]
struct BorrowArgs<'a> {
    ty: &'a syn::Ident,
    generics: &'a syn::Generics,
    out_ty: &'a syn::Type,
    field: Option<&'a syn::Ident>,
    field_id: usize,
}

fn borrow(args: BorrowArgs, span: Span, mutable: Option<Span>) -> TokenStream {
    let BorrowArgs {
        ty,
        generics,
        out_ty,
        field,
        field_id,
    } = args;
    let mutbl: Option<syn::token::Mut> = mutable.map(|m| syn::parse_quote_spanned! { m => mut });
    let mutable = mutable.is_some();
    let trait_name = syn::Ident::new(if mutable { "BorrowMut" } else { "Borrow" }, span);
    let fn_name = syn::Ident::new(if mutable { "borrow_mut" } else { "borrow" }, span);

    let field = field.map_or_else(
        || quote_spanned! { span => #field_id },
        syn::Ident::to_token_stream,
    );

    let (impl_gen, ty_gen, where_toks) = generics.split_for_impl();
    quote_spanned! { span =>
        impl #impl_gen ::std::borrow::#trait_name<#out_ty> for #ty #ty_gen #where_toks {
            fn #fn_name(&#mutbl self) -> &#mutbl #out_ty { &#mutbl self.#field }
        }
    }
}
