use syn::fold::Fold;

use crate::prelude::*;

#[derive(darling::FromMeta)]
struct Args {
    // TODO: why is this a literal
    trait_name: syn::Ident,
}

pub(super) fn run(args: syn::AttributeArgs, mut body: syn::ItemImpl) -> TokenStream {
    let Args { trait_name } = match darling::FromMeta::from_list(&args) {
        Ok(a) => a,
        Err(e) => return e.write_errors(),
    };

    // TODO: drain_filter Please
    let mut remove: Vec<_> = body
        .items
        .iter()
        .enumerate()
        .filter_map(|(i, t)| is_builder_method(t).then_some(i))
        .collect();
    remove.sort_unstable();

    let mut methods = vec![];
    for i in remove.into_iter().rev() {
        let m = body.items.remove(i);
        methods.push(make_builder_method(m, body.self_ty.span()));
    }

    // TODO: come up with a cleaner way to select methods
    let methods = methods.into_iter().fold(TokenStream::new(), |mut t, m| {
        m.to_tokens(&mut t);
        t
    });
    let ty_name = &body.self_ty;
    quote_spanned! { body.brace_token.span =>
        #body
        pub trait #trait_name: ::std::borrow::BorrowMut<#ty_name> + ::std::marker::Sized {
            #methods
        }

        // TODO: generics
        impl<T: ::std::borrow::BorrowMut<#ty_name> + ::std::marker::Sized> #trait_name for T {}
    }
}

fn is_builder_method(m: &syn::ImplItem) -> bool {
    let syn::ImplItem::Method(m) = m else { return false };

    match m.sig.output {
        syn::ReturnType::Default => (),
        syn::ReturnType::Type(_, ref t) => match **t {
            syn::Type::Tuple(ref t) if t.elems.is_empty() => (),
            _ => return false,
        },
    }

    let Some(syn::FnArg::Receiver(r)) = m.sig.inputs.first() else { return false };

    if r.reference.is_none() || r.mutability.is_none() {
        return false;
    }

    if !matches!(m.vis, syn::Visibility::Public(_)) {
        m.span()
            .warning("Non-public method looks like a builder method");
        return false;
    }

    true
}

fn make_builder_method(m: syn::ImplItem, ty_span: Span) -> syn::ImplItemMethod {
    let syn::ImplItem::Method(mut m) = m else { unreachable!() };

    m.vis = syn::Visibility::Inherited;

    let Some(syn::FnArg::Receiver(r)) = m.sig.inputs.first_mut() else { unreachable!() };
    r.reference = None;

    m.sig.output = syn::parse_quote_spanned! { ty_span => -> Self };

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
                    this: this.clone(),
                    repl,
                }
                .fold_block(m.block);
                syn::parse_quote_spanned! { block.span() => { #block; }; }
            },
            syn::Stmt::Expr(syn::Expr::Path(this)),
        ],
    };

    m
}

struct Folder {
    this: syn::ExprPath,
    repl: syn::ExprPath,
}

impl syn::fold::Fold for Folder {
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
            x.span().error("Unexpected return value");
            return e;
        }

        e.expr = Some(Box::new(syn::Expr::Path(self.this.clone())));
        e
    }
}
