use nfa_builder::NfaBuilder;

use crate::nfa::Nfa;

mod nfa_builder;
pub mod syntax;

#[derive(Debug)]
pub enum Regex<L> {
    Alt(Vec<Regex<L>>),
    Cat(Vec<Regex<L>>),
    Star(Box<Regex<L>>),
    Lit(L),
}

impl<L> Regex<L> {
    pub const BOTTOM: Regex<L> = Regex::Alt(Vec::new());
    pub const TOP: Regex<L> = Regex::Cat(Vec::new());
}

impl<L: IntoIterator> Regex<L>
where L::Item: Ord
{
    #[inline]
    #[must_use]
    pub fn compile(self) -> Nfa<L::Item, u64, (), ()> { NfaBuilder::build([(self, ())]).finish() }
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

impl<L: IntoIterator, T: Ord> RegexBag<L, T>
where L::Item: Ord
{
    #[inline]
    #[must_use]
    pub fn compile(self) -> Nfa<L::Item, u64, (), T> { NfaBuilder::build(self.0).finish() }
}
