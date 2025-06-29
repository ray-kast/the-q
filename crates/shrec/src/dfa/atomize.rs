use std::hash::Hash;

use hashbrown::HashMap;

use super::{Dfa, Node};
use crate::{
    free::{Free, Succ},
    partition_map::PartitionMap,
};

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

    pub fn atomize_nodes<I: Clone + Ord, E: Clone + Eq, T>(
        mut self,
        dfa: Dfa<I, N, E, T>,
    ) -> (Dfa<I, A, E, T>, HashMap<N, A>) {
        let Dfa {
            states,
            start,
            trap,
        } = dfa;

        let trap = self.get(trap);

        (
            Dfa {
                states: states
                    .into_iter()
                    .map(|(n, Node(e, k))| {
                        (
                            self.get(n),
                            Node(
                                PartitionMap::from_iter_with_default(
                                    e.into_partitions().map(|(k, (e, n))| (k, (e, self.get(n)))),
                                    (None, trap),
                                ),
                                k,
                            ),
                        )
                    })
                    .collect(),
                start: self.get(start),
                trap,
            },
            self.used,
        )
    }
}
