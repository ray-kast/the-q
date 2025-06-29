use std::{
    borrow::{Borrow, Cow},
    collections::{BTreeMap, BTreeSet},
    hash::Hash,
};

use self::dfa_builder::DfaBuilder;
use crate::{
    dot,
    free::Free,
    partition_map::{self, Partition, PartitionMap},
};

mod dfa_builder;

#[derive(Debug)]
pub struct Node<I, N, E, T> {
    nil: BTreeSet<(Option<E>, N)>,
    map: PartitionMap<I, BTreeSet<(Option<E>, N)>>,
    accept: Option<T>,
}

impl<I, N, E, T> Default for Node<I, N, E, T> {
    fn default() -> Self {
        Self {
            nil: BTreeSet::new(),
            map: PartitionMap::new(BTreeSet::new()),
            accept: None,
        }
    }
}

impl<I, N, E, T> Node<I, N, E, T> {
    #[inline]
    pub fn nil_edges(&self) -> &BTreeSet<(Option<E>, N)> { &self.nil }

    #[inline]
    pub fn edges(&self) -> Edges<I, N, E> {
        Edges {
            nil: Some(&self.nil),
            map: self.map.partitions(),
        }
    }
}

#[derive(Debug)]
pub struct Edges<'a, I, N, E> {
    nil: Option<&'a BTreeSet<(Option<E>, N)>>,
    map: partition_map::Partitions<'a, I, BTreeSet<(Option<E>, N)>>,
}

impl<'a, I, N, E> Iterator for Edges<'a, I, N, E> {
    type Item = (Option<Partition<&'a I>>, &'a BTreeSet<(Option<E>, N)>);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(nil) = self.nil.take() {
            return Some((None, nil));
        }

        let (part, val) = self.map.next()?;
        Some((Some(part), val))
    }
}

#[derive(Debug)]
pub struct Nfa<I, N, E, T> {
    nodes: BTreeMap<N, Node<I, N, E, T>>,
    start: N,
}

impl<I: Ord, N: Clone + Ord, E, T: Ord> Nfa<I, N, E, T> {
    pub fn new(start: N) -> Self {
        let mut me = Self {
            nodes: BTreeMap::new(),
            start: start.clone(),
        };
        assert!(me.insert(start).is_none());
        me
    }
}

impl<I: Ord, N: Ord, E, T: Ord> Nfa<I, N, E, T> {
    #[inline]
    pub fn start(&self) -> &N { &self.start }

    #[inline]
    pub fn get<Q: Ord + ?Sized>(&self, node: &Q) -> Option<&Node<I, N, E, T>>
    where N: Borrow<Q> {
        self.nodes.get(node)
    }

    #[inline]
    pub fn insert(&mut self, node: N) -> Option<Node<I, N, E, T>> {
        self.nodes.insert(node, Node::default())
    }

    #[inline]
    pub fn insert_accept(&mut self, node: N, tok: T) -> Option<Node<I, N, E, T>> {
        self.nodes.insert(node, Node {
            accept: Some(tok),
            ..Node::default()
        })
    }
}

impl<I: Clone + Ord, N: Clone + Ord, E: Clone + Ord, T: Ord> Nfa<I, N, E, T> {
    pub fn connect<Q: Ord + ?Sized>(
        &mut self,
        from: &Q,
        to: N,
        by: Option<Partition<I>>,
        out: Option<E>,
    ) -> bool
    where
        N: Borrow<Q>,
    {
        assert!(self.nodes.contains_key::<N>(&to));
        let from = self.nodes.get_mut(from).unwrap();

        if let Some(part) = by {
            let mut any = false;
            from.map.update(part.bounds(), |_, v| {
                let mut s = v.clone();
                any |= s.insert((out.clone(), to.clone()));
                s
            });
            any
        } else {
            from.nil.insert((out, to))
        }
    }
}

impl<I: Clone + Ord, N: Clone + Ord + Hash, E: Clone + Ord + Hash, T: Clone + Ord + Hash>
    Nfa<I, N, E, T>
{
    #[inline]
    pub fn compile(&self) -> dfa_builder::Output<I, N, E, T> { DfaBuilder::new(self).build() }
}

impl<I, N: Ord, E, T> Nfa<I, N, E, T> {
    pub fn dot<'a>(
        &self,
        fmt_input: impl Fn(Partition<&I>) -> Cow<'a, str>,
        fmt_state: impl Fn(&N) -> Cow<'a, str>,
        fmt_edge: impl Fn(&E) -> Option<Cow<'a, str>>,
        fmt_tok: impl Fn(&T) -> Option<Cow<'a, str>>,
    ) -> dot::Graph<'a> {
        let mut free_id = Free::from(0);
        let mut node_ids = BTreeMap::new();

        dot::Graph::state_machine(
            self.nodes.iter().map(|(s, n)| {
                (
                    s,
                    n.edges()
                        .map(|(k, v)| (k, v.iter().map(|(e, n)| (e.as_ref(), n)))),
                    n.accept.as_ref(),
                )
            }),
            &&self.start,
            |n| *node_ids.entry(*n).or_insert_with(|| free_id.fresh()),
            |i| i.map_or_else(|| "ϵ".into(), &fmt_input),
            fmt_state,
            fmt_edge,
            fmt_tok,
        )
    }
}
