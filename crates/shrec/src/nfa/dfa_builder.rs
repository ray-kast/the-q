use std::{
    borrow::BorrowMut,
    collections::{BTreeMap, BTreeSet, VecDeque},
    hash::Hash,
    sync::Arc,
};

use hashbrown::HashMap;

use super::Nfa;
use crate::{
    closure_builder::ClosureBuilder, dfa::Dfa, memoize::Memoize, nfa::Node,
    partition_map::PartitionMap,
};

// Note to future self: Don't attempt to convert the value to an Arc, it needs to
//                      be mutably borrowed.
struct State<I, N, E, T>(
    PartitionMap<I, BTreeSet<(Option<E>, N)>>,
    Option<Arc<BTreeSet<T>>>,
);

impl<I, N, E, T> Default for State<I, N, E, T> {
    fn default() -> Self { Self(PartitionMap::new(BTreeSet::new()), None) }
}

pub type Output<I, N, E, T> = Dfa<I, Arc<BTreeSet<N>>, Arc<BTreeSet<E>>, Arc<BTreeSet<T>>>;

pub struct DfaBuilder<'a, I, N, E, T> {
    nfa: &'a Nfa<I, N, E, T>,
    delta_closure: ClosureBuilder<(Option<E>, N)>,
}

impl<'a, I: Clone + Ord, N: Clone + Ord + Hash, E: Clone + Ord + Hash, T: Clone + Ord + Hash>
    DfaBuilder<'a, I, N, E, T>
{
    pub fn new(nfa: &'a Nfa<I, N, E, T>) -> Self {
        Self {
            nfa,
            delta_closure: ClosureBuilder::default(),
        }
    }

    fn solve_delta<S: BorrowMut<BTreeSet<(Option<E>, N)>>>(&mut self, set: S) -> S {
        self.delta_closure.solve(set, |(_, n)| {
            self.nfa
                .get(&n)
                .into_iter()
                .flat_map(super::Node::nil_edges)
                .cloned()
        })
    }

    #[inline]
    pub fn build(&mut self) -> Output<I, N, E, T> {
        let mut memo_node = Memoize::default();
        let mut memo_edge = Memoize::default();
        let mut memo_tok = Memoize::default();
        self.delta_closure.init([(None, self.nfa.start().clone())]);
        let start = memo_node.memoize(
            self.solve_delta(BTreeSet::new())
                .into_iter()
                .map(|(_, n)| n)
                .collect(),
        );
        let trap = memo_node.memoize(BTreeSet::new());

        let mut states: BTreeMap<Arc<BTreeSet<N>>, State<I, N, E, T>> = BTreeMap::default();
        let mut states_by_delta = HashMap::new();
        let mut q: VecDeque<_> = [Arc::clone(&start)].into_iter().collect();

        while let Some(state_set) = q.pop_front() {
            use std::collections::btree_map::Entry;

            // TODO: insert_entry pls
            let Entry::Vacant(v) = states.entry(Arc::clone(&state_set)) else {
                continue;
            };
            let node = v.insert(State::default());

            for state in state_set.iter() {
                for (inp, nodes) in self
                    .nfa
                    .get(state)
                    .into_iter()
                    .flat_map(Node::edges)
                    .filter_map(|(i, n)| i.map(|i| (i, n)))
                {
                    let mut states = BTreeSet::new();

                    self.delta_closure.init(nodes.iter().cloned());
                    self.solve_delta(&mut states);
                    node.0.update(inp.to_owned().bounds(), |_, v| {
                        states.union(v).cloned().collect()
                    });
                }
            }

            let toks: BTreeSet<_> = state_set
                .iter()
                .filter_map(|n| self.nfa.nodes.get(n).and_then(|n| n.accept.clone()))
                .collect();

            if !toks.is_empty() {
                assert!(node.1.replace(memo_tok.memoize(toks)).is_none());
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
                            memo_node.memoize(delta.iter().map(|(_, n)| n.clone()).collect()),
                        )
                    });

                // Try our very hardest to avoid cloning the set again
                if !states.contains_key(set) {
                    q.push_back(Arc::clone(set));
                }
            }
        }

        // TODO: check for unnecessary memory allocations
        Dfa::new(
            states.into_iter().map(|(n, State(e, k))| {
                (
                    n,
                    e.into_partitions()
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
                                (
                                    (!edges.is_empty()).then(|| memo_edge.memoize(edges)),
                                    memo_node.memoize(node),
                                ),
                            )
                        })
                        .collect(),
                    k,
                )
            }),
            start,
            trap,
        )
    }
}
