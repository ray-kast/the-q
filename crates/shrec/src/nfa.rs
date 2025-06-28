use std::{
    borrow::{Borrow, Cow},
    collections::{BTreeMap, BTreeSet},
    hash::Hash,
    sync::Arc,
};

use self::dfa_builder::DfaBuilder;
use crate::{
    dfa::Dfa,
    dot,
    free::Free,
    partition_map::{self, Partition, PartitionMap},
};

mod dfa_builder;

#[derive(Debug)]
pub struct Node<I, N, T> {
    nil: BTreeSet<N>,
    map: PartitionMap<I, BTreeSet<N>>,
    accept: Option<T>,
}

impl<I, N, T> Default for Node<I, N, T> {
    fn default() -> Self {
        Self {
            nil: BTreeSet::new(),
            map: PartitionMap::new(BTreeSet::new()),
            accept: None,
        }
    }
}

impl<I, N, T> Node<I, N, T> {
    #[inline]
    pub fn nil_edges(&self) -> &BTreeSet<N> { &self.nil }

    #[inline]
    pub fn edges(&self) -> Edges<I, N> {
        Edges {
            nil: Some(&self.nil),
            map: self.map.partitions(),
        }
    }
}

#[derive(Debug)]
pub struct Edges<'a, I, N> {
    nil: Option<&'a BTreeSet<N>>,
    map: partition_map::Partitions<'a, I, BTreeSet<N>>,
}

impl<'a, I, N> Iterator for Edges<'a, I, N> {
    type Item = (Option<Partition<&'a I>>, &'a BTreeSet<N>);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(nil) = self.nil.take() {
            return Some((None, nil));
        }

        let (part, val) = self.map.next()?;
        Some((Some(part), val))
    }
}

#[derive(Debug)]
pub struct Nfa<I, N, T> {
    nodes: BTreeMap<N, Node<I, N, T>>,
    start: N,
}

impl<I: Ord, N: Clone + Ord, T: Ord> Nfa<I, N, T> {
    pub fn new(start: N) -> Self {
        let mut me = Self {
            nodes: BTreeMap::new(),
            start: start.clone(),
        };
        assert!(me.insert(start).is_none());
        me
    }
}

impl<I: Ord, N: Ord, T: Ord> Nfa<I, N, T> {
    #[inline]
    pub fn start(&self) -> &N { &self.start }

    #[inline]
    pub fn get<Q: Ord + ?Sized>(&self, node: &Q) -> Option<&Node<I, N, T>>
    where N: Borrow<Q> {
        self.nodes.get(node)
    }

    #[inline]
    pub fn insert(&mut self, node: N) -> Option<Node<I, N, T>> {
        self.nodes.insert(node, Node::default())
    }

    #[inline]
    pub fn insert_accept(&mut self, node: N, tok: T) -> Option<Node<I, N, T>> {
        self.nodes.insert(node, Node {
            accept: Some(tok),
            ..Node::default()
        })
    }
}

impl<I: Clone + Ord, N: Clone + Ord, T: Ord> Nfa<I, N, T> {
    pub fn connect<Q: Ord + ?Sized>(&mut self, from: &Q, to: N, by: Option<Partition<I>>) -> bool
    where N: Borrow<Q> {
        assert!(self.nodes.contains_key::<N>(&to));
        let from = self.nodes.get_mut(from).unwrap();

        if let Some(part) = by {
            let mut any = false;
            from.map.update(part.bounds(), |_, v| {
                let mut s = v.clone();
                any |= s.insert(to.clone());
                s
            });
            any
        } else {
            from.nil.insert(to)
        }
    }
}

impl<I: Clone + Ord, N: Clone + Ord + Hash, T: Clone + Ord + Hash> Nfa<I, N, T> {
    #[inline]
    pub fn compile(&self) -> Dfa<I, Arc<BTreeSet<N>>, Arc<BTreeSet<T>>> {
        DfaBuilder::new(self).build()
    }
}

impl<I, N: Ord, T> Nfa<I, N, T> {
    pub fn dot<'a>(
        &self,
        fmt_input: impl Fn(Partition<&I>) -> Cow<'a, str>,
        fmt_state: impl Fn(&N) -> Cow<'a, str>,
        fmt_tok: impl Fn(&T) -> Option<Cow<'a, str>>,
    ) -> dot::Graph<'a> {
        let mut free_id = Free::from(0);
        let mut node_ids = BTreeMap::new();

        dot::Graph::state_machine(
            self.nodes
                .iter()
                .map(|(s, n)| (s, n.edges(), n.accept.as_ref())),
            &&self.start,
            |n| *node_ids.entry(*n).or_insert_with(|| free_id.fresh()),
            |i| i.map_or_else(|| "ϵ".into(), &fmt_input),
            fmt_state,
            fmt_tok,
        )
    }
}
