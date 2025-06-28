use std::{borrow::Borrow, fmt, ops};

use crate::partition_map::{IntoPartitions, Partition, PartitionBounds, PartitionMap, Partitions};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct RangeMap<K, V>(PartitionMap<K, Option<V>>);

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for RangeMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0
            .partitions()
            .fold(&mut f.debug_map(), |d, (k, v)| {
                if let Some(v) = v {
                    d.entry(&k, v)
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

    #[must_use]
    pub fn ranges(&self) -> Ranges<K, V> { Ranges(self.0.partitions()) }

    #[must_use]
    pub fn into_ranges(self) -> IntoRanges<K, V> { IntoRanges(self.0.into_partitions()) }

    #[inline]
    #[must_use]
    pub fn keys(&self) -> Keys<K, V> { Keys(Ranges(self.0.partitions())) }

    #[inline]
    #[must_use]
    pub fn values(&self) -> Values<K, V> { Values(Ranges(self.0.partitions())) }
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

impl<K: Clone + Ord, V: Clone + PartialEq, B: PartitionBounds<K>> FromIterator<(B, V)>
    for RangeMap<K, V>
{
    #[inline]
    fn from_iter<I: IntoIterator<Item = (B, V)>>(it: I) -> Self {
        Self(it.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }
}

#[derive(Debug, Clone)]
pub struct Ranges<'a, K, V>(Partitions<'a, K, Option<V>>);

impl<'a, K, V> Iterator for Ranges<'a, K, V> {
    type Item = (Partition<&'a K>, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (Partition { start, end }, value) = self.0.next()?;

            if let Some(value) = value {
                return Some((Partition { start, end }, value));
            }
        }
    }
}

#[derive(Debug)]
pub struct IntoRanges<K, V>(IntoPartitions<K, Option<V>>);

impl<K: Clone, V> Iterator for IntoRanges<K, V> {
    type Item = (Partition<K>, V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (Partition { start, end }, value) = self.0.next()?;

            if let Some(value) = value {
                return Some((Partition { start, end }, value));
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Keys<'a, K, V>(Ranges<'a, K, V>);

impl<'a, K, V> Iterator for Keys<'a, K, V> {
    type Item = Partition<&'a K>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> { self.0.next().map(|(k, _)| k) }
}

#[derive(Debug, Clone)]
pub struct Values<'a, K, V>(Ranges<'a, K, V>);

impl<'a, K, V> Iterator for Values<'a, K, V> {
    type Item = &'a V;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> { self.0.next().map(|(_, v)| v) }
}
