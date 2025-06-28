use std::{
    borrow::BorrowMut,
    collections::{BTreeMap, BTreeSet, VecDeque},
    hash::Hash,
    sync::Arc,
};

use super::Nfa;
use crate::{
    closure_builder::ClosureBuilder, dfa::Dfa, memoize::Memoize, nfa::Node,
    partition_map::PartitionMap,
};

// Note to future self: Don't attempt to convert the value to an Arc, it needs to
//                      be mutably borrowed.
struct State<I, N, T>(PartitionMap<I, BTreeSet<N>>, Option<Arc<BTreeSet<T>>>);

impl<I, N, T> Default for State<I, N, T> {
    fn default() -> Self { Self(PartitionMap::new(BTreeSet::new()), None) }
}

pub struct DfaBuilder<'a, I, N, T> {
    nfa: &'a Nfa<I, N, T>,
    closure: ClosureBuilder<N>,
}

impl<'a, I: Clone + Ord, N: Clone + Ord + Hash, T: Clone + Ord + Hash> DfaBuilder<'a, I, N, T> {
    pub fn new(nfa: &'a Nfa<I, N, T>) -> Self {
        Self {
            nfa,
            closure: ClosureBuilder::default(),
        }
    }

    fn solve_closure<S: BorrowMut<BTreeSet<N>>>(&mut self, set: S) -> S {
        self.closure.solve(set, |n| {
            self.nfa
                .get(&n)
                .into_iter()
                .flat_map(super::Node::nil_edges)
                .cloned()
        })
    }

    #[inline]
    pub fn build(&mut self) -> Dfa<I, Arc<BTreeSet<N>>, Arc<BTreeSet<T>>> {
        let mut memo_node = Memoize::default();
        let mut memo_tok = Memoize::default();
        self.closure.init([self.nfa.start().clone()]);
        let start = memo_node.memoize(self.solve_closure(BTreeSet::new()));
        let trap = memo_node.memoize(BTreeSet::new());

        let mut states: BTreeMap<Arc<BTreeSet<N>>, State<I, N, T>> = BTreeMap::default();
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

                    self.closure.init(nodes.iter().cloned());
                    self.solve_closure(&mut states);
                    node.0.update(inp.to_owned().bounds(), |_, v| {
                        states.union(v).cloned().collect()
                    });
                }
            }

            let toks: BTreeSet<_> = self
                .nfa
                .nodes
                .iter()
                .filter_map(|(n, s)| state_set.contains(n).then_some(s.accept.clone()).flatten())
                .collect();
            if !toks.is_empty() {
                assert!(node.1.replace(memo_tok.memoize(toks)).is_none());
            }

            // Drop the mutable borrow created by calling BTreeMap::entry
            let node = states.get(&state_set).unwrap_or_else(|| unreachable!());

            for set in node.0.values() {
                // Try our very hardest to avoid cloning the set again
                if !states.contains_key(set) {
                    q.push_back(memo_node.memoize_ref(set));
                }
            }
        }

        // TODO: check for unnecessary memory allocations
        Dfa::new(
            states.into_iter().map(|(n, State(e, k))| {
                (
                    n,
                    e.into_partitions()
                        .map(|(k, v)| (k, memo_node.memoize(v)))
                        .collect(),
                    k,
                )
            }),
            start,
            trap,
        )
    }
}
