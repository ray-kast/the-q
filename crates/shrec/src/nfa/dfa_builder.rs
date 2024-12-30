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

pub struct DfaBuilder<'a, I, N, T> {
    nfa: &'a Nfa<I, N, (), T>,
    closure: ClosureBuilder<&'a N>,
}

impl<'a, I: Ord, N: Ord + Hash, T: Ord + Hash> DfaBuilder<'a, I, N, T> {
    pub fn new(nfa: &'a Nfa<I, N, (), T>) -> Self {
        Self {
            nfa,
            closure: ClosureBuilder::default(),
        }
    }

    fn solve_closure<S: BorrowMut<BTreeSet<&'a N>>>(&mut self, set: S) -> S {
        self.closure.solve(set, |n| {
            #[expect(
                clippy::zero_sized_map_values,
                reason = "Nfa with unit edge type necessarily creates a BTreeMap representing a \
                          set"
            )]
            self.nfa
                .get(n)
                .into_iter()
                .filter_map(|n| n.get(&None))
                .flat_map(BTreeMap::keys)
        })
    }

    #[inline]
    pub fn build(&mut self) -> Dfa<&'a I, Rc<BTreeSet<&'a N>>, (), Rc<BTreeSet<&'a T>>> {
        let mut memo_node = Memoize::default();
        let mut memo_tok = Memoize::default();
        self.closure.init([self.nfa.start()]);
        let start = memo_node.memoize(self.solve_closure(BTreeSet::new()));

        let mut states: BTreeMap<Rc<BTreeSet<&'a N>>, State<&'a I, &'a N>> = BTreeMap::default();
        let mut accept: BTreeMap<Rc<BTreeSet<&'a N>>, Rc<BTreeSet<&'a T>>> = BTreeMap::default();
        let mut q: VecDeque<_> = [Rc::clone(&start)].into_iter().collect();

        while let Some(state_set) = q.pop_front() {
            use std::collections::btree_map::Entry;

            // TODO: insert_entry pls
            let Entry::Vacant(node) = states.entry(Rc::clone(&state_set)) else {
                continue;
            };
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
                    q.push_back(memo_node.memoize_owned(set));
                }
            }

            let toks: BTreeSet<_> = self
                .nfa
                .accept()
                .iter()
                .filter_map(|(t, a)| state_set.contains(a).then_some(t))
                .collect();
            if !toks.is_empty() {
                accept.insert(Rc::clone(&state_set), memo_tok.memoize(toks));
            }
        }

        // TODO: check for unnecessary memory allocations
        Dfa::new(
            states.into_iter().map(|(k, State(v))| {
                (
                    k,
                    v.into_iter()
                        .map(|(k, v)| (k, (memo_node.memoize(v), ())))
                        .collect(),
                )
            }),
            start,
            accept,
        )
    }
}
