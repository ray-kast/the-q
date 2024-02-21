use std::{fmt, ops};

use crate::{partition_map::PartitionBounds, range_map::RangeMap};

#[derive(Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct RangeSet<T>(RangeMap<T, ()>);

impl<T: fmt::Debug> fmt::Debug for RangeSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0
            .partitions()
            .fold(&mut f.debug_set(), |d, s| s.debug_range(|r| d.entry(r)))
            .finish()
    }
}

impl<T> RangeSet<T> {
    pub const EMPTY: Self = Self::new();
    pub const FULL: Self = Self::full();

    #[must_use]
    #[inline]
    pub const fn full() -> Self { Self(RangeMap::full(())) }

    #[must_use]
    #[inline]
    pub const fn new() -> Self { Self(RangeMap::new()) }
}

impl<T> ops::Deref for RangeSet<T> {
    type Target = RangeMap<T, ()>;

    #[inline]
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl<T> ops::DerefMut for RangeSet<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl<T: Clone + Ord, B: PartitionBounds<T>> Extend<B> for RangeSet<T> {
    #[inline]
    fn extend<I: IntoIterator<Item = B>>(&mut self, it: I) {
        self.0.extend(it.into_iter().map(|r| (r, ())));
    }
}

impl<T: Clone + Ord, B: PartitionBounds<T>> FromIterator<B> for RangeSet<T> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = B>>(it: I) -> Self {
        Self(it.into_iter().map(|r| (r, ())).collect())
    }
}
