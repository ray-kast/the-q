use std::{
    borrow::BorrowMut,
    collections::{BTreeSet, VecDeque},
    hash::Hash,
};

use hashbrown::HashSet;

pub trait SetInsert<T> {
    fn insert(&mut self, t: T) -> bool;
}

impl<T: Eq + Hash> SetInsert<T> for HashSet<T> {
    #[inline]
    fn insert(&mut self, t: T) -> bool { HashSet::insert(self, t) }
}

impl<T: Ord> SetInsert<T> for BTreeSet<T> {
    #[inline]
    fn insert(&mut self, t: T) -> bool { BTreeSet::insert(self, t) }
}

#[derive(Debug)]
pub struct ClosureBuilder<T>(VecDeque<T>);

impl<T> Default for ClosureBuilder<T> {
    #[inline]
    fn default() -> Self { Self(VecDeque::new()) }
}

impl<T> ClosureBuilder<T> {
    #[inline]
    pub fn init<I: IntoIterator<Item = T>>(&mut self, it: I) {
        assert!(self.0.is_empty());
        self.extend(it);
    }
}

impl<T: Clone> ClosureBuilder<T> {
    pub fn solve<S: BorrowMut<U>, U: SetInsert<T>, I: IntoIterator<Item = T>>(
        &mut self,
        mut set: S,
        f: impl Fn(T) -> I,
    ) -> S {
        {
            let set = set.borrow_mut();

            while let Some(el) = self.0.pop_front() {
                if set.insert(el.clone()) {
                    self.0.extend(f(el));
                }
            }
        }

        set
    }
}

impl<T> Extend<T> for ClosureBuilder<T> {
    #[inline]
    fn extend<I: IntoIterator<Item = T>>(&mut self, it: I) { self.0.extend(it); }
}
