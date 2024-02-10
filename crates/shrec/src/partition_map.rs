use std::{
    borrow::{Borrow, Cow},
    cmp::Ordering,
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

impl<T> PartitionBounds<T> for (Option<T>, Option<T>) {
    fn start(&self) -> Option<&T> { self.0.as_ref() }

    fn end(&self) -> Option<&T> { self.1.as_ref() }

    fn into_bounds(self) -> (Option<T>, Option<T>) { self }
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

#[cfg(test)]
impl<K, V: PartialEq> PartitionMap<K, V> {
    fn assert_invariants(&self) {
        let mut last = &self.unbounded_start;

        for val in self.ranges_from.values() {
            assert!(val != mem::replace(&mut last, val));
        }
    }
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

            match (&start, &end) {
                (Some(s), Some(e)) => match s.cmp(e) {
                    Ordering::Less => (),
                    Ordering::Equal => continue,
                    Ordering::Greater => panic!("Invalid range, start is greater than end"),
                },
                (_, None) | (None, _) => (),
            }

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
                debug_assert!(self.ranges_from.remove(&key).is_some());
            }

            let start_value = start
                .as_ref()
                .and_then(|s| {
                    self.ranges_from
                        .range((Bound::Unbounded, Bound::Excluded(s)))
                        .next_back()
                })
                .map_or(&self.unbounded_start, |(_, v)| v);

            let end = end.and_then(|(e, v)| {
                let end_val = v.unwrap_or(Cow::Borrowed(start_value));
                (end_val != Cow::Borrowed(&value)).then(|| (e, end_val.into_owned()))
            });

            if *start_value != value {
                if let Some(start) = start {
                    debug_assert!(self.ranges_from.insert(start, value).is_none());
                } else {
                    self.unbounded_start = value;
                }
            }

            if let Some((end, value)) = end {
                debug_assert!(self.ranges_from.insert(end, value).is_none());
            }
        }

        #[cfg(test)]
        self.assert_invariants();
    }
}

impl<K: Clone + Ord, V: Clone + PartialEq> Extend<Partition<K, V>> for PartitionMap<K, V> {
    #[inline]
    fn extend<T: IntoIterator<Item = Partition<K, V>>>(&mut self, iter: T) {
        self.extend(
            iter.into_iter()
                .map(|Partition { start, end, value }| ((start, end), value)),
        );
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Partition<K, V> {
    pub start: Option<K>,
    pub end: Option<K>,
    pub value: V,
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for Partition<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.debug_range(|r| {
            f.debug_tuple("Partition")
                .field(r)
                .field(&self.value)
                .finish()
        })
    }
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

#[cfg(test)]
mod tests {
    use std::{cmp::Reverse, ops::Range};

    use proptest::prelude::*;

    use super::*;

    type Map = PartitionMap<u64, char>;

    fn all_bounds<T: Copy>(range: Range<T>) -> impl Iterator<Item = (Option<T>, Option<T>)> {
        let Range { start, end } = range;

        [
            (Some(start), Some(end)),
            (None, Some(end)),
            (Some(start), None),
            (None, None),
        ]
        .into_iter()
    }

    fn insert_one(map: &Map, range: Range<u64>, val1: char, val2: char) {
        for b in all_bounds(range) {
            map.clone().extend([(b, val1)]);
            map.clone().extend([(b, val2)]);
        }
    }

    #[test]
    fn test_invariants_single() {
        let map = Map::new('a');
        map.assert_invariants();

        insert_one(&map, 1..2, 'a', 'b');
    }

    #[test]
    fn test_invariants_overlap_start() {
        let map: Map = [(2..4, 'a')].into_iter().collect();

        insert_one(&map, 1..3, 'a', 'b');
    }

    #[test]
    fn test_invariants_overlap_end() {
        let map: Map = [(1..3, 'a')].into_iter().collect();

        insert_one(&map, 2..4, 'a', 'b');
    }

    #[test]
    fn test_invariants_overlap_inner() {
        let map: Map = [(1..4, 'a')].into_iter().collect();

        insert_one(&map, 2..3, 'a', 'b');
    }

    #[test]
    fn test_invariants_overlap_outer() {
        let map: Map = [(2..3, 'a')].into_iter().collect();

        insert_one(&map, 1..4, 'a', 'b');
    }

    type Part = Partition<u64, char>;

    fn check_part(
        Partition {
            start,
            end,
            value: _,
        }: &Part,
    ) -> bool {
        start.zip(*end).map_or(true, |(s, e)| e >= s)
    }

    fn test_extend_impl(c: char, v: Vec<Part>) {
        let mut map: Map = Map::new(c);
        map.extend(v.clone());

        let event_vec = {
            let mut e: Vec<_> = v
                .into_iter()
                .enumerate()
                .flat_map(|(i, Partition { start, end, value })| {
                    [
                        Some((start, i, Reverse(Some(value)))),
                        end.map(|e| (Some(e), i, Reverse(None))),
                    ]
                    .into_iter()
                    .flatten()
                })
                .collect();
            e.sort();
            e
        };
        let events =
            event_vec
                .iter()
                .fold(BTreeMap::<_, Vec<_>>::new(), |mut m, (t, i, Reverse(v))| {
                    m.entry(t).or_default().push((i, v));
                    m
                });
        let mut enabled = BTreeMap::new();
        let mut parts = vec![];
        let mut start = None;
        let mut value = &c;

        for (time, curr_events) in events {
            for (index, event_val) in curr_events {
                match event_val {
                    Some(v) => assert!(enabled.insert(index, v).is_none()),
                    None => assert!(enabled.remove(&index).is_some()),
                }
            }

            let next_val = enabled.last_key_value().map_or(&c, |(_, v)| *v);
            let time = time.as_ref();

            if next_val != value {
                if time.is_some() {
                    parts.push(Partition {
                        start,
                        end: time,
                        value,
                    });
                }

                start = time;
                value = next_val;
            }
        }

        parts.push(Partition {
            start,
            end: None,
            value,
        });

        assert_eq!(map.partitions().collect::<Vec<_>>(), parts);
    }

    fn prop_part(
        t: impl Clone + Strategy<Value = u64>,
        c: impl Strategy<Value = char>,
    ) -> impl Strategy<Value = Part> {
        (prop::option::of(t.clone()), prop::option::of(t), c)
            .prop_map(|(start, end, value)| Partition { start, end, value })
            .prop_filter("Partitions must be valid", check_part)
    }

    proptest::proptest! {
        #[test]
        fn test_extend(
            c in any::<char>(),
            v in prop::collection::vec(
                prop_part(any::<u64>(), any::<char>()),
                0..256,
            ),
        ) {
            test_extend_impl(c, v);
        }

        #[test]
        fn test_extend_clobber(
            c in prop::char::range('a', 'z'),
            v in prop::collection::vec(
                prop_part(0_u64..16, prop::char::range('a', 'z')),
                0..512
            ),
        ) {
            test_extend_impl(c, v);
        }
    }
}
