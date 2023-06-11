use std::{borrow::Cow, collections::BTreeMap, hash::Hash};

use hashbrown::HashMap;
pub use scanner::Scanner;

use self::atomize::DfaAtomizer;
use crate::{dot, free::Succ};

mod atomize;
mod scanner;

#[derive(Debug)]
#[repr(transparent)]
pub struct Node<I, N, E>(BTreeMap<I, (N, E)>);

#[derive(Debug)]
pub struct Dfa<I, N, E, T> {
    states: BTreeMap<N, Node<I, N, E>>,
    start: N,
    accept: BTreeMap<N, T>,
}

impl<I, N: Ord, E, T> Dfa<I, N, E, T> {
    pub fn new(
        states: impl IntoIterator<Item = (N, BTreeMap<I, (N, E)>)>,
        start: N,
        accept: BTreeMap<N, T>,
    ) -> Self {
        let states: BTreeMap<_, _> = states.into_iter().map(|(k, v)| (k, Node(v))).collect();
        assert!(states.contains_key(&start));
        Self {
            states,
            start,
            accept,
        }
    }

    pub fn map_token<U>(self, f: impl Fn(T) -> U) -> Dfa<I, N, E, U> {
        let Self {
            states,
            start,
            accept,
        } = self;
        Dfa {
            states,
            start,
            accept: accept.into_iter().map(|(n, t)| (n, f(t))).collect(),
        }
    }

    // TODO: try_trait_v2 wen eta
    pub fn try_map_token<U, F>(self, f: impl Fn(T) -> Result<U, F>) -> Result<Dfa<I, N, E, U>, F> {
        let Self {
            states,
            start,
            accept,
        } = self;
        let accept = accept
            .into_iter()
            .map(|(n, t)| f(t).map(|t| (n, t)))
            .collect::<Result<_, _>>()?;
        Ok(Dfa {
            states,
            start,
            accept,
        })
    }
}

impl<I: Copy + Ord, N: Ord, E, T> Dfa<&I, N, E, T> {
    #[must_use]
    pub fn copied(self) -> Dfa<I, N, E, T> {
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

impl<I: Ord, N: Ord + Hash, E, T> Dfa<I, N, E, T> {
    pub fn atomize_nodes<A: Default + Copy + Ord + Succ>(self) -> (Dfa<I, A, E, T>, HashMap<N, A>) {
        DfaAtomizer::default().atomize_nodes(self)
    }
}

impl<I, N: Ord, E, T> Dfa<I, N, E, T> {
    pub fn dot<'a>(
        &self,
        fmt_input: impl Fn(&I) -> Cow<'a, str>,
        fmt_state: impl Fn(&N) -> Cow<'a, str>,
        fmt_output: impl Fn(&E) -> Option<Cow<'a, str>>,
        fmt_tok: impl Fn(&T) -> Option<Cow<'a, str>>,
    ) -> dot::Graph<'a> {
        let mut graph = dot::Graph::new(dot::GraphType::Directed, None);

        for (state, Node(edges)) in &self.states {
            let node_id = fmt_state(state);
            let node = graph.node(node_id.clone());

            if let Some(tok) = self.accept.get(state) {
                if let Some(tok) = fmt_tok(tok) {
                    node.label(format!("{node_id}:{tok}").into());
                }

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
