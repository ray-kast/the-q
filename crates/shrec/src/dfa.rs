use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

#[derive(Debug)]
#[repr(transparent)]
pub struct Node<I, N, E>(HashMap<I, (N, E)>);

#[derive(Debug)]
pub struct Dfa<I, N, E> {
    states: HashMap<N, Node<I, N, E>>,
    start: N,
    accept: HashSet<N>,
}

impl<I, N: Eq + Hash, E> Dfa<I, N, E> {
    pub fn new(
        states: impl IntoIterator<Item = (N, HashMap<I, (N, E)>)>,
        start: N,
        accept: HashSet<N>,
    ) -> Self {
        Self {
            states: states.into_iter().map(|(k, v)| (k, Node(v))).collect(),
            start,
            accept,
        }
    }
}
