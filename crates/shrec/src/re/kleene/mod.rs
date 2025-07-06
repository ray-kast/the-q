use std::borrow::Cow;

use nfa_builder::NfaBuilder;

use super::run::{IntoSymbols, Symbol};
use crate::{
    dot,
    free::{Free, Succ},
    nfa::Nfa,
};

mod nfa_builder;
pub mod syntax;

#[derive(Debug, Clone, PartialEq)]
pub enum Regex<L> {
    Alt(Vec<Regex<L>>),
    Cat(Vec<Regex<L>>),
    Star(Box<Regex<L>>),
    Lit(L),
}

impl<L> Regex<L> {
    // TODO: do tests with this, and probably some reachability analyses on the
    //       automata
    pub const BOTTOM: Regex<L> = Regex::Alt(Vec::new());
    pub const EMPTY: Regex<L> = Regex::Cat(Vec::new());

    fn map_impl<M, F: FnMut(L) -> M>(self, f: &mut F) -> Regex<M> {
        match self {
            Self::Alt(v) => Regex::Alt(v.into_iter().map(|r| r.map_impl(f)).collect()),
            Self::Cat(v) => Regex::Cat(v.into_iter().map(|r| r.map_impl(f)).collect()),
            Self::Star(r) => Regex::Star(r.map_impl(f).into()),
            Self::Lit(l) => Regex::Lit(f(l)),
        }
    }

    #[inline]
    pub fn map<M, F: FnMut(L) -> M>(self, mut f: F) -> Regex<M> { self.map_impl(&mut f) }

    #[must_use]
    pub fn alt<I: IntoIterator<Item = Self>>(self, it: I) -> Self {
        match self {
            Self::Alt(mut v) => {
                v.extend(it);
                Self::Alt(v)
            },
            r => Self::Alt([r].into_iter().chain(it).collect()),
        }
    }

    #[must_use]
    pub fn cat<I: IntoIterator<Item = Self>>(self, it: I) -> Self {
        match self {
            Self::Cat(mut v) => {
                v.extend(it);
                Self::Cat(v)
            },
            r => Self::Cat([r].into_iter().chain(it).collect()),
        }
    }

    #[must_use]
    pub fn star(self) -> Self {
        match self {
            r @ Self::Star(_) => r,
            r => Self::Star(r.into()),
        }
    }

    fn dot_impl<'a, F: Fn(&L) -> Cow<'a, str>>(
        &self,
        graph: &mut dot::Graph<'a>,
        free_id: &mut Free<usize>,
        fmt_lit: &mut F,
        tok: Option<Cow<'a, str>>,
    ) -> String {
        let id = free_id.fresh().to_string();

        let label = match self {
            Regex::Alt(v) => {
                for re in v {
                    let sub_id = re.dot_impl(graph, free_id, fmt_lit, None);
                    graph.edge(id.clone(), sub_id);
                }
                "∪".into()
            },
            Regex::Cat(v) => {
                for re in v {
                    let sub_id = re.dot_impl(graph, free_id, fmt_lit, None);
                    graph.edge(id.clone(), sub_id);
                }
                "+".into()
            },
            Regex::Star(r) => {
                let sub_id = r.dot_impl(graph, free_id, fmt_lit, None);
                graph.edge(id.clone(), sub_id);
                "∗".into()
            },
            Regex::Lit(l) => fmt_lit(l),
        };

        let node = graph.node(id.clone());
        let label = if let Some(tok) = tok {
            node.border_count("2");

            format!("{tok}: {label}").into()
        } else {
            label
        };

        node.label(label);

        id
    }

    #[inline]
    pub fn dot<'a, F: Fn(&L) -> Cow<'a, str>>(&self, mut fmt_lit: F) -> dot::Graph<'a> {
        let mut graph = dot::Graph::new(dot::GraphType::Directed);
        let mut free_id = Free::default();

        self.dot_impl(&mut graph, &mut free_id, &mut fmt_lit, None);

        graph
    }
}

impl<L: IntoSymbols> Regex<L> {
    pub fn flatten(self) -> Regex<Symbol<L::Atom>> {
        match self {
            Self::Alt(v) => Regex::Alt(v.into_iter().map(Self::flatten).collect()),
            Self::Cat(v) => Regex::Cat(v.into_iter().map(Self::flatten).collect()),
            Self::Star(r) => Regex::Star(r.flatten().into()),
            Self::Lit(l) => Regex::Cat(l.into_symbols().map(Regex::Lit).collect()),
        }
    }
}

impl<L: IntoSymbols<Atom: Clone + Ord + Succ>> Regex<L> {
    #[inline]
    #[must_use]
    pub fn compile(self) -> Nfa<L::Atom, Option<()>> { NfaBuilder::build([(self, ())]).finish() }
}

pub type Token<L, T> = (Regex<L>, T);
pub type TokenList<L, T> = Vec<Token<L, T>>;

#[derive(Debug, Default)]
#[repr(transparent)]
pub struct RegexBag<L, T>(TokenList<L, T>);

impl<L, T> RegexBag<L, T> {
    #[inline]
    pub fn dot<'a, FL: Fn(&L) -> Cow<'a, str>, FT: Fn(&T) -> Cow<'a, str>>(
        &self,
        mut fmt_lit: FL,
        fmt_tok: FT,
    ) -> dot::Graph<'a> {
        let mut graph = dot::Graph::new(dot::GraphType::Directed);
        let mut free_id = Free::default();

        for (re, tok) in &self.0 {
            re.dot_impl(&mut graph, &mut free_id, &mut fmt_lit, Some(fmt_tok(tok)));
        }

        graph
    }
}

impl<L, T> From<TokenList<L, T>> for RegexBag<L, T> {
    #[inline]
    fn from(toks: TokenList<L, T>) -> Self { Self(toks) }
}

impl<L, T> From<RegexBag<L, T>> for TokenList<L, T> {
    #[inline]
    fn from(RegexBag(toks): RegexBag<L, T>) -> Self { toks }
}

impl<L, T> Extend<Token<L, T>> for RegexBag<L, T> {
    #[inline]
    fn extend<I: IntoIterator<Item = Token<L, T>>>(&mut self, it: I) { self.0.extend(it); }
}

impl<L, T> FromIterator<Token<L, T>> for RegexBag<L, T> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = Token<L, T>>>(it: I) -> Self {
        Self(TokenList::from_iter(it))
    }
}

impl<L: IntoSymbols<Atom: Clone + Ord + Succ>, T: Ord> RegexBag<L, T> {
    #[inline]
    #[must_use]
    pub fn compile(self) -> Nfa<L::Atom, Option<T>> { NfaBuilder::build(self.0).finish() }
}

#[cfg(any(test, feature = "proptest"))]
pub use prop::*;

#[cfg(any(test, feature = "proptest"))]
mod prop {
    use prop::sample::SizeRange;
    use proptest::prelude::*;

    use super::Regex;

    pub fn re(
        depth: u32,
        tree_size: u32,
        branch_size: u32,
        lit_size: impl Into<SizeRange>,
        chr: impl Strategy<Value = char> + 'static,
    ) -> impl Strategy<Value = Regex<Vec<char>>> {
        prop::collection::vec(chr, lit_size)
            .prop_map(Regex::Lit)
            .prop_recursive(depth, tree_size, branch_size, move |s| {
                let size = 0..=(branch_size.try_into().unwrap());
                prop_oneof![
                    prop::collection::vec(s.clone(), size.clone()).prop_map(Regex::Alt),
                    prop::collection::vec(s.clone(), size).prop_map(Regex::Cat),
                    s.prop_map(|r| Regex::Star(r.into())),
                ]
            })
    }
}
