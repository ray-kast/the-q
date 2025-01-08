use std::hash::Hash;

use hashbrown::HashMap;

use super::{Dfa, Node};
use crate::free::{Free, Succ};

pub struct DfaAtomizer<N, A> {
    free: Free<A>,
    used: HashMap<N, A>,
}

impl<N, A: Default> Default for DfaAtomizer<N, A> {
    fn default() -> Self {
        Self {
            free: Free::default(),
            used: HashMap::default(),
        }
    }
}

impl<N: Eq + Hash, A: Copy + Ord + Succ> DfaAtomizer<N, A> {
    fn get(&mut self, node: N) -> A { *self.used.entry(node).or_insert_with(|| self.free.fresh()) }

    pub fn atomize_nodes<I: Ord, T>(mut self, dfa: Dfa<I, N, T>) -> (Dfa<I, A, T>, HashMap<N, A>) {
        let Dfa { states, start } = dfa;

        (
            Dfa {
                states: states
                    .into_iter()
                    .map(|(n, Node(e, a))| {
                        (
                            self.get(n),
                            Node(e.into_iter().map(|(k, n)| (k, self.get(n))).collect(), a),
                        )
                    })
                    .collect(),
                start: self.get(start),
            },
            self.used,
        )
    }
}
