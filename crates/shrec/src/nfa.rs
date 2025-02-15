use std::{
    borrow::{Borrow, Cow},
    collections::{btree_map, BTreeMap, BTreeSet},
    hash::Hash,
    sync::Arc,
};

use self::dfa_builder::DfaBuilder;
use crate::{dfa::Dfa, dot};

mod dfa_builder;

#[derive(Debug)]
pub struct Node<I, N, T>(BTreeMap<Option<I>, BTreeSet<N>>, Option<T>);

impl<I, N, T> Default for Node<I, N, T> {
    fn default() -> Self { Self(BTreeMap::default(), None) }
}

impl<I: Ord, N, T> Node<I, N, T> {
    #[inline]
    pub fn edges(&self) -> btree_map::Iter<Option<I>, BTreeSet<N>> { self.0.iter() }

    #[inline]
    pub fn get<Q: Ord + ?Sized>(&self, inp: &Q) -> Option<&BTreeSet<N>>
    where Option<I>: Borrow<Q> {
        self.0.get(inp)
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
        self.nodes.insert(node, Node(BTreeMap::new(), Some(tok)))
    }

    pub fn connect<Q: Ord + ?Sized>(&mut self, from: &Q, to: N, by: Option<I>) -> bool
    where N: Borrow<Q> {
        assert!(self.nodes.contains_key::<N>(&to));
        self.nodes
            .get_mut(from)
            .unwrap()
            .0
            .entry(by)
            .or_default()
            .insert(to)
    }
}

impl<I: Copy + Ord, N: Copy + Ord + Hash, T: Clone + Ord + Hash> Nfa<I, N, T> {
    #[inline]
    pub fn compile(&self) -> Dfa<I, Arc<BTreeSet<N>>, Arc<BTreeSet<T>>> {
        DfaBuilder::new(self).build()
    }
}

impl<I, N: Ord, T: Ord> Nfa<I, N, T> {
    pub fn dot<'a>(
        &self,
        fmt_input: impl Fn(&I) -> Cow<'a, str>,
        fmt_state: impl Fn(&N) -> Cow<'a, str>,
        fmt_tok: impl Fn(&T) -> Option<Cow<'a, str>>,
    ) -> dot::Graph<'a> {
        let mut graph = dot::Graph::new(dot::GraphType::Directed, None);

        for (state, Node(edges, accept)) in &self.nodes {
            let node_id = fmt_state(state);
            let node = graph.node(node_id.clone());

            if let Some(tok) = accept {
                if let Some(tok) = fmt_tok(tok) {
                    node.label(format!("{node_id}:{tok}").into());
                }

                node.border_count(2);
            }

            for (input, outputs) in edges {
                let input = input.as_ref().map_or_else(|| "ϵ".into(), &fmt_input);

                for next_state in outputs {
                    let edge = graph.edge(node_id.clone(), fmt_state(next_state));

                    edge.label(input.clone());
                }
            }
        }

        graph
    }
}
