use std::{borrow::Cow, collections::BTreeMap, hash::Hash};

use hashbrown::HashMap;
pub use scanner::Scanner;

use self::atomize::DfaAtomizer;
use crate::{
    dot,
    free::{Free, Succ},
};

mod atomize;
pub mod optimize;
mod scanner;

#[derive(Debug)]
pub struct Node<I, N, T>(BTreeMap<I, N>, Option<T>);

#[derive(Debug)]
pub struct Dfa<I, N, T> {
    states: BTreeMap<N, Node<I, N, T>>,
    start: N,
}

impl<I, N: Ord, T> Dfa<I, N, T> {
    pub fn new(states: impl IntoIterator<Item = (N, BTreeMap<I, N>, Option<T>)>, start: N) -> Self {
        let states: BTreeMap<_, _> = states
            .into_iter()
            .map(|(n, e, a)| (n, Node(e, a)))
            .collect();
        assert!(states.contains_key(&start));
        Self { states, start }
    }

    pub fn map_token<U>(self, f: impl Fn(T) -> U) -> Dfa<I, N, U> {
        let Self { states, start } = self;
        Dfa {
            states: states
                .into_iter()
                .map(|(n, Node(e, a))| (n, Node(e, a.map(&f))))
                .collect(),
            start,
        }
    }

    // TODO: try_trait_v2 wen eta
    pub fn try_map_token<U, F>(self, f: impl Fn(T) -> Result<U, F>) -> Result<Dfa<I, N, U>, F> {
        let Self { states, start } = self;
        let states = states
            .into_iter()
            .map(|(n, Node(e, a))| Ok((n, Node(e, a.map(&f).transpose()?))))
            .collect::<Result<_, _>>()?;
        Ok(Dfa { states, start })
    }
}

impl<I: Copy + Ord, N: Ord, T> Dfa<&I, N, T> {
    #[must_use]
    pub fn copied(self) -> Dfa<I, N, T> {
        let Self { states, start } = self;
        Dfa {
            states: states
                .into_iter()
                .map(|(n, Node(e, a))| (n, Node(e.into_iter().map(|(&k, v)| (k, v)).collect(), a)))
                .collect(),
            start,
        }
    }
}

impl<I: Ord, N: Ord + Hash, T> Dfa<I, N, T> {
    pub fn atomize_nodes<A: Default + Copy + Ord + Succ>(self) -> (Dfa<I, A, T>, HashMap<N, A>) {
        DfaAtomizer::default().atomize_nodes(self)
    }
}

impl<I: Copy + Ord + Hash, N: Copy + Ord + Hash, T: Clone + Ord + Hash> Dfa<I, N, T> {
    pub fn optimize(&self) -> optimize::Output<I, N, T> { optimize::run(self) }
}

impl<I, N: Ord, T> Dfa<I, N, T> {
    pub fn dot<'a>(
        &self,
        fmt_input: impl Fn(&I) -> Cow<'a, str>,
        fmt_state: impl Fn(&N) -> Cow<'a, str>,
        fmt_tok: impl Fn(&T) -> Option<Cow<'a, str>>,
    ) -> dot::Graph<'a> {
        let mut free_id = Free::from(0);
        let mut node_ids = BTreeMap::new();

        dot::Graph::state_machine(
            self.states
                .iter()
                .map(|(s, Node(e, a))| (s, e.iter().map(|(i, n)| (i, [n])), a.as_ref())),
            &&self.start,
            |n| *node_ids.entry(*n).or_insert_with(|| free_id.fresh()),
            fmt_input,
            fmt_state,
            fmt_tok,
        )
    }
}
