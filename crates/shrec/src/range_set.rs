use std::{borrow::Borrow, fmt, ops};

use crate::partition_map::{PartitionBounds, PartitionMap};

#[derive(Clone, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct RangeSet<T>(PartitionMap<T, bool>);

impl<T: fmt::Debug> fmt::Debug for RangeSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0
            .partitions()
            .fold(&mut f.debug_map(), |d, s| {
                s.debug_range(|r| d.entry(r, &s.value))
            })
            .finish()
    }
}

impl<T> RangeSet<T> {
    pub const EMPTY: Self = Self::new();
    pub const FULL: Self = Self::full();

    #[must_use]
    #[inline]
    pub const fn full() -> Self { Self(PartitionMap::new(true)) }

    #[must_use]
    #[inline]
    pub const fn new() -> Self { Self(PartitionMap::new(false)) }
}

impl<T: Ord> RangeSet<T> {
    #[inline]
    pub fn contains<U: ?Sized + Ord>(&self, at: &U) -> bool
    where T: Borrow<U> {
        *self.0.sample(at)
    }
}

impl<T> ops::Deref for RangeSet<T> {
    type Target = PartitionMap<T, bool>;

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
        self.0.extend(it.into_iter().map(|b| (b, true)));
    }
}

impl<T: Clone + Ord, B: PartitionBounds<T>> FromIterator<B> for RangeSet<T> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = B>>(it: I) -> Self {
        Self(it.into_iter().map(|b| (b, true)).collect())
    }
}
