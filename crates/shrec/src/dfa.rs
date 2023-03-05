use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    hash::Hash,
};

mod atomize;
mod scanner;

use hashbrown::HashMap;
pub use scanner::Scanner;

use self::atomize::DfaAtomizer;
use crate::{dot, free::Succ};

#[derive(Debug)]
#[repr(transparent)]
pub struct Node<I, N, E>(BTreeMap<I, (N, E)>);

#[derive(Debug)]
pub struct Dfa<I, N, E> {
    states: BTreeMap<N, Node<I, N, E>>,
    start: N,
    accept: BTreeSet<N>,
}

impl<I, N: Ord, E> Dfa<I, N, E> {
    pub fn new(
        states: impl IntoIterator<Item = (N, BTreeMap<I, (N, E)>)>,
        start: N,
        accept: BTreeSet<N>,
    ) -> Self {
        let states: BTreeMap<_, _> = states.into_iter().map(|(k, v)| (k, Node(v))).collect();
        assert!(states.contains_key(&start));
        Self {
            states,
            start,
            accept,
        }
    }
}

impl<I: Copy + Ord, N: Ord, E> Dfa<&I, N, E> {
    #[must_use]
    pub fn copied(self) -> Dfa<I, N, E> {
        let Self {
            states,
            start,
            accept,
        } = self;
        Dfa {
            states: states
                .into_iter()
                .map(|(k, Node(v))| (k, Node(v.into_iter().map(|(&k, v)| (k, v)).collect())))
                .collect(),
            start,
            accept,
        }
    }
}

impl<I: Ord, N: Ord + Hash, E> Dfa<I, N, E> {
    pub fn atomize_nodes<A: Default + Copy + Ord + Succ>(self) -> (Dfa<I, A, E>, HashMap<N, A>) {
        DfaAtomizer::default().atomize_nodes(self)
    }
}

impl<I, N: Ord, E> Dfa<I, N, E> {
    pub fn dot<'a>(
        &self,
        fmt_input: impl Fn(&I) -> Cow<'a, str>,
        fmt_state: impl Fn(&N) -> Cow<'a, str>,
        fmt_output: impl Fn(&E) -> Option<Cow<'a, str>>,
    ) -> dot::Graph<'a> {
        let mut graph = dot::Graph::new(dot::GraphType::Directed, None);

        for (state, Node(edges)) in &self.states {
            let node_id = fmt_state(state);
            let node = graph.node(node_id.clone());

            if self.accept.contains(state) {
                node.border_count(2);
            }

            for (input, (next_state, output)) in edges {
                let edge = graph.edge(node_id.clone(), fmt_state(next_state));

                let input = fmt_input(input);
                edge.label(if let Some(output) = fmt_output(output) {
                    format!("{input}:{output}").into()
                } else {
                    input.clone()
                });
            }
        }

        graph
    }
}
