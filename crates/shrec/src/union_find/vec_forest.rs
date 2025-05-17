use std::{
    fmt, iter, slice,
    sync::atomic::{self, AtomicUsize},
};

use super::disjoint_set::{forest_find, ForestFind, NoNode, RankedUnion};

#[derive(Debug)]
pub(super) struct Node {
    pub parent: AtomicUsize,
    pub rank: usize,
}

impl Clone for Node {
    fn clone(&self) -> Self {
        Self {
            parent: self.parent.load(atomic::Ordering::Relaxed).into(),
            rank: self.rank,
        }
    }
}

#[derive(Default, Clone)]
#[repr(transparent)]
pub struct VecForestSet(pub(super) Vec<Node>);

impl fmt::Debug for VecForestSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self(nodes) = self;
        let mut f = f.debug_map();

        for (klass, node) in nodes.iter().enumerate() {
            let mut root = node.parent.load(atomic::Ordering::Relaxed);
            loop {
                let par = nodes.get(root).unwrap_or_else(|| unreachable!());
                let gpar = par.parent.load(atomic::Ordering::Relaxed);
                if gpar == root {
                    break;
                }

                root = gpar;
            }

            f.entry(&klass, &(root != klass).then_some(root));
        }

        f.finish()
    }
}

impl VecForestSet {
    #[must_use]
    #[inline]
    pub fn new() -> Self { Self::default() }

    #[must_use]
    #[inline]
    pub fn len(&self) -> usize { self.0.len() }

    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool { self.0.is_empty() }

    pub fn add(&mut self) -> usize {
        let key = self.0.len();
        self.0.push(Node {
            parent: key.into(),
            rank: 1,
        });
        key
    }

    #[inline]
    pub fn roots(&self) -> Roots { Roots(self.0.iter().enumerate()) }
}

impl ForestFind<usize> for VecForestSet {
    fn load_parent(&self, key: usize) -> Option<usize> {
        Some(self.0.get(key)?.parent.load(atomic::Ordering::SeqCst))
    }

    fn compare_exchange_parent(
        &self,
        key: usize,
        current: usize,
        new: usize,
    ) -> Option<Result<usize, usize>> {
        Some(self.0.get(key)?.parent.compare_exchange(
            current,
            new,
            atomic::Ordering::SeqCst,
            atomic::Ordering::SeqCst,
        ))
    }
}

impl RankedUnion<usize> for VecForestSet {
    type Rank = usize;
    type Root = usize;

    #[inline]
    fn find(&self, key: usize) -> Result<usize, NoNode<usize>> { forest_find(self, key) }

    #[inline]
    fn cmp_roots(&self, &a: &usize, &b: &usize) -> std::cmp::Ordering { a.cmp(&b) }

    #[inline]
    fn rank(&self, &key: &usize) -> Option<Self::Rank> { Some(self.0.get(key)?.rank) }

    #[inline]
    fn merge(&mut self, &root: &usize, &merged: &usize) {
        let merged_rank = self.0.get(merged).unwrap_or_else(|| unreachable!()).rank;
        let node = self.0.get_mut(root).unwrap_or_else(|| unreachable!());
        node.rank = node
            .rank
            .checked_add(merged_rank)
            .unwrap_or_else(|| unreachable!());
        self.0
            .get_mut(merged)
            .unwrap_or_else(|| unreachable!())
            .parent = root.into();
    }
}

#[derive(Debug, Clone)]
#[must_use]
#[repr(transparent)]
pub struct Roots<'a>(iter::Enumerate<slice::Iter<'a, Node>>);

impl Iterator for Roots<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        loop {
            let (id, node) = self.0.next()?;

            let parent = node.parent.load(atomic::Ordering::Relaxed);
            if parent == id {
                break Some(parent);
            }
        }
    }
}

#[cfg(test)]
#[allow(dead_code)]
fn assert_impls() {
    use crate::union_find::disjoint_set;

    fn set() -> VecForestSet { unreachable!() }

    fn key() -> usize { unreachable!() }

    disjoint_set::forest_find(&set(), key()).unwrap();
    disjoint_set::ranked_union(&mut set(), key(), key()).unwrap();
}
