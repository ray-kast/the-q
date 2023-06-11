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

    pub fn atomize_nodes<I: Ord, E, T>(
        mut self,
        dfa: Dfa<I, N, E, T>,
    ) -> (Dfa<I, A, E, T>, HashMap<N, A>) {
        let Dfa {
            states,
            start,
            accept,
        } = dfa;

        (
            Dfa {
                states: states
                    .into_iter()
                    .map(|(k, Node(v))| {
                        (
                            self.get(k),
                            Node(
                                v.into_iter()
                                    .map(|(k, (n, e))| (k, (self.get(n), e)))
                                    .collect(),
                            ),
                        )
                    })
                    .collect(),
                start: self.get(start),
                accept: accept.into_iter().map(|(a, t)| (self.get(a), t)).collect(),
            },
            self.used,
        )
    }
}
