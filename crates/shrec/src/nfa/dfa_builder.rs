use std::{
    borrow::BorrowMut,
    collections::{BTreeMap, BTreeSet, VecDeque},
    hash::Hash,
    sync::Arc,
};

use super::Nfa;
use crate::{closure_builder::ClosureBuilder, dfa::Dfa, memoize::Memoize, nfa::Node};

// Note to future self: Don't attempt to convert the value to an Arc, it needs to
//                      be mutably borrowed.
struct State<I, N, T>(BTreeMap<I, BTreeSet<N>>, Option<Arc<BTreeSet<T>>>);

impl<I, N, T> Default for State<I, N, T> {
    fn default() -> Self { Self(BTreeMap::new(), None) }
}

pub struct DfaBuilder<'a, I, N, T> {
    nfa: &'a Nfa<I, N, T>,
    closure: ClosureBuilder<N>,
}

impl<'a, I: Copy + Ord, N: Copy + Ord + Hash, T: Clone + Ord + Hash> DfaBuilder<'a, I, N, T> {
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
                .filter_map(|n| n.get(&None))
                .flatten()
                .copied()
        })
    }

    #[inline]
    pub fn build(&mut self) -> Dfa<I, Arc<BTreeSet<N>>, Arc<BTreeSet<T>>> {
        let mut memo_node = Memoize::default();
        let mut memo_tok = Memoize::default();
        self.closure.init([*self.nfa.start()]);
        let start = memo_node.memoize(self.solve_closure(BTreeSet::new()));

        let mut states: BTreeMap<Arc<BTreeSet<N>>, State<I, N, T>> = BTreeMap::default();
        let mut q: VecDeque<_> = [Arc::clone(&start)].into_iter().collect();

        while let Some(state_set) = q.pop_front() {
            use std::collections::btree_map::Entry;

            // TODO: insert_entry pls
            let Entry::Vacant(v) = states.entry(Arc::clone(&state_set)) else {
                continue;
            };
            let node = v.insert(State::default());

            for &state in &*state_set {
                for (inp, nodes) in self
                    .nfa
                    .get(&state)
                    .into_iter()
                    .flat_map(Node::edges)
                    .filter_map(|(i, n)| i.map(|i| (i, n)))
                {
                    let states = node.0.entry(inp).or_default();

                    self.closure.init(nodes.iter().copied());
                    self.solve_closure(states);
                }
            }

            let toks: BTreeSet<_> = self
                .nfa
                .nodes
                .iter()
                .filter_map(|(n, Node(_, a))| state_set.contains(n).then_some(a.clone()).flatten())
                .collect();
            if !toks.is_empty() {
                assert!(node.1.replace(memo_tok.memoize(toks)).is_none());
            }

            // Drop the mutable borrow created by calling BTreeMap::entry
            let node = states.get(&state_set).unwrap_or_else(|| unreachable!());

            for set in node.0.values() {
                // Try our very hardest to avoid cloning the set again
                if !states.contains_key(set) {
                    q.push_back(memo_node.memoize_owned(set));
                }
            }
        }

        // TODO: check for unnecessary memory allocations
        Dfa::new(
            states.into_iter().map(|(n, State(e, a))| {
                (
                    n,
                    e.into_iter()
                        .map(|(k, v)| (k, memo_node.memoize(v)))
                        .collect(),
                    a,
                )
            }),
            start,
        )
    }
}
