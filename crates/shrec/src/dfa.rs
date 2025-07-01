use std::{borrow::Cow, collections::BTreeMap, hash::Hash, mem};

use hashbrown::HashMap;

use crate::{
    autom::Accept,
    dot,
    free::{Free, Succ},
    partition_map::Partition,
    range_map::RangeMap,
};

pub mod optimize;

pub const DFA_START: usize = 0;

pub fn collect_state_keys<
    S: Clone + Eq + Hash,
    I: IntoIterator<Item = S, IntoIter: ExactSizeIterator>,
>(
    it: I,
    start: &S,
) -> HashMap<S, usize> {
    let mut zero = None;

    let it = it.into_iter();
    let len = it.len();
    let mut map: HashMap<_, _> = it
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            if i == 0 && s != *start {
                assert!(zero.replace(s.clone()).is_none());
            }

            (s, i)
        })
        .collect();
    assert!(map.len() == len);

    if let Some(zero) = zero {
        let [l, r] = map.get_many_mut([&zero, start]);
        mem::swap(l.unwrap(), r.unwrap());
    }

    map
}

pub fn collect_states<
    K: Eq + Hash,
    V,
    I: IntoIterator<Item = (K, V), IntoIter: ExactSizeIterator>,
>(
    map: &HashMap<K, usize>,
    it: I,
) -> Vec<V> {
    let it = it.into_iter();
    let len = it.len();
    let map: BTreeMap<usize, _> = it.map(|(k, v)| (*map.get(&k).unwrap(), v)).collect();
    assert!(map.len() == len);
    map.into_values().collect()
}

#[derive(Debug, PartialEq)]
pub struct State<I, T = (), E = ()>(pub RangeMap<I, (E, usize)>, pub T);

pub type DfaStates<I, T = (), E = ()> = Vec<State<I, T, E>>;

#[derive(Debug, PartialEq)]
pub struct Dfa<I, T = (), E = ()>(DfaStates<I, T, E>);

impl<I, T, E> Dfa<I, T, E> {
    #[inline]
    #[must_use]
    pub fn into_states(self) -> DfaStates<I, T, E> { self.0 }
}

impl<I, T, E> Dfa<I, T, E> {
    #[must_use]
    pub fn new(states: DfaStates<I, T, E>) -> Self {
        assert!(!states.is_empty(), "Missing DFA start state");

        for node in &states {
            for &(_, out) in node.0.values() {
                assert!(out < states.len(), "DFA delta out of bounds");
            }
        }

        Self(states)
    }

    pub fn map_nodes<U>(self, f: impl Fn(T) -> U) -> Dfa<I, U, E> {
        Dfa(self
            .0
            .into_iter()
            .map(|State(e, a)| State(e, f(a)))
            .collect())
    }

    // TODO: try_trait_v2 wen eta
    pub fn try_map_nodes<U, F>(self, f: impl Fn(T) -> Result<U, F>) -> Result<Dfa<I, U, E>, F> {
        Ok(Dfa(self
            .0
            .into_iter()
            .map(|State(e, a)| Ok(State(e, f(a)?)))
            .collect::<Result<_, _>>()?))
    }
}

impl<I: Copy + Ord, T, E> Dfa<&I, T, E> {
    #[must_use]
    pub fn copied(self) -> Dfa<I, T, E> {
        Dfa(self
            .0
            .into_iter()
            .map(|State(e, a)| State(e.copied_keys(), a))
            .collect())
    }
}

impl<I: Clone + Ord + Hash + Succ, T: Clone + Ord + Hash, E: Clone + Ord + Hash> Dfa<I, T, E> {
    #[must_use]
    pub fn optimize(&self) -> optimize::Output<I, T, E> { optimize::run_default(self) }
}

impl<I, T: Accept, E: Accept> Dfa<I, T, E> {
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
            self.0
                .iter()
                .enumerate()
                .map(|(s, State(e, a))| (s, e.ranges().map(|(k, &(ref e, v))| (k, [(e, v)])), a)),
            DFA_START,
            |n| *node_ids.entry(n).or_insert_with(|| free_id.fresh()),
            fmt_state,
            fmt_input,
            fmt_node_tok,
            fmt_edge_tok,
        )
    }
}
