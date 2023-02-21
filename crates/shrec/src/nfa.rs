use std::{
    borrow::Borrow,
    collections::{hash_map, BTreeSet, HashMap},
    hash::Hash,
    rc::Rc,
};

use self::dfa_builder::DfaBuilder;
use crate::dfa::Dfa;

mod dfa_builder;

#[derive(Debug)]
pub struct Node<I, N, E>(HashMap<Option<I>, HashMap<N, E>>);

impl<I, N, E> Default for Node<I, N, E> {
    fn default() -> Self { Self(HashMap::default()) }
}

impl<I: Eq + Hash, N, E> Node<I, N, E> {
    #[inline]
    #[must_use]
    pub fn edges(&self) -> hash_map::Iter<Option<I>, HashMap<N, E>> { self.0.iter() }

    #[inline]
    pub fn get<Q: Eq + Hash + ?Sized>(&self, inp: &Q) -> Option<&HashMap<N, E>>
    where Option<I>: Borrow<Q> {
        self.0.get(inp)
    }
}

#[derive(Debug)]
pub struct Nfa<I, N, E> {
    nodes: HashMap<N, Node<I, N, E>>,
    head: N,
    tail: N,
}

impl<I: Eq + Hash, N: Clone + Eq + Hash, E> Nfa<I, N, E> {
    pub fn new(head: N, tail: N) -> Self {
        let mut me = Self {
            nodes: HashMap::new(),
            head: head.clone(),
            tail: tail.clone(),
        };
        assert!(me.insert(head).is_none());
        assert!(me.insert(tail).is_none());
        me
    }
}

impl<I: Eq + Hash, N: Eq + Hash, E> Nfa<I, N, E> {
    #[inline]
    pub fn head(&self) -> &N { &self.head }

    #[inline]
    pub fn tail(&self) -> &N { &self.tail }

    #[inline]
    pub fn get<Q: Eq + Hash + ?Sized>(&self, node: &Q) -> Option<&Node<I, N, E>>
    where N: Borrow<Q> {
        self.nodes.get(node)
    }

    #[inline]
    pub fn insert(&mut self, node: N) -> Option<Node<I, N, E>> {
        self.nodes.insert(node, Node::default())
    }

    pub fn connect<Q: Eq + Hash + ?Sized>(
        &mut self,
        from: &Q,
        to: N,
        by: Option<I>,
        out: E,
    ) -> Option<E>
    where
        N: Borrow<Q>,
    {
        assert!(self.nodes.contains_key::<N>(&to));
        self.nodes
            .get_mut(from)
            .unwrap()
            .0
            .entry(by)
            .or_default()
            .insert(to, out)
    }
}

impl<I: Eq + Hash, N: Eq + Ord + Hash> Nfa<I, N, ()> {
    pub fn compile(&self) -> Dfa<&I, Rc<BTreeSet<&N>>, ()> { DfaBuilder::new(self).build() }
}
