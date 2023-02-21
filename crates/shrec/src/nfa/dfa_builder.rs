use std::{
    borrow::BorrowMut,
    collections::{BTreeSet, HashMap, HashSet, VecDeque},
    hash::Hash,
    rc::Rc,
};

use super::Nfa;
use crate::{dfa::Dfa, nfa::Node};

struct ClosureBuilder<T>(VecDeque<T>);

impl<T> Default for ClosureBuilder<T> {
    #[inline]
    fn default() -> Self { Self(VecDeque::new()) }
}

impl<T> ClosureBuilder<T> {
    #[inline]
    fn init<I: IntoIterator<Item = T>>(&mut self, it: I) {
        assert!(self.0.is_empty());
        self.extend(it);
    }
}

impl<T: Clone + Eq + Hash> ClosureBuilder<T> {
    fn solve_hash<S: BorrowMut<HashSet<T>>, I: IntoIterator<Item = T>>(
        &mut self,
        mut set: S,
        f: impl Fn(T) -> I,
    ) -> S {
        {
            let set = set.borrow_mut();

            while let Some(el) = self.0.pop_front() {
                if set.insert(el.clone()) {
                    self.0.extend(f(el));
                }
            }
        }

        set
    }
}

impl<T: Clone + Eq + Ord> ClosureBuilder<T> {
    fn solve_btree<S: BorrowMut<BTreeSet<T>>, I: IntoIterator<Item = T>>(
        &mut self,
        mut set: S,
        f: impl Fn(T) -> I,
    ) -> S {
        {
            let set = set.borrow_mut();

            while let Some(el) = self.0.pop_front() {
                if set.insert(el.clone()) {
                    self.0.extend(f(el));
                }
            }
        }

        set
    }
}

impl<T> Extend<T> for ClosureBuilder<T> {
    #[inline]
    fn extend<I: IntoIterator<Item = T>>(&mut self, it: I) { self.0.extend(it); }
}

struct State<I, N>(HashMap<I, BTreeSet<N>>);

impl<I, N> Default for State<I, N> {
    fn default() -> Self { Self(HashMap::new()) }
}

pub struct DfaBuilder<'a, I, N> {
    nfa: &'a Nfa<I, N, ()>,
    closure: ClosureBuilder<&'a N>,
}

impl<'a, I: Eq + Hash, N: Eq + Ord + Hash> DfaBuilder<'a, I, N> {
    pub fn new(nfa: &'a Nfa<I, N, ()>) -> Self {
        Self {
            nfa,
            closure: ClosureBuilder::default(),
        }
    }

    fn solve_closure<S: BorrowMut<BTreeSet<&'a N>>>(&mut self, set: S) -> S {
        self.closure.solve_btree(set, |n| {
            self.nfa
                .get(n)
                .into_iter()
                .filter_map(|n| n.get(&None))
                .flat_map(HashMap::keys)
        })
    }

    #[inline]
    pub fn build(&mut self) -> Dfa<&'a I, Rc<BTreeSet<&'a N>>, ()> {
        // TODO: use union-find for epsilon closure?
        self.closure.init([self.nfa.head()]);
        let head = Rc::new(self.solve_closure(BTreeSet::new()));

        let mut states: HashMap<Rc<BTreeSet<&'a N>>, State<&'a I, &'a N>> = HashMap::default();
        let mut accept: HashSet<Rc<BTreeSet<&'a N>>> = HashSet::default();
        let mut q: VecDeque<_> = [Rc::clone(&head)].into_iter().collect();

        while let Some(state_set) = q.pop_front() {
            use std::collections::hash_map::Entry;

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

            // Drop the mutable borrow created by calling HashMap::entry
            let node = states.get(&state_set).unwrap();

            for set in node.0.values() {
                // Try our very hardest to avoid cloning the set again
                if !states.contains_key(set) {
                    q.push_back(Rc::new(set.clone()));
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
                    v.into_iter().map(|(k, v)| (k, (Rc::new(v), ()))).collect(),
                )
            }),
            head,
            accept,
        )
    }
}
