use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    hash::Hash,
};

use self::dfa_builder::DfaBuilder;
use crate::{
    autom::Accept,
    dfa::{self, Dfa},
    dot,
    free::Free,
    partition_map::{self, Partition, PartitionMap},
};

mod dfa_builder;

pub const NFA_START: usize = 0;

#[derive(Debug)]
pub struct Node<I, T = (), E = ()> {
    nil: BTreeSet<(E, usize)>,
    map: PartitionMap<I, BTreeSet<(E, usize)>>,
    accept: T,
}

impl<I, T, E> Node<I, T, E> {
    fn new(accept: T) -> Self {
        Self {
            nil: BTreeSet::new(),
            map: PartitionMap::new(BTreeSet::new()),
            accept,
        }
    }
}

impl<I, T, E> Node<I, T, E> {
    #[inline]
    pub fn nil_edges(&self) -> &BTreeSet<(E, usize)> { &self.nil }

    #[inline]
    pub fn edges(&self) -> Edges<I, E> {
        Edges {
            nil: Some(&self.nil),
            map: self.map.partitions(),
        }
    }
}

#[derive(Debug)]
pub struct Edges<'a, I, E> {
    nil: Option<&'a BTreeSet<(E, usize)>>,
    map: partition_map::Partitions<'a, I, BTreeSet<(E, usize)>>,
}

impl<'a, I, E> Iterator for Edges<'a, I, E> {
    type Item = (Option<Partition<&'a I>>, &'a BTreeSet<(E, usize)>);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(nil) = self.nil.take() {
            return Some((None, nil));
        }

        let (part, val) = self.map.next()?;
        Some((Some(part), val))
    }
}

#[derive(Debug)]
pub struct Nfa<I, T = (), E = ()>(Vec<Node<I, T, E>>);

impl<I, T: Default, E> Default for Nfa<I, T, E> {
    #[inline]
    fn default() -> Self { Self::new() }
}

impl<I, T, E> Nfa<I, T, E> {
    #[must_use]
    pub fn with_start_accept(start_accept: T) -> Self {
        let mut me = Self(vec![]);
        assert!(me.push_accept(start_accept) == 0);
        me
    }

    #[inline]
    #[must_use]
    pub fn get(&self, node: usize) -> Option<&Node<I, T, E>> { self.0.get(node) }

    #[inline]
    pub fn push_accept(&mut self, accept: T) -> usize {
        let ret = self.0.len();
        self.0.push(Node::new(accept));
        ret
    }
}

impl<I, T: Default, E> Nfa<I, T, E> {
    #[inline]
    #[must_use]
    pub fn new() -> Self { Self::with_start_accept(T::default()) }

    #[inline]
    pub fn push(&mut self) -> usize { self.push_accept(T::default()) }
}

impl<I: Clone + Ord, T: Ord, E: Clone + Ord> Nfa<I, T, E> {
    pub fn connect(&mut self, from: usize, to: usize, by: Option<Partition<I>>, out: E) -> bool {
        assert!(to < self.0.len());
        let from = self.0.get_mut(from).unwrap();

        if let Some(part) = by {
            let mut any = false;
            from.map.update(part.bounds(), |_, v| {
                let mut s = v.clone();
                any |= s.insert((out.clone(), to));
                s
            });
            any
        } else {
            from.nil.insert((out, to))
        }
    }
}

impl<
        I: Clone + Ord,
        T: Accept<Token: Clone + Ord + Hash>,
        E: Accept<Token: Clone + Ord + Hash>,
    > Nfa<I, T, E>
{
    #[inline]
    #[must_use]
    pub fn compile(&self) -> dfa_builder::Output<I, T, E> { DfaBuilder::new(self).build() }
}

impl<I, T: Accept, E: Accept> Nfa<I, T, E> {
    pub fn dot<'a>(
        &self,
        fmt_state: impl Fn(usize) -> Cow<'a, str>,
        fmt_input: impl Fn(Partition<&I>) -> Cow<'a, str>,
        fmt_node_tok: impl Fn(&T::Token) -> Option<Cow<'a, str>>,
        fmt_edge_tok: impl Fn(&E::Token) -> Option<Cow<'a, str>>,
    ) -> dot::Graph<'a> {
        let mut free_id = Free::from(0);
        let mut node_ids = BTreeMap::new();

        dot::Graph::state_machine(
            self.0.iter().enumerate().map(|(s, n)| {
                (
                    s,
                    n.edges()
                        .map(|(k, v)| (k, v.iter().map(|&(ref e, n)| (e, n)))),
                    &n.accept,
                )
            }),
            NFA_START,
            |n| *node_ids.entry(n).or_insert_with(|| free_id.fresh()),
            fmt_state,
            |i| i.map_or_else(|| "ϵ".into(), &fmt_input),
            fmt_node_tok,
            fmt_edge_tok,
        )
    }
}

impl<I: Clone + Ord, T, E: Clone + Ord> From<Dfa<I, T, E>> for Nfa<I, T, E> {
    fn from(value: Dfa<I, T, E>) -> Self {
        Self(
            value
                .into_states()
                .into_iter()
                .map(|dfa::State(edges, accept)| Node {
                    nil: BTreeSet::new(),
                    map: edges
                        .into_ranges()
                        .map(|(k, v)| (k, [v].into_iter().collect()))
                        .collect(),
                    accept,
                })
                .collect(),
        )
    }
}
