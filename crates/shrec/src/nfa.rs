use std::{
    borrow::{Borrow, Cow},
    collections::{btree_map, BTreeMap, BTreeSet},
    hash::Hash,
    rc::Rc,
};

use self::dfa_builder::DfaBuilder;
use crate::{dfa::Dfa, dot};

mod dfa_builder;

#[derive(Debug)]
pub struct Node<I, N, E>(BTreeMap<Option<I>, BTreeMap<N, E>>);

impl<I, N, E> Default for Node<I, N, E> {
    fn default() -> Self { Self(BTreeMap::default()) }
}

impl<I: Ord, N, E> Node<I, N, E> {
    #[inline]
    pub fn edges(&self) -> btree_map::Iter<Option<I>, BTreeMap<N, E>> { self.0.iter() }

    #[inline]
    pub fn get<Q: Ord + ?Sized>(&self, inp: &Q) -> Option<&BTreeMap<N, E>>
    where Option<I>: Borrow<Q> {
        self.0.get(inp)
    }
}

#[derive(Debug)]
pub struct Nfa<I, N, E, T> {
    nodes: BTreeMap<N, Node<I, N, E>>,
    start: N,
    accept: BTreeMap<T, N>,
}

impl<I: Ord, N: Clone + Ord, E, T: Ord> Nfa<I, N, E, T> {
    pub fn new(start: N) -> Self {
        let mut me = Self {
            nodes: BTreeMap::new(),
            start: start.clone(),
            accept: BTreeMap::new(),
        };
        assert!(me.insert(start).is_none());
        me
    }
}

impl<I: Ord, N: Ord, E, T: Ord> Nfa<I, N, E, T> {
    #[inline]
    pub fn start(&self) -> &N { &self.start }

    #[inline]
    pub fn accept(&self) -> &BTreeMap<T, N> { &self.accept }

    #[inline]
    pub fn get<Q: Ord + ?Sized>(&self, node: &Q) -> Option<&Node<I, N, E>>
    where N: Borrow<Q> {
        self.nodes.get(node)
    }

    #[inline]
    pub fn insert(&mut self, node: N) -> Option<Node<I, N, E>> {
        self.nodes.insert(node, Node::default())
    }

    pub fn connect<Q: Ord + ?Sized>(
        &mut self,
        from: &Q,
        to: N,
        by: Option<I>,
        out: E,
    ) -> Option<E>
    where
        N: Borrow<Q>,
    {
        assert!(self.nodes.contains_key::<N>(&to));
        self.nodes
            .get_mut(from)
            .unwrap()
            .0
            .entry(by)
            .or_default()
            .insert(to, out)
    }
}

impl<I, N: Clone + Ord, E, T: Ord> Nfa<I, N, E, T> {
    #[inline]
    pub fn insert_accept(&mut self, node: N, tok: T) -> Option<(Node<I, N, E>, Option<N>)> {
        let prev = self.nodes.insert(node.clone(), Node::default());
        let prev_accept = self.accept.insert(tok, node);
        assert!(prev.is_some() || prev_accept.is_none());
        prev.map(|p| (p, prev_accept))
    }
}

impl<I: Ord, N: Ord + Hash, T: Ord + Hash> Nfa<I, N, (), T> {
    #[inline]
    pub fn compile(&self) -> Dfa<&I, Rc<BTreeSet<&N>>, (), Rc<BTreeSet<&T>>> {
        DfaBuilder::new(self).build()
    }
}

impl<I, N: Ord, E, T: Ord> Nfa<I, N, E, T> {
    pub fn dot<'a>(
        &self,
        fmt_input: impl Fn(&I) -> Cow<'a, str>,
        fmt_state: impl Fn(&N) -> Cow<'a, str>,
        fmt_output: impl Fn(&E) -> Option<Cow<'a, str>>,
        fmt_tok: impl Fn(&T) -> Option<Cow<'a, str>>,
    ) -> dot::Graph<'a> {
        let mut graph = dot::Graph::new(dot::GraphType::Directed, None);

        let accept_rev: BTreeMap<_, _> = self.accept.iter().map(|(k, v)| (v, k)).collect();

        for (state, Node(edges)) in &self.nodes {
            let node_id = fmt_state(state);
            let node = graph.node(node_id.clone());

            if let Some(tok) = accept_rev.get(state) {
                if let Some(tok) = fmt_tok(tok) {
                    node.label(format!("{node_id}:{tok}").into());
                }

                node.border_count(2);
            }

            for (input, outputs) in edges {
                let input = input.as_ref().map_or_else(|| "ϵ".into(), &fmt_input);

                for (next_state, output) in outputs {
                    let edge = graph.edge(node_id.clone(), fmt_state(next_state));

                    edge.label(if let Some(output) = fmt_output(output) {
                        format!("{input}:{output}").into()
                    } else {
                        input.clone()
                    });
                }
            }
        }

        graph
    }
}
