use std::str::Chars;

use crate::{
    free::Succ,
    partition_map::Partition,
    range_set::{self, RangeSet},
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Symbol<S> {
    Atom(S),
    Set(RangeSet<S>),
}

impl<S: Clone> Symbol<S> {
    pub fn into_partitions(self) -> IntoPartitions<S> {
        IntoPartitions(match self {
            Symbol::Atom(a) => IntoPartitionsInner::Atom(Some(a)),
            Symbol::Set(v) => IntoPartitionsInner::Set(v.into_ranges()),
        })
    }
}

impl<S: Clone + Succ> IntoIterator for Symbol<S> {
    type IntoIter = IntoPartitions<S>;
    type Item = Partition<S>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter { self.into_partitions() }
}

#[derive(Debug)]
enum IntoPartitionsInner<S> {
    Atom(Option<S>),
    Set(range_set::IntoRanges<S>),
}

#[derive(Debug)]
pub struct IntoPartitions<S>(IntoPartitionsInner<S>);

impl<S: Clone + Succ> Iterator for IntoPartitions<S> {
    type Item = Partition<S>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            IntoPartitionsInner::Atom(a) => a.take().map(|a| (a.clone()..a.succ()).into()),
            IntoPartitionsInner::Set(v) => v.next(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Run<S, V = Vec<S>> {
    Run(V),
    Set(RangeSet<S>),
}

impl<'a> From<&'a str> for Run<char, Chars<'a>> {
    #[inline]
    fn from(value: &'a str) -> Self { Self::Run(value.chars()) }
}

// TODO: needs IntoChars
// impl From<String> for Run<char, String> {
//     #[inline]
//     fn from(value: String) -> Self { Self::Run(value) }
// }

// impl From<Cow<'_, str>> for Run<char, String> {
//     #[inline]
//     fn from(value: Cow<'_, str>) -> Self { Self::Run(value.into_owned()) }
// }

#[derive(Debug, Clone)]
enum IntoSymsInner<S, V> {
    Run(V),
    Set(Option<RangeSet<S>>),
}

#[derive(Debug, Clone)]
pub struct IntoSyms<S, V>(IntoSymsInner<S, V>);

impl<S, V: Iterator<Item = S>> Iterator for IntoSyms<S, V> {
    type Item = Symbol<S>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            IntoSymsInner::Run(v) => v.next().map(Symbol::Atom),
            IntoSymsInner::Set(s) => s.take().map(Symbol::Set),
        }
    }
}

pub trait IntoSymbols {
    type Atom;
    type Run: Iterator<Item = Symbol<Self::Atom>>;

    fn into_symbols(self) -> Self::Run;
}

impl<S, V: IntoIterator<Item = S>> IntoSymbols for Run<S, V> {
    type Atom = S;
    type Run = IntoSyms<S, V::IntoIter>;

    #[inline]
    fn into_symbols(self) -> <Self as IntoSymbols>::Run {
        IntoSyms(match self {
            Self::Run(v) => IntoSymsInner::Run(v.into_iter()),
            Self::Set(s) => IntoSymsInner::Set(Some(s)),
        })
    }
}

impl<T: IntoIterator> IntoSymbols for T {
    type Atom = T::Item;
    type Run = IntoSyms<T::Item, T::IntoIter>;

    #[inline]
    fn into_symbols(self) -> Self::Run { Run::Run(self).into_symbols() }
}
