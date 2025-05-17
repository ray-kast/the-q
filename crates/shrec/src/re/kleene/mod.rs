use nfa_builder::NfaBuilder;

use crate::nfa::Nfa;

mod nfa_builder;

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
}

impl<L: IntoIterator<Item: Ord>> Regex<L> {
    #[inline]
    #[must_use]
    pub fn compile_atomic(self) -> Nfa<L::Item, u64, ()> {
        NfaBuilder::build([(self, ())]).finish()
    }
}

pub type Token<L, T> = (Regex<L>, T);
pub type TokenList<L, T> = Vec<Token<L, T>>;

#[derive(Debug, Default)]
#[repr(transparent)]
pub struct RegexBag<L, T>(TokenList<L, T>);

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

impl<L: IntoIterator<Item: Ord>, T: Ord> RegexBag<L, T> {
    #[inline]
    #[must_use]
    pub fn compile_atomic(self) -> Nfa<L::Item, u64, T> { NfaBuilder::build(self.0).finish() }
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
