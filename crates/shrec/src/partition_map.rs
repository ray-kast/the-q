use std::{
    borrow::{Borrow, Cow},
    collections::{btree_map, BTreeMap},
    fmt, mem,
    ops::{self, Bound},
};

pub trait PartitionBounds<T> {
    /// The start of the range (inclusive), if any
    fn start(&self) -> Option<&T>;

    /// The end of the range (exclusive), if any
    fn end(&self) -> Option<&T>;

    /// Returns a half-open range \[start, end)
    fn into_bounds(self) -> (Option<T>, Option<T>);
}

impl<T> PartitionBounds<T> for ops::Range<T> {
    fn start(&self) -> Option<&T> { Some(&self.start) }

    fn end(&self) -> Option<&T> { Some(&self.end) }

    fn into_bounds(self) -> (Option<T>, Option<T>) {
        let Self { start, end } = self;
        (Some(start), Some(end))
    }
}

impl<T> PartitionBounds<T> for ops::RangeFrom<T> {
    fn start(&self) -> Option<&T> { Some(&self.start) }

    fn end(&self) -> Option<&T> { None }

    fn into_bounds(self) -> (Option<T>, Option<T>) {
        let Self { start } = self;
        (Some(start), None)
    }
}

impl<T> PartitionBounds<T> for ops::RangeTo<T> {
    fn start(&self) -> Option<&T> { None }

    fn end(&self) -> Option<&T> { Some(&self.end) }

    fn into_bounds(self) -> (Option<T>, Option<T>) {
        let Self { end } = self;
        (None, Some(end))
    }
}

impl<T> PartitionBounds<T> for ops::RangeFull {
    fn start(&self) -> Option<&T> { None }

    fn end(&self) -> Option<&T> { None }

    fn into_bounds(self) -> (Option<T>, Option<T>) {
        let Self = self;
        (None, None)
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PartitionMap<K, V> {
    unbounded_start: V,
    ranges_from: BTreeMap<K, V>,
}

// TODO: const Default?
impl<K, V: Default> Default for PartitionMap<K, V> {
    fn default() -> Self { Self::new(V::default()) }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for PartitionMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.partitions()
            .fold(&mut f.debug_map(), |d, s| {
                s.debug_range(|r| d.entry(r, &s.value))
            })
            .finish()
    }
}

impl<K, V> PartitionMap<K, V> {
    #[inline]
    #[must_use]
    pub const fn new(unbounded_start: V) -> Self {
        Self {
            unbounded_start,
            ranges_from: BTreeMap::new(),
        }
    }

    #[inline]
    #[must_use]
    pub fn partitions(&self) -> Partitions<K, V> { Partitions::new(self) }
}

impl<K: Ord, V> PartitionMap<K, V> {
    #[inline]
    pub fn sample<T: ?Sized + Ord>(&self, at: &T) -> &V
    where K: Borrow<T> {
        self.ranges_from
            .range((Bound::Unbounded, Bound::Included(at)))
            .next_back()
            .map_or(&self.unbounded_start, |(_, v)| v)
    }
}

// fn as_open<T>(opt: &Option<T>) -> ops::Bound<&T> {
//     opt.as_ref()
//         .map_or(ops::Bound::Unbounded, ops::Bound::Excluded)
// }

fn as_closed<T>(opt: &Option<T>) -> Bound<&T> {
    opt.as_ref().map_or(Bound::Unbounded, Bound::Included)
}

impl<K: Clone + Ord, V: Clone + PartialEq, B: PartitionBounds<K>> Extend<(B, V)>
    for PartitionMap<K, V>
{
    fn extend<T: IntoIterator<Item = (B, V)>>(&mut self, it: T) {
        let mut over = vec![];

        for (range, value) in it {
            let (start, end) = range.into_bounds();

            debug_assert!(over.is_empty());
            over.extend(
                self.ranges_from
                    .range((as_closed(&start), as_closed(&end)))
                    .map(|(k, _)| k.clone()),
            );

            let end = end.map(|e| match over.pop() {
                Some(o) => (e, Some(Cow::Owned(self.ranges_from.remove(&o).unwrap()))),
                None => (e, None),
            });

            for key in over.drain(..) {
                assert!(self.ranges_from.remove(&key).is_some());
            }

            let start_value = start.as_ref().map(|s| {
                self.ranges_from
                    .range((Bound::Unbounded, Bound::Excluded(s)))
                    .next_back()
                    .map_or(&self.unbounded_start, |(_, v)| v)
            });

            let end = end.and_then(|(k, v)| {
                let end_val = v.unwrap_or(Cow::Borrowed(start_value.unwrap_or(&value)));
                (end_val != Cow::Borrowed(&value)).then(|| (k, end_val.into_owned()))
            });

            if start_value != Some(&value) {
                if let Some(start) = start {
                    assert!(self.ranges_from.insert(start, value).is_none());
                } else {
                    self.unbounded_start = value;
                }
            }

            if let Some((end, value)) = end {
                assert!(self.ranges_from.insert(end, value).is_none());
            }
        }
    }
}

impl<K: Clone + Ord, V: Clone + Default + PartialEq, B: PartitionBounds<K>> FromIterator<(B, V)>
    for PartitionMap<K, V>
{
    fn from_iter<I: IntoIterator<Item = (B, V)>>(it: I) -> Self {
        let mut me = Self::default();
        me.extend(it);
        me
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Partition<K, V> {
    pub start: Option<K>,
    pub end: Option<K>,
    pub value: V,
}

impl<K: fmt::Debug, V> Partition<K, V> {
    #[inline]
    pub(super) fn debug_range<T, F: FnOnce(&dyn fmt::Debug) -> T>(&self, f: F) -> T {
        // i hate ranges SO much
        match (&self.start, &self.end) {
            (None, None) => f(&(..)),
            (None, Some(e)) => f(&(..e)),
            (Some(s), None) => f(&(s..)),
            (Some(s), Some(e)) => f(&(s..e)),
        }
    }
}

#[derive(Debug, Clone)]
struct PartitionsInner<'a, K, V> {
    start: Option<&'a K>,
    value: &'a V,
    iter: btree_map::Iter<'a, K, V>,
}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct Partitions<'a, K, V>(Option<PartitionsInner<'a, K, V>>);

impl<'a, K, V> Partitions<'a, K, V> {
    #[inline]
    fn new(map: &'a PartitionMap<K, V>) -> Self {
        Self(Some(PartitionsInner {
            start: None,
            value: &map.unbounded_start,
            iter: map.ranges_from.iter(),
        }))
    }
}

impl<'a, K, V> Iterator for Partitions<'a, K, V> {
    type Item = Partition<&'a K, &'a V>;

    fn next(&mut self) -> Option<Self::Item> {
        let this = self.0.as_mut()?;

        Some(if let Some((end, next_val)) = this.iter.next() {
            let start = mem::replace(&mut this.start, Some(end));
            let value = mem::replace(&mut this.value, next_val);
            Partition {
                start,
                end: Some(end),
                value,
            }
        } else {
            let start = this.start;
            let value = this.value;
            self.0 = None;
            Partition {
                start,
                end: None,
                value,
            }
        })
    }
}
