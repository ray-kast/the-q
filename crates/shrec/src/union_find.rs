//! A disjoint-set data structure and relevant support types

use core::slice;
use std::{
    cmp::Ordering,
    fmt, hash, iter,
    marker::PhantomData,
    sync::atomic::{self, AtomicUsize},
};

#[repr(transparent)]
pub struct ClassId<C = ()>(usize, PhantomData<fn(&C)>);

impl<C> ClassId<C> {
    const fn new(id: usize) -> Self { Self(id, PhantomData) }

    #[must_use]
    pub fn id(self) -> usize { self.0 }
}

impl<C> fmt::Debug for ClassId<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self(id, PhantomData) = self;
        f.debug_tuple("EClass").field(id).finish()
    }
}

impl<C> Clone for ClassId<C> {
    fn clone(&self) -> Self { *self }
}

impl<C> Copy for ClassId<C> {}

impl<C> PartialEq for ClassId<C> {
    fn eq(&self, other: &Self) -> bool {
        let Self(id, PhantomData) = *self;
        id == other.0
    }
}

impl<C> Eq for ClassId<C> {}

impl<C> Ord for ClassId<C> {
    fn cmp(&self, other: &Self) -> Ordering {
        let Self(id, PhantomData) = *self;
        id.cmp(&other.0)
    }
}

impl<C> PartialOrd for ClassId<C> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl<C> hash::Hash for ClassId<C> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
        self.1.hash(state);
    }
}

impl<C> From<ClassId<C>> for usize {
    fn from(value: ClassId<C>) -> Self {
        let ClassId(i, PhantomData) = value;
        i
    }
}

/// Error indicating a node ID passed to a [`UnionFind`] operation does not
/// exist.
#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("No disjoint-set node found with ID {0}")]
pub struct NoNode(usize);

pub struct Union<C> {
    pub root: ClassId<C>,
    pub unioned: Option<ClassId<C>>,
}

impl<C> Union<C> {
    #[inline]
    #[must_use]
    pub fn did_merge(self) -> bool { self.unioned.is_some() }
}

impl<C> fmt::Debug for Union<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { root, unioned } = self;
        f.debug_struct("Union")
            .field("root", root)
            .field("unioned", unioned)
            .finish()
    }
}

impl<C> Clone for Union<C> {
    fn clone(&self) -> Self { *self }
}

impl<C> Copy for Union<C> {}

#[derive(Debug)]
struct UnionFindNode {
    parent: AtomicUsize,
    rank: usize,
}

impl Clone for UnionFindNode {
    fn clone(&self) -> Self {
        Self {
            parent: self.parent.load(atomic::Ordering::Relaxed).into(),
            rank: self.rank,
        }
    }
}

/// A disjoint-set data structure
pub struct UnionFind<C = ()>(Vec<UnionFindNode>, PhantomData<[ClassId<C>]>);

impl<C> fmt::Debug for UnionFind<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self(nodes, PhantomData) = self;
        f.debug_tuple("UnionFind").field(nodes).finish()
    }
}

impl<C> Default for UnionFind<C> {
    fn default() -> Self { Self(Vec::new(), PhantomData) }
}

impl<C> Clone for UnionFind<C> {
    fn clone(&self) -> Self { Self(self.0.clone(), PhantomData) }
}

impl<C> UnionFind<C> {
    /// Construct a new, empty union-find
    #[must_use]
    pub fn new() -> Self { Self::default() }

    /// Gets the number of nodes in the union-find
    #[must_use]
    pub fn len(&self) -> usize { self.0.len() }

    /// Returns true if the union-find has no nodes
    #[must_use]
    pub fn is_empty(&self) -> bool { self.0.is_empty() }

    pub fn roots(&self) -> Roots<C> { Roots(self.0.iter().enumerate(), PhantomData) }

    /// Add a new node to the union-find, returning its ID
    pub fn add(&mut self) -> ClassId<C> {
        let key = self.0.len();
        self.0.push(UnionFindNode {
            parent: key.into(),
            rank: 1,
        });
        ClassId::new(key)
    }

    /// Find the partition root ID for the given node ID, and optimize the
    /// search path between the node and its root
    ///
    /// # Errors
    /// This method first checks if the node ID is valid, returning an error if
    /// no associated node can be found.
    pub fn find(&self, key: ClassId<C>) -> Result<ClassId<C>, NoNode> {
        let key = key.0;
        let entry = self.0.get(key).ok_or(NoNode(key))?;
        let parent = entry.parent.load(atomic::Ordering::SeqCst);

        if parent == key {
            Ok(ClassId::new(parent))
        } else {
            let root = self
                .find(ClassId::new(parent))
                .unwrap_or_else(|_| unreachable!());

            debug_assert!(self.0.len() > key);
            // Safety: find does not change the element count
            let prev = unsafe { &self.0.get_unchecked(key).parent }.compare_exchange(
                parent,
                root.0,
                atomic::Ordering::SeqCst,
                atomic::Ordering::SeqCst,
            );
            assert!(prev == Ok(parent) || prev == Err(root.0));

            Ok(root)
        }
    }

    /// Perform the in-place union of the partitions containing the two given
    /// node IDs
    ///
    /// # Errors
    /// This method first checks if both node IDs are valid, returning an error
    /// if either cannot be found.
    pub fn union(&mut self, a: ClassId<C>, b: ClassId<C>) -> Result<Union<C>, NoNode> {
        let mut a = self.find(a)?.0;
        let mut b = self.find(b)?.0;

        let cmp = a.cmp(&b);

        if matches!(cmp, Ordering::Equal) {
            return Ok(Union {
                root: ClassId::new(a),
                unioned: None,
            });
        }

        let mut a_rank;
        let mut b_rank;
        debug_assert!(self.0.len() > a);
        debug_assert!(self.0.len() > b);
        // Safety: find does not change the element count
        unsafe {
            a_rank = self.0.get_unchecked(a).rank;
            b_rank = self.0.get_unchecked(b).rank;
        }

        match (cmp, a_rank.cmp(&b_rank)) {
            (Ordering::Equal, _) => unreachable!(),
            (_, Ordering::Less) | (Ordering::Greater, Ordering::Equal) => {
                std::mem::swap(&mut a, &mut b);
                std::mem::swap(&mut a_rank, &mut b_rank);
            },
            _ => (),
        }

        debug_assert!((a_rank, b) > (b_rank, a));

        // Safety: find nor any operations since the last unsafe block do not
        //         change the element count or key values
        unsafe {
            self.0.get_unchecked_mut(a).rank += b_rank;
            debug_assert!(self.0[a].rank == a_rank + b_rank);
            self.0.get_unchecked_mut(b).parent = a.into();
        }

        Ok(Union {
            root: ClassId::new(a),
            unioned: Some(ClassId::new(b)),
        })
    }
}

#[must_use]
#[repr(transparent)]
pub struct Roots<'a, C>(
    iter::Enumerate<slice::Iter<'a, UnionFindNode>>,
    PhantomData<&'a [ClassId<C>]>,
);

impl<C> fmt::Debug for Roots<'_, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self(it, PhantomData) = self;
        f.debug_tuple("Roots").field(it).finish()
    }
}

impl<C> Iterator for Roots<'_, C> {
    type Item = ClassId<C>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (id, node) = self.0.next()?;

            let parent = node.parent.load(atomic::Ordering::Relaxed);
            if parent == id {
                break Some(ClassId::new(parent));
            }
        }
    }
}
