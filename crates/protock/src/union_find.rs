#[derive(Debug)]
struct UnionFindNode {
    parent: usize,
    rank: usize,
}

#[derive(Debug, Default)]
pub struct UnionFind(Vec<UnionFindNode>);

impl UnionFind {
    pub fn put(&mut self) -> usize {
        let key = self.0.len();
        self.0.push(UnionFindNode {
            parent: key,
            rank: 1,
        });
        key
    }

    pub fn find(&mut self, key: usize) -> Option<usize> {
        let entry = self.0.get(key)?;

        if entry.parent == key {
            Some(entry.parent)
        } else {
            let root = self.find(entry.parent).unwrap();

            debug_assert!(self.0.len() > key);
            // Safety: find does not change the element count
            unsafe { self.0.get_unchecked_mut(key).parent = root };

            Some(root)
        }
    }

    pub fn union(&mut self, a: usize, b: usize) -> Result<Option<usize>, ()> {
        use std::cmp::Ordering;

        let mut a = self.find(a).ok_or(())?;
        let mut b = self.find(b).ok_or(())?;

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
