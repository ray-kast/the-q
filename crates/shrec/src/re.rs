use nfa_builder::NfaBuilder;

use crate::nfa::Nfa;

mod nfa_builder;

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
    pub fn compile(self) -> Nfa<L::Item, u64, ()> { NfaBuilder::build(self).finish() }
}
