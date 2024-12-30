use std::borrow::Cow;

use syn::fold::Fold;

use crate::prelude::*;

#[derive(Default)]
pub(super) struct Args {
    trait_name: Option<syn::Ident>,
}

// TODO(design): Refactor builder interface to handle errors better

impl Args {
    pub fn parser(&'_ mut self) -> impl syn::parse::Parser<Output = ()> + '_ {
        syn::meta::parser(|meta| {
            if meta.path.is_ident("trait_name") {
                let _eq: syn::token::Eq = meta.input.parse()?;
                let ident: syn::Ident = meta.input.parse()?;

                self.trait_name = Some(ident);
                return Ok(());
            }

            Err(meta.error("Invalid #[builder] attribute"))
        })
    }

    fn finish(self, span: Span) -> Result<(syn::Ident,), syn::Error> {
        let trait_name = self
            .trait_name
            .ok_or_else(|| span.error("Missing trait_name in #[builder] attribute"))?;

        Ok((trait_name,))
    }
}

pub(super) fn run(args: Args, arg_span: Span, mut body: syn::ItemImpl) -> TokenStream {
    let (trait_name,) = match args.finish(arg_span) {
        Ok(a) => a,
        Err(e) => return e.into_compile_error(),
    };

    if let Some(t) = body.trait_ {
        return t
            .1
            .span()
            .error("#[builder] must be used on a non-trait impl block")
            .into_compile_error();
    }

    let mut diag = TokenStream::new();
    let mut vis = None;
    for item in &body.items {
        if let Err((span, err)) = is_builder_method(item, &mut vis) {
            diag.extend(span.error(err).into_compile_error());
        }
    }

    if !diag.is_empty() {
        return diag;
    }

    if !body.generics.params.empty_or_trailing() {
        body.generics
            .params
            .push_punct(syn::token::Comma::default());
    }

    let impl_generic_params = &body.generics.params;
    let impl_generic_where = &body.generics.where_clause;
    let trait_generics: syn::punctuated::Punctuated<_, syn::token::Comma> = {
        impl_generic_params
            .iter()
            .cloned()
            .map(|i| match i {
                syn::GenericParam::Type(syn::TypeParam { ident, .. }) => {
                    syn::GenericArgument::Type(syn::Type::Path(syn::TypePath {
                        qself: None,
                        path: syn::Path {
                            leading_colon: None,
                            segments: [syn::PathSegment {
                                ident,
                                arguments: syn::PathArguments::None,
                            }]
                            .into_iter()
                            .collect(),
                        },
                    }))
                },
                syn::GenericParam::Lifetime(syn::LifetimeParam { lifetime, .. }) => {
                    syn::GenericArgument::Lifetime(lifetime)
                },
                syn::GenericParam::Const(syn::ConstParam { ident, .. }) => {
                    syn::GenericArgument::Const(syn::Expr::Path(syn::ExprPath {
                        attrs: vec![],
                        qself: None,
                        path: syn::Path {
                            leading_colon: None,
                            segments: [syn::PathSegment {
                                ident,
                                arguments: syn::PathArguments::None,
                            }]
                            .into_iter()
                            .collect(),
                        },
                    }))
                },
            })
            .collect()
    };
    let methods = body
        .items
        .into_iter()
        .map(|i| make_builder_method(i, &mut diag))
        .fold(TokenStream::new(), |mut t, m| {
            m.to_tokens(&mut t);
            t
        });
    let attrs = &body.attrs;
    let ty_name = &body.self_ty;
    quote_spanned! { body.brace_token.span =>
        #diag
        #(#attrs)*
        #vis trait #trait_name <#impl_generic_params>:
            ::std::borrow::BorrowMut<#ty_name> + ::std::marker::Sized
            where #impl_generic_where
        {
            #methods
        }

        impl<
            #impl_generic_params
            #[expect(non_camel_case_types, reason = "Macro-generated variable")]
            __Builder_T: ::std::borrow::BorrowMut<#ty_name> + ::std::marker::Sized
        > #trait_name <#trait_generics> for __Builder_T #impl_generic_where {}
    }
}

fn is_builder_method(
    m: &syn::ImplItem,
    vis: &mut Option<syn::Visibility>,
) -> Result<(), (Span, Cow<'static, str>)> {
    let syn::ImplItem::Fn(m) = m else {
        return Err((m.span(), "Builder impl can only contain methods".into()));
    };

    match m.sig.output {
        syn::ReturnType::Default => (),
        syn::ReturnType::Type(_, ref t) => match **t {
            syn::Type::Tuple(ref t) if t.elems.is_empty() => (),
            _ => return Err((m.span(), "Builder method must return ()".into())),
        },
    }

    let Some(syn::FnArg::Receiver(r)) = m.sig.inputs.first() else {
        return Err((
            m.span(),
            "Builder method must have a &mut self param".into(),
        ));
    };

    if r.reference.is_none() || r.mutability.is_none() {
        return Err((
            m.span(),
            "Builder method's self param must be &mut self".into(),
        ));
    }

    match (vis, &m.vis) {
        (v @ None, w) => *v = Some(w.clone()),
        (Some(v), w) if *v == *w => (),
        (Some(v), w) => {
            return Err((
                w.span(),
                format!(
                    "Visibility must be {} to match previous methods",
                    v.to_token_stream()
                )
                .into(),
            ));
        },
    }

    Ok(())
}

fn make_builder_method(m: syn::ImplItem, diag: &mut TokenStream) -> syn::ImplItemFn {
    let syn::ImplItem::Fn(mut m) = m else {
        unreachable!()
    };

    m.vis = syn::Visibility::Inherited;

    let span = m.span();
    let Some(syn::FnArg::Receiver(r)) = m.sig.inputs.first_mut() else {
        unreachable!()
    };
    #[expect(
        clippy::assigning_clones,
        reason = "elem.clone() eliminates a borrow of r"
    )]
    if let syn::Type::Reference(syn::TypeReference { ref elem, .. }) = *r.ty {
        r.ty = elem.clone();
    }
    r.reference = None;

    m.attrs.push(syn::parse_quote! {
        #[expect(
            clippy::return_self_not_must_use,
            reason = "Self may be a mutable reference type"
        )]
    });

    m.sig.output = syn::parse_quote_spanned! { span => -> Self };

    let this: syn::ExprPath = syn::parse_quote_spanned! { r.span() =>
        self
    };

    let repl: syn::ExprPath = syn::parse_quote_spanned! { r.span() =>
        __builder_self
    };

    m.block = syn::Block {
        brace_token: m.block.brace_token,
        stmts: vec![
            syn::parse_quote_spanned! { m.sig.ident.span() =>
                let #repl = ::std::borrow::BorrowMut::borrow_mut(&mut self);
            },
            {
                let block = Folder {
                    diag,
                    this: this.clone(),
                    repl,
                }
                .fold_block(m.block);
                syn::parse_quote_spanned! { block.span() =>
                    #[allow(
                        clippy::unnecessary_operation,
                        reason = "Inside macro-generated code"
                    )]
                    { #block; };
                }
            },
            syn::Stmt::Expr(syn::Expr::Path(this), None),
        ],
    };

    m
}

struct Folder<'a> {
    diag: &'a mut TokenStream,
    this: syn::ExprPath,
    repl: syn::ExprPath,
}

impl Fold for Folder<'_> {
    fn fold_item(&mut self, i: syn::Item) -> syn::Item { i }

    fn fold_expr_path(&mut self, e: syn::ExprPath) -> syn::ExprPath {
        if e.path.is_ident("self") {
            self.repl.clone()
        } else {
            e
        }
    }

    fn fold_expr_return(&mut self, mut e: syn::ExprReturn) -> syn::ExprReturn {
        if let Some(ref x) = e.expr {
            self.diag.extend(
                x.span()
                    .error("Unexpected return value")
                    .into_compile_error(),
            );
            return e;
        }

        e.expr = Some(Box::new(syn::Expr::Path(self.this.clone())));
        e
    }
}
