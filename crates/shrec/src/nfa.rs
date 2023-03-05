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
pub struct Nfa<I, N, E> {
    nodes: BTreeMap<N, Node<I, N, E>>,
    head: N,
    tail: N,
}

impl<I: Ord, N: Clone + Ord, E> Nfa<I, N, E> {
    pub fn new(head: N, tail: N) -> Self {
        let mut me = Self {
            nodes: BTreeMap::new(),
            head: head.clone(),
            tail: tail.clone(),
        };
        assert!(me.insert(head).is_none());
        assert!(me.insert(tail).is_none());
        me
    }
}

impl<I: Ord, N: Ord, E> Nfa<I, N, E> {
    #[inline]
    pub fn head(&self) -> &N { &self.head }

    #[inline]
    pub fn tail(&self) -> &N { &self.tail }

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

impl<I: Ord, N: Ord + Hash> Nfa<I, N, ()> {
    #[inline]
    pub fn compile(&self) -> Dfa<&I, Rc<BTreeSet<&N>>, ()> { DfaBuilder::new(self).build() }
}

impl<I, N: Eq, E> Nfa<I, N, E> {
    pub fn dot<'a>(
        &self,
        fmt_input: impl Fn(&I) -> Cow<'a, str>,
        fmt_state: impl Fn(&N) -> Cow<'a, str>,
        fmt_output: impl Fn(&E) -> Option<Cow<'a, str>>,
    ) -> dot::Graph<'a> {
        let mut graph = dot::Graph::new(dot::GraphType::Directed, None);

        for (state, Node(edges)) in &self.nodes {
            let node_id = fmt_state(state);
            let node = graph.node(node_id.clone());

            if *state == self.tail {
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
