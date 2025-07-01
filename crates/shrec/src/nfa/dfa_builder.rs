use std::{
    borrow::BorrowMut,
    collections::{BTreeMap, BTreeSet, VecDeque},
    hash::Hash,
    mem,
    sync::Arc,
};

use hashbrown::HashMap;

use super::{Nfa, NFA_START};
use crate::{
    autom::Accept,
    closure_builder::ClosureBuilder,
    dfa::{self, collect_state_keys, collect_states, Dfa},
    memoize::Memoize,
    partition_map::PartitionMap,
    range_map::RangeMap,
};

type Delta<E> = (Option<<E as Accept>::Token>, usize);

// NOTE to future self: Don't attempt to convert the value to an Arc, it needs to
//                      be mutably borrowed.
struct State<I, T: Accept, E: Accept>(PartitionMap<I, BTreeSet<Delta<E>>>, Arc<BTreeSet<T::Token>>);

impl<I, T: Accept, E: Accept> State<I, T, E> {
    fn new(accept: Arc<BTreeSet<T::Token>>) -> Self {
        Self(PartitionMap::new(BTreeSet::new()), accept)
    }
}

pub type Output<I, T, E> =
    Dfa<I, Arc<BTreeSet<<T as Accept>::Token>>, Arc<BTreeSet<<E as Accept>::Token>>>;

pub struct DfaBuilder<'a, I, T: Accept, E: Accept> {
    nfa: &'a Nfa<I, T, E>,
    delta_closure: ClosureBuilder<Delta<E>>,
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

    fn solve_delta<S: BorrowMut<BTreeSet<Delta<E>>>>(&mut self, set: S) -> S {
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
        let mut memo_node: Memoize<Arc<BTreeSet<usize>>> = Memoize::default();
        let mut memo_node_tok: Memoize<Arc<BTreeSet<T::Token>>> = Memoize::default();
        let mut memo_edge_tok: Memoize<Arc<BTreeSet<E::Token>>> = Memoize::default();
        let no_node_tok = memo_node_tok.memoize(BTreeSet::new());
        self.delta_closure.init([(None, NFA_START)]);
        let start = memo_node.memoize(
            self.solve_delta(BTreeSet::new())
                .into_iter()
                .map(|(_, n)| n)
                .collect(),
        );

        if start.is_empty() {
            return Dfa::new(vec![dfa::State(RangeMap::new(), no_node_tok)]);
        }

        let mut states: BTreeMap<Arc<BTreeSet<usize>>, State<I, T, E>> = BTreeMap::default();
        let mut states_by_delta = HashMap::new();
        let mut q: VecDeque<_> = [Arc::clone(&start)].into_iter().collect();

        while let Some(state_set) = q.pop_front() {
            use std::collections::btree_map::Entry;

            debug_assert!(!state_set.is_empty());

            // TODO: insert_entry pls
            let Entry::Vacant(v) = states.entry(Arc::clone(&state_set)) else {
                continue;
            };
            let node = v.insert(State::new(Arc::clone(&no_node_tok)));

            for &state in state_set.iter() {
                for (inp, deltas) in self
                    .nfa
                    .get(state)
                    .unwrap()
                    .edges()
                    .filter_map(|(i, n)| i.map(|i| (i, n)))
                {
                    let mut states = BTreeSet::new();

                    self.delta_closure
                        .init(deltas.iter().map(|&(ref e, n)| (e.as_token().cloned(), n)));
                    self.solve_delta(&mut states);
                    node.0.update(inp.to_owned().bounds(), |_, v| {
                        states.union(v).cloned().collect()
                    });
                }
            }

            let toks: BTreeSet<_> = state_set
                .iter()
                .filter_map(|&n| self.nfa.0.get(n).unwrap().accept.as_token().cloned())
                .collect();

            if !toks.is_empty() {
                assert!(mem::replace(&mut node.1, memo_node_tok.memoize(toks)).is_empty());
            }

            // Drop the mutable borrow created by calling BTreeMap::entry
            let node = states.get(&state_set).unwrap_or_else(|| unreachable!());

            for delta in node.0.values() {
                let (_, set) = states_by_delta
                    .raw_entry_mut()
                    .from_key(delta)
                    .or_insert_with(|| {
                        (
                            delta.clone(),
                            memo_node.memoize(delta.iter().map(|&(_, n)| n).collect()),
                        )
                    });

                // Try our very hardest to avoid cloning the set again
                if !(set.is_empty() || states.contains_key(set)) {
                    q.push_back(Arc::clone(set));
                }
            }
        }

        let state_ids = collect_state_keys(states.keys().cloned(), &start);

        // TODO: check for unnecessary memory allocations
        Dfa::new(collect_states(
            &state_ids,
            states.into_iter().map(|(s, State(e, k))| {
                (
                    s,
                    dfa::State(
                        e.into_partitions()
                            .filter(|(_, v)| !v.is_empty())
                            .map(|(k, v)| {
                                let (edges, node) = v.into_iter().fold(
                                    (BTreeSet::new(), BTreeSet::new()),
                                    |(mut es, mut ns), (e, n)| {
                                        es.extend(e);
                                        ns.insert(n);
                                        (es, ns)
                                    },
                                );
                                (
                                    k,
                                    (memo_edge_tok.memoize(edges), *state_ids.get(&node).unwrap()),
                                )
                            })
                            .collect(),
                        k,
                    ),
                )
            }),
        ))
    }
}
