use std::{borrow::Borrow, fmt, hash::Hash};

use hashbrown::HashMap;

#[derive(thiserror::Error)]
pub enum InsertError<L, R> {
    LhsClash(L, (L, R)),
    RhsClash(R, (L, R)),
}

impl<L: fmt::Debug, R: fmt::Debug> fmt::Debug for InsertError<L, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InsertError::LhsClash(l1, (l2, r)) => {
                write!(
                    f,
                    "Left-hand side of {l2:?} ⇔ {r:?} clashes with existing mapping {l1:?} ⇔ {r:?}"
                )
            },
            InsertError::RhsClash(r1, (l, r2)) => {
                write!(
                    f,
                    "Right-hand side of {l:?} ⇔ {r2:?} clashes with existing mapping {l:?} ⇔ \
                     {r1:?}"
                )
            },
        }
    }
}

impl<L: fmt::Display, R: fmt::Display> fmt::Display for InsertError<L, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InsertError::LhsClash(l1, (l2, r)) => {
                write!(
                    f,
                    "Left-hand side of {l2} ⇔ {r} clashes with existing mapping {l1} ⇔ {r}"
                )
            },
            InsertError::RhsClash(r1, (l, r2)) => {
                write!(
                    f,
                    "Right-hand side of {l} ⇔ {r2} clashes with existing mapping {l} ⇔ {r1}"
                )
            },
        }
    }
}

#[derive(Clone)]
pub struct Bijection<L, R> {
    fwd: HashMap<L, R>,
    bck: HashMap<R, L>,
}

impl<L: fmt::Debug, R: fmt::Debug> fmt::Debug for Bijection<L, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Pair<'a, L, R>(&'a L, &'a R);

        impl<L: fmt::Debug, R: fmt::Debug> fmt::Debug for Pair<'_, L, R> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let Self(l, r) = self;
                write!(f, "{l:?} ⇔ {r:?}")
            }
        }

        f.debug_set()
            .entries(self.fwd.iter().map(|(l, r)| Pair(l, r)))
            .finish()
    }
}

impl<L, R> Default for Bijection<L, R> {
    #[inline]
    fn default() -> Self { Self::new() }
}

impl<L, R> Bijection<L, R> {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            fwd: HashMap::new(),
            bck: HashMap::new(),
        }
    }
}

impl<L: Eq + Hash, R: Eq + Hash> Bijection<L, R> {
    #[inline]
    pub fn image<Q: Eq + Hash>(&self, l: &Q) -> Option<&R>
    where L: Borrow<Q> {
        self.fwd.get(l)
    }

    #[inline]
    pub fn preimage<Q: Eq + Hash>(&self, r: &Q) -> Option<&L>
    where R: Borrow<Q> {
        self.bck.get(r)
    }
}

impl<L: Clone + Eq + Hash, R: Clone + Eq + Hash> Bijection<L, R> {
    /// Returns true if a new mapping was inserted
    pub fn insert(&mut self, l: L, r: R) -> Result<bool, InsertError<L, R>> {
        use hashbrown::hash_map::Entry;

        let fwd_entry = self.fwd.entry(l.clone());
        let bck_entry = self.bck.entry(r.clone());

        match (fwd_entry, bck_entry) {
            (Entry::Vacant(f), Entry::Vacant(b)) => {
                f.insert(r);
                b.insert(l);
                Ok(true)
            },
            (Entry::Occupied(f), Entry::Vacant(_)) => {
                Err(InsertError::RhsClash(f.get().clone(), (l, r)))
            },
            (Entry::Vacant(_), Entry::Occupied(b)) => {
                Err(InsertError::LhsClash(b.get().clone(), (l, r)))
            },
            (Entry::Occupied(ol), Entry::Occupied(or)) => {
                if *ol.get() == r && *or.get() == l {
                    Ok(false)
                } else {
                    Err(InsertError::RhsClash(ol.get().clone(), (l, r)))
                }
            },
        }
    }
}
