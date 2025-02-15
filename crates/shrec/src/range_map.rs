use std::{borrow::Borrow, fmt, ops};

use crate::partition_map::{Partition, PartitionBounds, PartitionMap};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct RangeMap<K, V>(PartitionMap<K, Option<V>>);

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for RangeMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0
            .partitions()
            .fold(&mut f.debug_map(), |d, s| {
                if let Some(ref value) = s.value {
                    s.debug_range(|r| d.entry(r, value))
                } else {
                    d
                }
            })
            .finish()
    }
}

impl<K, V> Default for RangeMap<K, V> {
    fn default() -> Self { Self::new() }
}

impl<K, V> RangeMap<K, V> {
    pub const EMPTY: Self = Self::new();

    #[must_use]
    #[inline]
    pub const fn full(value: V) -> Self { Self(PartitionMap::new(Some(value))) }

    #[must_use]
    #[inline]
    pub const fn new() -> Self { Self(PartitionMap::new(None)) }
}

impl<K: Ord, V> RangeMap<K, V> {
    #[inline]
    pub fn contains<Q: ?Sized + Ord>(&self, at: &Q) -> bool
    where K: Borrow<Q> {
        self.0.sample(at).is_some()
    }
}

impl<K: Clone + Ord, V: Clone + PartialEq> RangeMap<K, V> {
    #[inline]
    pub fn insert<B: PartitionBounds<K>>(&mut self, range: B, value: V) {
        self.0.set(range, Some(value));
    }

    #[inline]
    pub fn remove<B: PartitionBounds<K>>(&mut self, range: B) { self.0.set(range, None); }
}
impl<K, V> ops::Deref for RangeMap<K, V> {
    type Target = PartitionMap<K, Option<V>>;

    #[inline]
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl<K, V> ops::DerefMut for RangeMap<K, V> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl<K: Clone + Ord, V: Clone + PartialEq, B: PartitionBounds<K>> Extend<(B, V)>
    for RangeMap<K, V>
{
    #[inline]
    fn extend<I: IntoIterator<Item = (B, V)>>(&mut self, it: I) {
        self.0.extend(it.into_iter().map(|(k, v)| (k, Some(v))));
    }
}

impl<K: Clone + Ord, V: Clone + PartialEq> Extend<Partition<K, V>> for RangeMap<K, V> {
    #[inline]
    fn extend<T: IntoIterator<Item = Partition<K, V>>>(&mut self, it: T) {
        self.0.extend(it.into_iter().map(|p| p.map_value(Some)));
    }
}

impl<K: Clone + Ord, V: Clone + Default + PartialEq, B: PartitionBounds<K>> FromIterator<(B, V)>
    for RangeMap<K, V>
{
    #[inline]
    fn from_iter<I: IntoIterator<Item = (B, V)>>(it: I) -> Self {
        Self(it.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }
}

impl<K: Clone + Ord, V: Clone + PartialEq> FromIterator<Partition<K, V>> for RangeMap<K, V> {
    #[inline]
    fn from_iter<T: IntoIterator<Item = Partition<K, V>>>(it: T) -> Self {
        Self(it.into_iter().map(|p| p.map_value(Some)).collect())
    }
}
