use std::{
    borrow::{Borrow, Cow},
    cmp::Ordering,
    collections::{btree_map, BTreeMap},
    fmt, mem,
    ops::{self, Bound, Deref},
};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Partition<T> {
    pub start: Option<T>,
    pub end: Option<T>,
}

impl<T: fmt::Debug> fmt::Debug for Partition<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.start, &self.end) {
            (None, None) => fmt::Debug::fmt(&(..), f),
            (None, Some(e)) => fmt::Debug::fmt(&(..e), f),
            (Some(s), None) => fmt::Debug::fmt(&(s..), f),
            (Some(s), Some(e)) => fmt::Debug::fmt(&(s..e), f),
        }
    }
}

impl<T> From<Partition<T>> for (Option<T>, Option<T>) {
    #[inline]
    fn from(Partition { start, end }: Partition<T>) -> Self { (start, end) }
}

impl<K: ToOwned> Partition<&K> {
    #[must_use]
    pub fn to_owned(self) -> Partition<K::Owned> {
        let Self { start, end } = self;
        Partition {
            start: start.map(ToOwned::to_owned),
            end: end.map(ToOwned::to_owned),
        }
    }
}

impl<T: Copy> Partition<&T> {
    #[must_use]
    pub fn copied(self) -> Partition<T> {
        let Self { start, end } = self;
        Partition {
            start: start.copied(),
            end: end.copied(),
        }
    }
}

impl<T> Partition<T> {
    #[inline]
    #[must_use]
    pub fn bounds(self) -> (Option<T>, Option<T>) { (self.start, self.end) }

    #[must_use]
    pub fn as_ref(&self) -> Partition<&T> {
        let Self { start, end } = self;
        Partition {
            start: start.as_ref(),
            end: end.as_ref(),
        }
    }
}

impl<T: Deref> Partition<T> {
    #[must_use]
    pub fn as_deref(&self) -> Partition<&T::Target> {
        let Self { start, end } = self;
        Partition {
            start: start.as_deref(),
            end: end.as_deref(),
        }
    }
}

pub trait PartitionBounds<T>: Into<Partition<T>> {
    /// The start of the range (inclusive), if any
    fn start(&self) -> Option<&T>;

    /// The end of the range (exclusive), if any
    fn end(&self) -> Option<&T>;
}

impl<T> From<ops::Range<T>> for Partition<T> {
    #[inline]
    fn from(ops::Range { start, end }: ops::Range<T>) -> Self {
        Partition {
            start: Some(start),
            end: Some(end),
        }
    }
}

impl<T> PartitionBounds<T> for ops::Range<T> {
    fn start(&self) -> Option<&T> { Some(&self.start) }

    fn end(&self) -> Option<&T> { Some(&self.end) }
}

impl<T> From<ops::RangeFrom<T>> for Partition<T> {
    #[inline]
    fn from(ops::RangeFrom { start }: ops::RangeFrom<T>) -> Self {
        Partition {
            start: Some(start),
            end: None,
        }
    }
}

impl<T> PartitionBounds<T> for ops::RangeFrom<T> {
    fn start(&self) -> Option<&T> { Some(&self.start) }

    fn end(&self) -> Option<&T> { None }
}

impl<T> From<ops::RangeTo<T>> for Partition<T> {
    #[inline]
    fn from(ops::RangeTo { end }: ops::RangeTo<T>) -> Self {
        Partition {
            start: None,
            end: Some(end),
        }
    }
}

impl<T> PartitionBounds<T> for ops::RangeTo<T> {
    fn start(&self) -> Option<&T> { None }

    fn end(&self) -> Option<&T> { Some(&self.end) }
}

impl<T> From<ops::RangeFull> for Partition<T> {
    #[inline]
    fn from(ops::RangeFull: ops::RangeFull) -> Self {
        Self {
            start: None,
            end: None,
        }
    }
}

impl<T> PartitionBounds<T> for ops::RangeFull {
    fn start(&self) -> Option<&T> { None }

    fn end(&self) -> Option<&T> { None }
}

impl<T> From<(Option<T>, Option<T>)> for Partition<T> {
    #[inline]
    fn from((start, end): (Option<T>, Option<T>)) -> Self { Self { start, end } }
}

impl<T> PartitionBounds<T> for (Option<T>, Option<T>) {
    fn start(&self) -> Option<&T> { self.0.as_ref() }

    fn end(&self) -> Option<&T> { self.1.as_ref() }
}

impl<T> PartitionBounds<T> for Partition<T> {
    fn start(&self) -> Option<&T> { self.start.as_ref() }

    fn end(&self) -> Option<&T> { self.end.as_ref() }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
            .fold(&mut f.debug_map(), |d, (k, v)| d.entry(&k, &v))
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

    #[inline]
    #[must_use]
    pub fn into_partitions(self) -> IntoPartitions<K, V> { IntoPartitions::new(self) }

    #[inline]
    #[must_use]
    pub fn keys(&self) -> Keys<K, V> { Keys(Partitions::new(self)) }

    #[inline]
    #[must_use]
    pub fn values(&self) -> Values<K, V> { Values(Partitions::new(self)) }
}

#[cfg(any(test, feature = "test"))]
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

impl<K: Copy + Ord, V> PartitionMap<&K, V> {
    #[inline]
    pub fn copied_keys(self) -> PartitionMap<K, V> {
        let Self {
            unbounded_start,
            ranges_from,
        } = self;

        PartitionMap {
            unbounded_start,
            ranges_from: ranges_from.into_iter().map(|(&k, v)| (k, v)).collect(),
        }
    }
}

fn check_bounds<T: Ord, B: PartitionBounds<T>>(range: B) -> Option<(Option<T>, Option<T>)> {
    let Partition { start, end } = range.into();

    if let (Some(s), Some(e)) = (&start, &end) {
        match s.cmp(e) {
            Ordering::Less => (),
            Ordering::Equal => return None,
            Ordering::Greater => panic!("Invalid range, start is greater than end"),
        }
    }

    Some((start, end))
}

impl<K: Clone + Ord, V: Clone + PartialEq> PartitionMap<K, V> {
    pub fn from_iter_with_default<B: PartitionBounds<K>, I: IntoIterator<Item = (B, V)>>(
        it: I,
        default: V,
    ) -> Self {
        let mut me = Self::new(default);
        me.extend(it);
        me
    }

    fn set_internal<B: PartitionBounds<K>>(&mut self, range: B, value: V, over: &mut Vec<K>) {
        let Some((start, end)) = check_bounds(range) else {
            return;
        };

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
            let ok = self.ranges_from.remove(&key).is_some();
            debug_assert!(ok);
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
                let ok = self.ranges_from.insert(start, value).is_none();
                debug_assert!(ok);
            } else {
                self.unbounded_start = value;
            }
        }

        if let Some((end, value)) = end {
            let ok = self.ranges_from.insert(end, value).is_none();
            debug_assert!(ok);
        }
    }

    #[inline]
    pub fn set<B: PartitionBounds<K>>(&mut self, range: B, value: V) {
        self.set_internal(range, value, &mut vec![]);

        #[cfg(any(test, feature = "test"))]
        self.assert_invariants();
    }

    // TODO: this could maybe be faster but like.  ghh
    fn update_internal<B: PartitionBounds<K>, F: FnMut(Partition<&K>, &V) -> V>(
        &mut self,
        range: B,
        mut f: F,
        over: &mut Vec<(K, V)>,
        set_over: &mut Vec<K>,
    ) {
        let Some((start, end)) = check_bounds(range) else {
            return;
        };

        debug_assert!(over.is_empty());
        over.extend(
            self.ranges_from
                .range((as_open(&start), as_open(&end)))
                .map(|(k, v)| (k.clone(), v.clone())),
        );

        let mut start = start;
        // TODO: maybe Cow this?  idk if it's worth performance-wise
        let mut orig_value = start
            .as_ref()
            .and_then(|s| {
                self.ranges_from
                    .range((Bound::Unbounded, Bound::Included(s)))
                    .next_back()
            })
            .map_or(&self.unbounded_start, |(_, v)| v)
            .clone();

        for (end, next_value) in over.drain(..) {
            let end = Some(end);
            debug_assert!(end > start);

            let value = f(
                Partition {
                    start: start.as_ref(),
                    end: end.as_ref(),
                },
                &orig_value,
            );
            if value != orig_value {
                self.set_internal((start, end.clone()), value, set_over);
            }

            start = end;
            orig_value = next_value;
        }

        if end.is_none() || end > start {
            let value = f(
                Partition {
                    start: start.as_ref(),
                    end: end.as_ref(),
                },
                &orig_value,
            );
            if value != orig_value {
                self.set_internal((start, end), value, set_over);
            }
        }
    }

    pub fn update<B: PartitionBounds<K>, F: FnMut(Partition<&K>, &V) -> V>(
        &mut self,
        range: B,
        f: F,
    ) {
        self.update_internal(range, f, &mut vec![], &mut vec![]);

        #[cfg(any(test, feature = "test"))]
        self.assert_invariants();
    }

    pub fn update_all<
        I: IntoIterator<Item: PartitionBounds<K>>,
        F: FnMut(Partition<&K>, &V) -> V,
    >(
        &mut self,
        it: I,
        mut f: F,
    ) {
        let mut over = vec![];
        let mut set_over = vec![];

        for range in it {
            self.update_internal(range, &mut f, &mut over, &mut set_over);
        }

        #[cfg(any(test, feature = "test"))]
        self.assert_invariants();
    }

    pub fn fold<F: FnMut((Partition<&K>, &V), &V) -> V>(
        &mut self,
        other: &PartitionMap<K, V>,
        mut f: F,
    ) {
        let mut over = vec![];
        let mut set_over = vec![];

        for (part, value) in other.partitions() {
            self.update_internal(
                part.to_owned(),
                |k, v| f((k, v), value),
                &mut over,
                &mut set_over,
            );
        }

        #[cfg(any(test, feature = "test"))]
        self.assert_invariants();
    }

    #[must_use]
    pub fn folded<F: FnMut((Partition<&K>, &V), &V) -> V>(
        mut self,
        other: &PartitionMap<K, V>,
        f: F,
    ) -> Self {
        self.fold(other, f);
        self
    }
}

#[expect(
    clippy::ref_option,
    reason = "This is a pattern-specific helper method"
)]
fn as_open<T>(opt: &Option<T>) -> ops::Bound<&T> {
    opt.as_ref().map_or(Bound::Unbounded, Bound::Excluded)
}

#[expect(
    clippy::ref_option,
    reason = "This is a pattern-specific helper method"
)]
fn as_closed<T>(opt: &Option<T>) -> Bound<&T> {
    opt.as_ref().map_or(Bound::Unbounded, Bound::Included)
}

impl<K: Clone + Ord, V: Clone + PartialEq, B: PartitionBounds<K>> Extend<(B, V)>
    for PartitionMap<K, V>
{
    fn extend<T: IntoIterator<Item = (B, V)>>(&mut self, it: T) {
        let mut over = vec![];

        for (range, value) in it {
            self.set_internal(range, value, &mut over);
        }

        #[cfg(any(test, feature = "test"))]
        self.assert_invariants();
    }
}

impl<K: Clone + Ord, V: Clone + Default + PartialEq, B: PartitionBounds<K>> FromIterator<(B, V)>
    for PartitionMap<K, V>
{
    fn from_iter<I: IntoIterator<Item = (B, V)>>(it: I) -> Self {
        Self::from_iter_with_default(it, V::default())
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
    type Item = (Partition<&'a K>, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        let this = self.0.as_mut()?;

        Some(if let Some((end, next_val)) = this.iter.next() {
            let start = this.start.replace(end);
            let value = mem::replace(&mut this.value, next_val);
            (
                Partition {
                    start,
                    end: Some(end),
                },
                value,
            )
        } else {
            let PartitionsInner {
                start,
                value,
                iter: _,
            } = self.0.take().unwrap_or_else(|| unreachable!());
            (Partition { start, end: None }, value)
        })
    }
}

#[derive(Debug)]
struct IntoPartitionsInner<K, V> {
    start: Option<K>,
    value: V,
    iter: btree_map::IntoIter<K, V>,
}

#[derive(Debug)]
#[repr(transparent)]
pub struct IntoPartitions<K, V>(Option<IntoPartitionsInner<K, V>>);

impl<K, V> IntoPartitions<K, V> {
    #[inline]
    fn new(map: PartitionMap<K, V>) -> Self {
        Self(Some(IntoPartitionsInner {
            start: None,
            value: map.unbounded_start,
            iter: map.ranges_from.into_iter(),
        }))
    }
}

impl<K: Clone, V> Iterator for IntoPartitions<K, V> {
    type Item = (Partition<K>, V);

    fn next(&mut self) -> Option<Self::Item> {
        let this = self.0.as_mut()?;

        Some(if let Some((end, next_val)) = this.iter.next() {
            let start = this.start.replace(end.clone());
            let value = mem::replace(&mut this.value, next_val);
            (
                Partition {
                    start,
                    end: Some(end),
                },
                value,
            )
        } else {
            let IntoPartitionsInner {
                start,
                value,
                iter: _,
            } = self.0.take().unwrap_or_else(|| unreachable!());
            (Partition { start, end: None }, value)
        })
    }
}

#[derive(Debug, Clone)]
pub struct Keys<'a, K, V>(Partitions<'a, K, V>);

impl<'a, K, V> Iterator for Keys<'a, K, V> {
    type Item = Partition<&'a K>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> { self.0.next().map(|(p, _)| p) }
}

#[derive(Debug, Clone)]
pub struct Values<'a, K, V>(Partitions<'a, K, V>);

impl<'a, K, V> Iterator for Values<'a, K, V> {
    type Item = &'a V;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> { self.0.next().map(|(_, v)| v) }
}

#[cfg(test)]
mod test {
    use std::{cmp::Reverse, ops::Range};

    use proptest::prelude::*;

    use super::*;

    type Map = PartitionMap<u64, char>;

    fn assert_parts<'a, P: IntoIterator<Item = &'a (Partition<u64>, char)>>(map: &Map, parts: P) {
        assert_eq!(
            map.partitions().collect::<Vec<_>>(),
            parts
                .into_iter()
                .map(|(Partition { start, end }, value)| (
                    Partition {
                        start: start.as_ref(),
                        end: end.as_ref(),
                    },
                    value
                ))
                .collect::<Vec<_>>(),
        );
    }

    fn intersect_regression_op(lhs: char, rhs: char) -> char {
        if lhs == 'b' && rhs == 'b' {
            'b'
        } else {
            'a'
        }
    }

    #[test]
    fn range_set_intersect_regression_update() {
        let mut lhs = Map::new('a');
        lhs.extend([part(1.., 'b')]);

        lhs.update(..1, |_, v| intersect_regression_op(*v, 'b'));
        lhs.update(1.., |_, v| intersect_regression_op(*v, 'a'));

        assert_eq!(lhs, Map::new('a'));
    }

    #[test]
    fn range_set_intersect_regression_fold() {
        let mut lhs = Map::new('a');
        lhs.extend([part(1.., 'b')]);
        let mut rhs = Map::new('a');
        rhs.extend([part(..1, 'b')]);

        lhs.fold(&rhs, |(_, v), s| intersect_regression_op(*v, *s));

        assert_eq!(lhs, Map::new('a'));
    }

    fn assert_sanity<'a, I: IntoIterator, P: IntoIterator<Item = &'a (Partition<u64>, char)>>(
        u: char,
        items: I,
        parts: P,
    ) where
        Map: Extend<I::Item>,
    {
        let mut map = Map::new(u);
        map.extend(items);
        assert_parts(&map, parts);
    }

    fn assert_sanity_2<'a, A, B, P: IntoIterator<Item = &'a (Partition<u64>, char)>>(
        u: char,
        a: A,
        b: B,
        parts: P,
    ) where
        Map: Extend<A> + Extend<B>,
    {
        let mut map = Map::new(u);
        map.extend([a]);
        map.extend([b]);
        assert_parts(&map, parts);
    }

    fn part<B: super::PartitionBounds<K>, K, V>(b: B, v: V) -> (Partition<K>, V) { (b.into(), v) }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "Gestures vaguely at exhaustive handwritten base cases"
    )]
    fn test_sanity() {
        // -----[=====)-----
        //  0   2 3   5 6
        //    1     4     7

        //      0 1 2 3 4 5 6 7
        // 0,1 -[+)-[=====)-----
        // 0,2 -[++)[=====)-----
        // 0,3 -[++++)[===)-----
        // 0,5 -[+++++++++)-----
        // 0,6 -[+++++++++++)---
        // 2,3 -----[)[===)-----
        // 2,5 -----[+++++)-----
        // 2,6 -----[+++++++)---
        // 3,4 -----[)[+)[)-----
        // 3,5 -----[)[+++)-----
        // 3,6 -----[)[+++++)---
        // 5,6 -----[====)[+)---
        // 6,7 -----[=====)-[+)-

        assert_sanity::<[(Range<u64>, char); 0], _>('a', [], &[part(.., 'a')]);
        assert_sanity('a', [(2..5, 'b')], &[
            part(..2, 'a'),
            part(2..5, 'b'),
            part(5.., 'a'),
        ]);

        for t in [0, 2, 3, 5, 6] {
            assert_sanity('a', [(2..5, 'b'), (t..t, 'c')], &[
                part(..2, 'a'),
                part(2..5, 'b'),
                part(5.., 'a'),
            ]);
        }

        assert_sanity('a', [(2..5, 'b'), (0..1, 'c')], &[
            part(..0, 'a'),
            part(0..1, 'c'),
            part(1..2, 'a'),
            part(2..5, 'b'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (0..2, 'c')], &[
            part(..0, 'a'),
            part(0..2, 'c'),
            part(2..5, 'b'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (0..3, 'c')], &[
            part(..0, 'a'),
            part(0..3, 'c'),
            part(3..5, 'b'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (0..5, 'c')], &[
            part(..0, 'a'),
            part(0..5, 'c'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (0..6, 'c')], &[
            part(..0, 'a'),
            part(0..6, 'c'),
            part(6.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (2..3, 'c')], &[
            part(..2, 'a'),
            part(2..3, 'c'),
            part(3..5, 'b'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (2..5, 'c')], &[
            part(..2, 'a'),
            part(2..5, 'c'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (2..6, 'c')], &[
            part(..2, 'a'),
            part(2..6, 'c'),
            part(6.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (3..4, 'c')], &[
            part(..2, 'a'),
            part(2..3, 'b'),
            part(3..4, 'c'),
            part(4..5, 'b'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (3..5, 'c')], &[
            part(..2, 'a'),
            part(2..3, 'b'),
            part(3..5, 'c'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (3..6, 'c')], &[
            part(..2, 'a'),
            part(2..3, 'b'),
            part(3..6, 'c'),
            part(6.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (5..6, 'c')], &[
            part(..2, 'a'),
            part(2..5, 'b'),
            part(5..6, 'c'),
            part(6.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (6..7, 'c')], &[
            part(..2, 'a'),
            part(2..5, 'b'),
            part(5..6, 'a'),
            part(6..7, 'c'),
            part(7.., 'a'),
        ]);

        assert_sanity('a', [(2..5, 'b'), (0..1, 'b')], &[
            part(..0, 'a'),
            part(0..1, 'b'),
            part(1..2, 'a'),
            part(2..5, 'b'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (0..2, 'b')], &[
            part(..0, 'a'),
            part(0..5, 'b'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (0..3, 'b')], &[
            part(..0, 'a'),
            part(0..5, 'b'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (0..5, 'b')], &[
            part(..0, 'a'),
            part(0..5, 'b'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (0..6, 'b')], &[
            part(..0, 'a'),
            part(0..6, 'b'),
            part(6.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (2..3, 'b')], &[
            part(..2, 'a'),
            part(2..5, 'b'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (2..5, 'b')], &[
            part(..2, 'a'),
            part(2..5, 'b'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (2..6, 'b')], &[
            part(..2, 'a'),
            part(2..6, 'b'),
            part(6.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (3..4, 'b')], &[
            part(..2, 'a'),
            part(2..5, 'b'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (3..5, 'b')], &[
            part(..2, 'a'),
            part(2..5, 'b'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (3..6, 'b')], &[
            part(..2, 'a'),
            part(2..6, 'b'),
            part(6.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (5..6, 'b')], &[
            part(..2, 'a'),
            part(2..6, 'b'),
            part(6.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (6..7, 'b')], &[
            part(..2, 'a'),
            part(2..5, 'b'),
            part(5..6, 'a'),
            part(6..7, 'b'),
            part(7.., 'a'),
        ]);

        assert_sanity('a', [(2..5, 'b'), (0..1, 'a')], &[
            part(..2, 'a'),
            part(2..5, 'b'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (0..2, 'a')], &[
            part(..2, 'a'),
            part(2..5, 'b'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (0..3, 'a')], &[
            part(..3, 'a'),
            part(3..5, 'b'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (0..5, 'a')], &[part(.., 'a')]);
        assert_sanity('a', [(2..5, 'b'), (0..6, 'a')], &[part(.., 'a')]);
        assert_sanity('a', [(2..5, 'b'), (2..3, 'a')], &[
            part(..3, 'a'),
            part(3..5, 'b'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (2..5, 'a')], &[part(.., 'a')]);
        assert_sanity('a', [(2..5, 'b'), (2..6, 'a')], &[part(.., 'a')]);
        assert_sanity('a', [(2..5, 'b'), (3..4, 'a')], &[
            part(..2, 'a'),
            part(2..3, 'b'),
            part(3..4, 'a'),
            part(4..5, 'b'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (3..5, 'a')], &[
            part(..2, 'a'),
            part(2..3, 'b'),
            part(3.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (3..6, 'a')], &[
            part(..2, 'a'),
            part(2..3, 'b'),
            part(3.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (5..6, 'a')], &[
            part(..2, 'a'),
            part(2..5, 'b'),
            part(5.., 'a'),
        ]);
        assert_sanity('a', [(2..5, 'b'), (6..7, 'a')], &[
            part(..2, 'a'),
            part(2..5, 'b'),
            part(5.., 'a'),
        ]);

        // ---[===)---
        //  0 1 2 3 4

        //      0 1 2 3 4
        // ..0 +)-[===)---
        // ..1 ++)[===)---
        // ..2 ++++)[=)---
        // ..3 +++++++)---
        // ..4 +++++++++)-
        // ..  +++++++++++
        // 0.. -[+++++++++
        // 1.. ---[+++++++
        // 2.. ---[)[+++++
        // 3.. ---[==)[+++
        // 4.. ---[===)-[+

        assert_sanity_2('a', (1..3, 'b'), (..0, 'c'), &[
            part(..0, 'c'),
            part(0..1, 'a'),
            part(1..3, 'b'),
            part(3.., 'a'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (..1, 'c'), &[
            part(..1, 'c'),
            part(1..3, 'b'),
            part(3.., 'a'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (..2, 'c'), &[
            part(..2, 'c'),
            part(2..3, 'b'),
            part(3.., 'a'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (..3, 'c'), &[
            part(..3, 'c'),
            part(3.., 'a'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (..4, 'c'), &[
            part(..4, 'c'),
            part(4.., 'a'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (.., 'c'), &[part(.., 'c')]);
        assert_sanity_2('a', (1..3, 'b'), (0.., 'c'), &[
            part(..0, 'a'),
            part(0.., 'c'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (1.., 'c'), &[
            part(..1, 'a'),
            part(1.., 'c'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (2.., 'c'), &[
            part(..1, 'a'),
            part(1..2, 'b'),
            part(2.., 'c'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (3.., 'c'), &[
            part(..1, 'a'),
            part(1..3, 'b'),
            part(3.., 'c'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (4.., 'c'), &[
            part(..1, 'a'),
            part(1..3, 'b'),
            part(3..4, 'a'),
            part(4.., 'c'),
        ]);

        assert_sanity_2('a', (1..3, 'b'), (..0, 'b'), &[
            part(..0, 'b'),
            part(0..1, 'a'),
            part(1..3, 'b'),
            part(3.., 'a'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (..1, 'b'), &[
            part(..3, 'b'),
            part(3.., 'a'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (..2, 'b'), &[
            part(..3, 'b'),
            part(3.., 'a'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (..3, 'b'), &[
            part(..3, 'b'),
            part(3.., 'a'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (..4, 'b'), &[
            part(..4, 'b'),
            part(4.., 'a'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (.., 'b'), &[part(.., 'b')]);
        assert_sanity_2('a', (1..3, 'b'), (0.., 'b'), &[
            part(..0, 'a'),
            part(0.., 'b'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (1.., 'b'), &[
            part(..1, 'a'),
            part(1.., 'b'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (2.., 'b'), &[
            part(..1, 'a'),
            part(1.., 'b'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (3.., 'b'), &[
            part(..1, 'a'),
            part(1.., 'b'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (4.., 'b'), &[
            part(..1, 'a'),
            part(1..3, 'b'),
            part(3..4, 'a'),
            part(4.., 'b'),
        ]);

        assert_sanity_2('a', (1..3, 'b'), (..0, 'a'), &[
            part(..1, 'a'),
            part(1..3, 'b'),
            part(3.., 'a'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (..1, 'a'), &[
            part(..1, 'a'),
            part(1..3, 'b'),
            part(3.., 'a'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (..2, 'a'), &[
            part(..2, 'a'),
            part(2..3, 'b'),
            part(3.., 'a'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (..3, 'a'), &[part(.., 'a')]);
        assert_sanity_2('a', (1..3, 'b'), (..4, 'a'), &[part(.., 'a')]);
        assert_sanity_2('a', (1..3, 'b'), (.., 'a'), &[part(.., 'a')]);
        assert_sanity_2('a', (1..3, 'b'), (0.., 'a'), &[part(.., 'a')]);
        assert_sanity_2('a', (1..3, 'b'), (1.., 'a'), &[part(.., 'a')]);
        assert_sanity_2('a', (1..3, 'b'), (2.., 'a'), &[
            part(..1, 'a'),
            part(1..2, 'b'),
            part(2.., 'a'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (3.., 'a'), &[
            part(..1, 'a'),
            part(1..3, 'b'),
            part(3.., 'a'),
        ]);
        assert_sanity_2('a', (1..3, 'b'), (4.., 'a'), &[
            part(..1, 'a'),
            part(1..3, 'b'),
            part(3.., 'a'),
        ]);
    }

    type Part = (Partition<u64>, char);

    fn check_part((Partition { start, end }, _): &Part) -> bool {
        start.zip(*end).is_none_or(|(s, e)| e >= s)
    }

    fn test_extend_impl(c: char, v: Vec<Part>) {
        let mut map = Map::new(c);
        map.extend(v.clone());

        let event_vec = {
            let mut e: Vec<_> = v
                .into_iter()
                .enumerate()
                .flat_map(|(i, (Partition { start, end }, value))| {
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
                    parts.push((Partition { start, end: time }, value));
                }

                start = time;
                value = next_val;
            }
        }

        parts.push((Partition { start, end: None }, value));

        assert_eq!(map.partitions().collect::<Vec<_>>(), parts);
    }

    fn test_update_sanity_impl(c: char, v: Vec<Part>) {
        let mut ext = Map::new(c);
        ext.extend(v.clone());

        let mut upd = Map::new(c);
        let mut over = vec![];
        let mut set_over = vec![];
        for (Partition { start, end }, value) in v {
            upd.update_internal((start, end), |_, _| value, &mut over, &mut set_over);
        }
    }

    fn prop_part(
        t: impl Clone + Strategy<Value = u64>,
        c: impl Strategy<Value = char>,
    ) -> impl Strategy<Value = Part> {
        (prop::option::of(t.clone()), prop::option::of(t), c)
            .prop_map(|(start, end, value)| {
                let (start, end) = if start.zip(end).is_some_and(|(s, e)| e < s) {
                    (end, start)
                } else {
                    (start, end)
                };

                (Partition { start, end }, value)
            })
            .prop_filter("Partitions must be valid", check_part)
    }

    proptest! {
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
        fn test_update_sanity(
            c in any::<char>(),
            v in prop::collection::vec(
                prop_part(any::<u64>(), any::<char>()),
                0..256,
            ),
        ) {
            test_update_sanity_impl(c, v);
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
