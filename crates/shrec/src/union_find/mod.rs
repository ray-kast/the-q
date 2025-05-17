//! A disjoint-set data structure and relevant support types

use std::{cmp::Ordering, fmt, hash, marker::PhantomData};

use vec_forest::VecForestSet;

pub mod disjoint_set;
pub mod linked_arc;
pub mod vec_forest;

#[repr(transparent)]
pub struct ClassId<C: ?Sized = ()>(usize, PhantomData<fn(&C)>);

impl<C: ?Sized> ClassId<C> {
    const fn new(id: usize) -> Self { Self(id, PhantomData) }

    #[must_use]
    pub fn id(self) -> usize { self.0 }
}

impl<C: ?Sized> fmt::Debug for ClassId<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self(id, PhantomData) = self;
        f.debug_tuple("EClass").field(id).finish()
    }
}

impl<C: ?Sized> Clone for ClassId<C> {
    fn clone(&self) -> Self { *self }
}

impl<C: ?Sized> Copy for ClassId<C> {}

impl<C: ?Sized> PartialEq for ClassId<C> {
    fn eq(&self, other: &Self) -> bool {
        let Self(id, PhantomData) = *self;
        id == other.0
    }
}

impl<C: ?Sized> Eq for ClassId<C> {}

impl<C: ?Sized> Ord for ClassId<C> {
    fn cmp(&self, other: &Self) -> Ordering {
        let Self(id, PhantomData) = *self;
        id.cmp(&other.0)
    }
}

impl<C: ?Sized> PartialOrd for ClassId<C> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl<C: ?Sized> hash::Hash for ClassId<C> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
        self.1.hash(state);
    }
}

impl<C: ?Sized> From<ClassId<C>> for usize {
    fn from(value: ClassId<C>) -> Self {
        let ClassId(i, PhantomData) = value;
        i
    }
}

pub type NoNode = disjoint_set::NoNode<usize>;
pub type Unioned<C> = disjoint_set::Unioned<ClassId<C>>;

/// A disjoint-set data structure
pub struct UnionFind<C: ?Sized = ()>(vec_forest::VecForestSet, PhantomData<[ClassId<C>]>);

impl<C: ?Sized> fmt::Debug for UnionFind<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self(nodes, PhantomData) = self;
        fmt::Debug::fmt(nodes, f)
    }
}

impl<C: ?Sized> Default for UnionFind<C> {
    fn default() -> Self { Self(VecForestSet::default(), PhantomData) }
}

impl<C: ?Sized> Clone for UnionFind<C> {
    fn clone(&self) -> Self { Self(self.0.clone(), PhantomData) }
}

impl<C: ?Sized> UnionFind<C> {
    /// Construct a new, empty union-find
    #[must_use]
    #[inline]
    pub fn new() -> Self { Self::default() }

    /// Gets the number of nodes in the union-find
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize { self.0.len() }

    /// Returns true if the union-find has no nodes
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool { self.0.is_empty() }

    pub fn classes(&self) -> Classes<C> { Classes(0..self.0.len(), PhantomData) }

    #[inline]
    pub fn roots(&self) -> Roots<C> { Roots(self.0.roots(), PhantomData) }

    /// Add a new node to the union-find, returning its ID
    #[inline]
    pub fn add(&mut self) -> ClassId<C> { ClassId::new(self.0.add()) }

    /// Find the partition root ID for the given node ID, and optimize the
    /// search path between the node and its root
    ///
    /// # Errors
    /// This method first checks if the node ID is valid, returning an error if
    /// no associated node can be found.
    #[inline]
    pub fn find(&self, key: ClassId<C>) -> Result<ClassId<C>, NoNode> {
        disjoint_set::forest_find(&self.0, key.0).map(ClassId::new)
    }

    /// Perform the in-place union of the partitions containing the two given
    /// node IDs
    ///
    /// # Errors
    /// This method first checks if both node IDs are valid, returning an error
    /// if either cannot be found.
    #[inline]
    pub fn union(&mut self, a: ClassId<C>, b: ClassId<C>) -> Result<Unioned<C>, NoNode> {
        disjoint_set::ranked_union(&mut self.0, a.0, b.0).map(|u| u.map(ClassId::new))
    }
}

#[must_use]
#[repr(transparent)]
pub struct Classes<C: ?Sized>(std::ops::Range<usize>, PhantomData<[ClassId<C>]>);

impl<C: ?Sized> fmt::Debug for Classes<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self(it, PhantomData) = self;
        f.debug_tuple("Classes").field(it).finish()
    }
}

impl<C: ?Sized> Iterator for Classes<C> {
    type Item = ClassId<C>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> { self.0.next().map(ClassId::new) }
}

#[must_use]
#[repr(transparent)]
pub struct Roots<'a, C: ?Sized>(vec_forest::Roots<'a>, PhantomData<&'a [ClassId<C>]>);

impl<C: ?Sized> fmt::Debug for Roots<'_, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self(it, PhantomData) = self;
        fmt::Debug::fmt(it, f)
    }
}

impl<C: ?Sized> Clone for Roots<'_, C> {
    fn clone(&self) -> Self { Self(self.0.clone(), PhantomData) }
}

impl<C: ?Sized> Iterator for Roots<'_, C> {
    type Item = ClassId<C>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> { self.0.next().map(ClassId::new) }
}

#[cfg(test)]
mod test {
    use prop::sample::SizeRange;
    use proptest::prelude::*;

    use super::{disjoint_set, linked_arc};
    use crate::free::Free;

    type Node = linked_arc::LinkedNode<Linked>;
    #[derive(Debug)]
    struct Linked(Node);

    impl linked_arc::AsNode for Linked {
        type Extra = linked_arc::CircularList<Linked>;
        type Rank = usize;

        #[inline]
        fn as_node(&self) -> &Node { &self.0 }
    }

    fn run_diff(len: usize, merges: &[(usize, usize)]) {
        let mut linked_ids = Free::default();
        let mut linked = vec![];
        let mut vec = super::UnionFind::<()>::new();

        for _ in 0..len {
            linked.push(Node::new_fresh(&mut linked_ids, Linked));
            assert_eq!(vec.add().id(), *linked.last().unwrap().0.root().id());
        }

        for (a, b) in merges.iter().copied() {
            let vec_u = vec
                .union(super::ClassId::new(a), super::ClassId::new(b))
                .unwrap();

            let link_u =
                disjoint_set::ranked_union(&mut linked_arc::LinkedArc, &linked[a], &linked[b])
                    .unwrap();

            assert_eq!(vec_u.map(super::ClassId::id), link_u.map(|n| *n.id()));
        }

        for (i, linked) in linked.into_iter().enumerate() {
            assert_eq!(
                vec.clone().find(super::ClassId::new(i)).unwrap().id(),
                *linked.0.root().id(),
            );
        }
    }

    fn len_and_merges(
        len: impl Strategy<Value = usize>,
        merge_len: impl Clone + Into<SizeRange>,
    ) -> impl Strategy<Value = (usize, Vec<(usize, usize)>)> {
        len.prop_flat_map(move |l| {
            prop::collection::vec((0..l, 0..l), merge_len.clone()).prop_map(move |v| (l, v))
        })
    }

    proptest! {
        // #![proptest_config(ProptestConfig {
        //     cases: 2 << 16,
        //     ..ProptestConfig::default()
        // })]

        #[test]
        fn differential((len, merges) in len_and_merges(64..=2048_usize, 1..=128)) {
            run_diff(len, &merges);
        }
    }
}
