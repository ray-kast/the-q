//! A disjoint-set data structure and relevant support types

use std::cmp::Ordering;

/// Error indicating a node ID passed to a [`UnionFind`] operation does not
/// exist.
#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("No disjoint-set node found with ID {0}")]
pub struct NoNode(usize);

#[derive(Debug)]
struct UnionFindNode {
    parent: usize,
    rank: usize,
}

/// A disjoint-set data structure
#[derive(Debug, Default)]
pub struct UnionFind(Vec<UnionFindNode>);

impl UnionFind {
    /// Add a new node to the union-find, returning its ID
    pub fn put(&mut self) -> usize {
        let key = self.0.len();
        self.0.push(UnionFindNode {
            parent: key,
            rank: 1,
        });
        key
    }

    /// Find the partition root ID for the given node ID, and optimize the
    /// search path between the node and its root
    ///
    /// # Errors
    /// This method first checks if the node ID is valid, returning an error if
    /// no associated node can be found.
    pub fn find(&mut self, key: usize) -> Result<usize, NoNode> {
        let entry = self.0.get(key).ok_or(NoNode(key))?;

        if entry.parent == key {
            Ok(entry.parent)
        } else {
            let root = self.find(entry.parent).unwrap_or_else(|_| unreachable!());

            debug_assert!(self.0.len() > key);
            // Safety: find does not change the element count
            unsafe { self.0.get_unchecked_mut(key).parent = root };

            Ok(root)
        }
    }

    /// Perform the in-place union of the partitions containing the two given
    /// node IDs
    ///
    /// # Errors
    /// This method first checks if both node IDs are valid, returning an error
    /// if either cannot be found.
    pub fn union(&mut self, a: usize, b: usize) -> Result<Option<usize>, NoNode> {
        let mut a = self.find(a)?;
        let mut b = self.find(b)?;

        let mut a_rank;
        let mut b_rank;
        debug_assert!(self.0.len() > a);
        debug_assert!(self.0.len() > b);
        // Safety: find does not change the element count
        unsafe {
            a_rank = self.0.get_unchecked(a).rank;
            b_rank = self.0.get_unchecked(b).rank;
        }

        match a.cmp(&b) {
            Ordering::Equal => return Ok(None),
            Ordering::Greater if a_rank <= b_rank => {
                std::mem::swap(&mut a, &mut b);
                std::mem::swap(&mut a_rank, &mut b_rank);
            },
            Ordering::Less | Ordering::Greater => (),
        }

        debug_assert!((a_rank, b) > (b_rank, a));

        // Safety: find nor any operations since the last unsafe block do not
        //         change the element count or key values
        unsafe {
            self.0.get_unchecked_mut(a).rank += b_rank;
            debug_assert!(self.0[a].rank == a_rank + b_rank);
            self.0.get_unchecked_mut(b).parent = a;
        }

        Ok(Some(a))
    }
}
