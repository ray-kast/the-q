use std::{
    collections::{BTreeMap, BTreeSet},
    hash::Hash,
    mem,
};

use hashbrown::HashMap;

use super::Dfa;
use crate::{
    dfa::Node,
    egraph::{EGraph, ENode},
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Op<I, N, T> {
    Node {
        accept: Option<T>,
        edges: BTreeSet<I>,
    },
    Impostor(N),
}

pub type Graph<I, N, T> = EGraph<Op<I, N, T>, N>;

pub(super) fn run<I: Copy + Ord + Hash, N: Copy + Ord + Hash, T: Clone + Ord + Hash>(
    dfa: &Dfa<I, N, T>,
) -> (Dfa<I, usize, T>, Graph<I, N, T>) {
    enum Command<N> {
        Explore(N),
        Add(N),
    }

    let mut eg = EGraph::<Op<I, N, T>, N>::new();
    let mut stk = Vec::new();
    let mut classes = HashMap::new();
    let mut impostors = HashMap::new();

    stk.push(Command::Explore(dfa.start));

    while let Some(node) = stk.pop() {
        match node {
            Command::Explore(n) => {
                use hashbrown::hash_map::Entry;

                match classes.entry(n) {
                    Entry::Occupied(_) => continue,
                    Entry::Vacant(v) => drop(v.insert(None)),
                }

                stk.push(Command::Add(n));
                for &n in dfa.states[&n].0.values().rev() {
                    if classes.get(&n).is_none() {
                        stk.push(Command::Explore(n));
                    }
                }
            },
            Command::Add(n) => {
                let Node(ref edges, ref accept) = dfa.states[&n];
                let enode = ENode::new(
                    Op::Node {
                        accept: accept.clone(),
                        edges: edges.keys().copied().collect(),
                    }
                    .into(),
                    edges
                        .values()
                        .map(|&n| {
                            classes[&n].unwrap_or_else(|| {
                                *impostors.entry(n).or_insert_with(|| {
                                    eg.add(ENode::new(Op::Impostor(n).into(), [].into()))
                                        .unwrap()
                                })
                            })
                        })
                        .collect(),
                );

                let klass = eg.add(enode.clone()).unwrap();
                assert!(mem::replace(classes.get_mut(&n).unwrap(), Some(klass)).is_none());
            },
        }
    }

    let mut wr = eg.write();
    for (node, klass) in impostors {
        wr.merge(classes[&node].unwrap(), klass).unwrap();
    }
    drop(wr);

    let states = eg
        .class_nodes()
        .into_iter()
        .map(|(k, v)| (k, v.into_iter().collect()))
        .collect::<BTreeMap<_, BTreeSet<_>>>()
        .into_iter()
        .map(|(c, mut n)| {
            n.retain(|n| !matches!(n.op(), Op::Impostor(_)));
            assert!(n.len() == 1);

            let node = &n.into_iter().next().unwrap();
            let Op::Node { accept, edges } = node.op() else {
                unreachable!();
            };
            let args = node.args();

            (
                c.id(),
                edges
                    .iter()
                    .enumerate()
                    .map(|(i, &e)| (e, args[i].id()))
                    .collect(),
                accept.clone(),
            )
        });

    (Dfa::new(states, classes[&dfa.start].unwrap().id()), eg)
}
