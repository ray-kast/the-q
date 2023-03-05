use std::{
    borrow::BorrowMut,
    collections::{BTreeMap, BTreeSet, VecDeque},
    hash::Hash,
    rc::Rc,
};

use super::Nfa;
use crate::{closure_builder::ClosureBuilder, dfa::Dfa, memoize::Memoize, nfa::Node};

// Note to future self: Don't attempt to convert the value to an Rc, it needs to
//                      be mutably borrowed.
struct State<I, N>(BTreeMap<I, BTreeSet<N>>);

impl<I, N> Default for State<I, N> {
    fn default() -> Self { Self(BTreeMap::new()) }
}

pub struct DfaBuilder<'a, I, N> {
    nfa: &'a Nfa<I, N, ()>,
    closure: ClosureBuilder<&'a N>,
}

impl<'a, I: Ord, N: Ord + Hash> DfaBuilder<'a, I, N> {
    pub fn new(nfa: &'a Nfa<I, N, ()>) -> Self {
        Self {
            nfa,
            closure: ClosureBuilder::default(),
        }
    }

    fn solve_closure<S: BorrowMut<BTreeSet<&'a N>>>(&mut self, set: S) -> S {
        self.closure.solve(set, |n| {
            #[allow(clippy::zero_sized_map_values)]
            self.nfa
                .get(n)
                .into_iter()
                .filter_map(|n| n.get(&None))
                .flat_map(BTreeMap::keys)
        })
    }

    #[inline]
    pub fn build(&mut self) -> Dfa<&'a I, Rc<BTreeSet<&'a N>>, ()> {
        let mut memo = Memoize::default();
        self.closure.init([self.nfa.head()]);
        let head = memo.memoize(self.solve_closure(BTreeSet::new()));

        let mut states: BTreeMap<Rc<BTreeSet<&'a N>>, State<&'a I, &'a N>> = BTreeMap::default();
        let mut accept: BTreeSet<Rc<BTreeSet<&'a N>>> = BTreeSet::default();
        let mut q: VecDeque<_> = [Rc::clone(&head)].into_iter().collect();

        while let Some(state_set) = q.pop_front() {
            use std::collections::btree_map::Entry;

            // TODO: insert_entry pls
            let Entry::Vacant(node) = states.entry(Rc::clone(&state_set)) else { continue };
            let node = node.insert(State::default());

            // TODO: e-class analysis
            for &state in &*state_set {
                for (inp, nodes) in self
                    .nfa
                    .get(state)
                    .into_iter()
                    .flat_map(Node::edges)
                    .filter_map(|(i, n)| i.as_ref().map(|i| (i, n)))
                {
                    let states = node.0.entry(inp).or_default();

                    self.closure.init(nodes.keys());
                    self.solve_closure(states);
                }
            }

            // Drop the mutable borrow created by calling BTreeMap::entry
            let node = states.get(&state_set).unwrap_or_else(|| unreachable!());

            for set in node.0.values() {
                // Try our very hardest to avoid cloning the set again
                if !states.contains_key(set) {
                    q.push_back(memo.memoize_owned(set));
                }
            }

            if state_set.contains(self.nfa.tail()) {
                accept.insert(Rc::clone(&state_set));
            }
        }

        // TODO: check for unnecessary memory allocations
        Dfa::new(
            states.into_iter().map(|(k, State(v))| {
                (
                    k,
                    v.into_iter()
                        .map(|(k, v)| (k, (memo.memoize(v), ())))
                        .collect(),
                )
            }),
            head,
            accept,
        )
    }
}
