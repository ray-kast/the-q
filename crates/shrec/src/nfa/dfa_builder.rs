use std::{
    borrow::BorrowMut,
    collections::{BTreeMap, BTreeSet, VecDeque},
    hash::Hash,
    sync::Arc,
};

use super::{Nfa, NFA_START};
use crate::{
    autom::{Accept, ClosedAccept},
    closure_builder::ClosureBuilder,
    dfa::{self, collect_state_keys, collect_states, Dfa},
    memoize::Memoize,
    partition_map::PartitionMap,
    range_map::RangeMap,
};

type Delta<E> = (Option<E>, usize);
type DeltaSet<E> = BTreeSet<Delta<E>>;

fn unzip_deltas<E, T: Ord, I: IntoIterator<Item = Delta<E>>, F: FnMut(E) -> T>(
    it: I,
    mut map: F,
) -> (BTreeSet<T>, BTreeSet<usize>) {
    it.into_iter().fold(
        (BTreeSet::new(), BTreeSet::new()),
        |(mut es, mut ns), (e, n)| {
            es.extend(e.map(&mut map));
            ns.insert(n);
            (es, ns)
        },
    )
}

// NOTE to future self: Don't attempt to convert the value to an Arc, it needs to
//                      be mutably borrowed.
struct State<I, T: Accept, E: Accept>(
    PartitionMap<I, DeltaSet<E::Token>>,
    BTreeSet<ClosedAccept<T::Token, E::Token>>,
);

impl<I, T: Accept, E: Accept> State<I, T, E> {
    fn new(toks: BTreeSet<ClosedAccept<T::Token, E::Token>>) -> Self {
        Self(PartitionMap::new(BTreeSet::new()), toks)
    }
}

pub type Output<I, T, E> = (
    Dfa<
        I,
        Arc<BTreeSet<ClosedAccept<<T as Accept>::Token, <E as Accept>::Token>>>,
        Arc<BTreeSet<<E as Accept>::Token>>,
    >,
    Vec<Arc<BTreeSet<usize>>>,
);

pub struct DfaBuilder<'a, I, T: Accept, E: Accept> {
    nfa: &'a Nfa<I, T, E>,
    delta_closure: ClosureBuilder<Delta<E::Token>>,
}

impl<
        'a,
        I: Clone + Ord,
        T: Accept<Token: Clone + Ord + Hash>,
        E: Accept<Token: Clone + Ord + Hash>,
    > DfaBuilder<'a, I, T, E>
{
    pub fn new(nfa: &'a Nfa<I, T, E>) -> Self {
        Self {
            nfa,
            delta_closure: ClosureBuilder::default(),
        }
    }

    fn solve_delta<S: BorrowMut<DeltaSet<E::Token>>>(&mut self, set: S) -> S {
        self.delta_closure.solve(set, |(_, n)| {
            self.nfa
                .get(n)
                .unwrap()
                .nil_edges()
                .iter()
                .map(|&(ref e, n)| (e.as_token().cloned(), n))
        })
    }

    #[inline]
    pub fn build(&mut self) -> Output<I, T, E> {
        let mut memo_state = Memoize::default();
        let mut memo_node_tok = Memoize::default();
        let mut memo_edge_tok = Memoize::default();
        let no_node_tok = memo_node_tok.memoize(BTreeSet::new());
        self.delta_closure.init([(None, NFA_START)]);
        let (start_edge_toks, start) =
            unzip_deltas(self.solve_delta(BTreeSet::new()), ClosedAccept::Edge);
        let start: Arc<BTreeSet<usize>> = memo_state.memoize(start);

        if start.is_empty() {
            return (
                Dfa::new(vec![dfa::State(RangeMap::new(), no_node_tok)]),
                vec![start],
            );
        }

        let mut states: BTreeMap<Arc<BTreeSet<usize>>, State<I, T, E>> = BTreeMap::default();
        let mut q: VecDeque<_> = [(start_edge_toks, Arc::clone(&start))]
            .into_iter()
            .collect();

        while let Some((closed_toks, state_set)) = q.pop_front() {
            use std::collections::btree_map::Entry;

            debug_assert!(!state_set.is_empty());

            // TODO: insert_entry pls
            let Entry::Vacant(v) = states.entry(Arc::clone(&state_set)) else {
                continue;
            };
            let node = v.insert(State::new(closed_toks));

            for &state in state_set.iter() {
                for (inp, deltas) in self.nfa.get(state).unwrap().map.partitions() {
                    let mut closed_deltas = BTreeSet::new();

                    self.delta_closure
                        .init(deltas.iter().map(|&(ref e, n)| (e.as_token().cloned(), n)));
                    self.solve_delta(&mut closed_deltas);
                    node.0.update(inp.to_owned().bounds(), |_, v| {
                        closed_deltas.union(v).cloned().collect()
                    });
                }
            }

            node.1.extend(state_set.iter().filter_map(|&n| {
                self.nfa
                    .0
                    .get(n)
                    .unwrap()
                    .accept
                    .as_token()
                    .cloned()
                    .map(ClosedAccept::Node)
            }));

            // Drop the mutable borrow created by calling BTreeMap::entry
            let node = states.get(&state_set).unwrap_or_else(|| unreachable!());

            for delta in node.0.values() {
                let set = memo_state.memoize(delta.iter().map(|&(_, n)| n).collect());

                // Try our very hardest to avoid cloning the set again
                if !(set.is_empty() || states.contains_key(&set)) {
                    q.push_back((BTreeSet::new(), set));
                }
            }
        }

        let state_ids = collect_state_keys(states.keys().cloned(), &start);

        let (states, state_sets) = collect_states(
            &state_ids,
            states.into_iter().map(|(s, State(e, k))| {
                (
                    s,
                    dfa::State(
                        e.into_partitions()
                            .filter(|(_, s)| !s.is_empty())
                            .map(|(i, s)| {
                                let (edges, node) = unzip_deltas(s, |e| e);
                                (
                                    i,
                                    (memo_edge_tok.memoize(edges), *state_ids.get(&node).unwrap()),
                                )
                            })
                            .collect(),
                        memo_node_tok.memoize(k),
                    ),
                )
            }),
        );

        (Dfa::new(states), state_sets)
    }
}

pub type Moore<I, T> = (
    Dfa<I, Arc<BTreeSet<<T as Accept>::Token>>, ()>,
    Vec<Arc<BTreeSet<usize>>>,
);

impl<I: Clone + Ord, T: Accept<Token: Clone + Ord + Hash>> DfaBuilder<'_, I, T, ()> {
    pub fn build_moore(&mut self) -> Moore<I, T> {
        let (dfa, sets) = self.build();
        let mut memo_tok = Memoize::default();

        (
            dfa.map_nodes(|t| {
                memo_tok.memoize(
                    t.iter()
                        .map(|t| match t {
                            ClosedAccept::Node(n) => n.clone(),
                            ClosedAccept::Edge(e) => match *e {},
                        })
                        .collect(),
                )
            })
            .map_edges(|e| debug_assert!(e.is_empty())),
            sets,
        )
    }
}
