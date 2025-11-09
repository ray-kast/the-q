use std::{borrow::Borrow, fmt, ops};

use crate::partition_map::{IntoPartitions, Partition, PartitionBounds, PartitionMap, Partitions};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct RangeSet<T>(PartitionMap<T, bool>);

impl<T: fmt::Debug> fmt::Debug for RangeSet<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0
            .partitions()
            .filter_map(|(k, &v)| v.then_some(k))
            .fold(&mut f.debug_set(), |d, k| d.entry(&k))
            .finish()
    }
}

impl<T> RangeSet<T> {
    pub const EMPTY: Self = Self::empty();
    pub const FULL: Self = Self::full();

    #[must_use]
    #[inline]
    pub const fn new(init: bool) -> Self { Self(PartitionMap::new(init)) }

    #[must_use]
    #[inline]
    pub const fn empty() -> Self { Self::new(false) }

    #[must_use]
    #[inline]
    pub const fn full() -> Self { Self::new(true) }

    #[must_use]
    pub fn all_ranges(&self) -> AllRanges<'_, T> { AllRanges(self.0.partitions()) }

    #[must_use]
    pub fn ranges(&self) -> Ranges<'_, T> { Ranges(self.0.partitions(), true) }

    #[must_use]
    pub fn empty_ranges(&self) -> Ranges<'_, T> { Ranges(self.0.partitions(), false) }

    #[must_use]
    pub fn into_ranges(self) -> IntoRanges<T> { IntoRanges(self.0.into_partitions(), true) }

    #[must_use]
    pub fn into_empty_ranges(self) -> IntoRanges<T> { IntoRanges(self.0.into_partitions(), false) }
}

impl<T: Ord> RangeSet<T> {
    #[inline]
    pub fn contains<Q: ?Sized + Ord>(&self, at: &Q) -> bool
    where T: Borrow<Q> {
        *self.0.sample(at)
    }
}

impl<T: Clone + Ord> RangeSet<T> {
    #[inline]
    pub fn insert<B: PartitionBounds<T>>(&mut self, range: B) { self.0.set(range, true); }

    #[inline]
    pub fn remove<B: PartitionBounds<T>>(&mut self, range: B) { self.0.set(range, false); }

    pub fn union(&mut self, other: &Self) { self.0.fold(other, |(_, &v), &s| v || s); }

    #[must_use]
    #[inline]
    pub fn unioned(mut self, other: &Self) -> Self {
        self.union(other);
        self
    }

    pub fn intersect(&mut self, other: &Self) { self.0.fold(other, |(_, &v), &s| v && s); }

    #[must_use]
    #[inline]
    pub fn intersected(mut self, other: &Self) -> Self {
        self.intersect(other);
        self
    }

    pub fn invert(&mut self) { self.0.update(.., |_, &v| !v); }

    #[must_use]
    #[inline]
    pub fn inverted(mut self) -> Self {
        self.invert();
        self
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
        self.0.extend(it.into_iter().map(|r| (r, true)));
    }
}

impl<T: Clone + Ord, B: PartitionBounds<T>> FromIterator<B> for RangeSet<T> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = B>>(it: I) -> Self {
        Self(it.into_iter().map(|r| (r, true)).collect())
    }
}

#[derive(Debug, Clone)]
pub struct Ranges<'a, T>(Partitions<'a, T, bool>, bool);

impl<'a, T> Iterator for Ranges<'a, T> {
    type Item = Partition<&'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (part, &value) = self.0.next()?;

            if value == self.1 {
                return Some(part);
            }
        }
    }
}

#[derive(Debug)]
pub struct IntoRanges<T>(IntoPartitions<T, bool>, bool);

impl<T: Clone> Iterator for IntoRanges<T> {
    type Item = Partition<T>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (part, value) = self.0.next()?;

            if value == self.1 {
                return Some(part);
            }
        }
    }
}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct AllRanges<'a, T>(Partitions<'a, T, bool>);

impl<'a, T> Iterator for AllRanges<'a, T> {
    type Item = (Partition<&'a T>, bool);

    fn next(&mut self) -> Option<Self::Item> {
        let (part, &value) = self.0.next()?;
        Some((part, value))
    }
}

#[cfg(test)]
mod test {
    use proptest::prelude::*;

    use super::*;

    const MAX: usize = 1024;
    type Part = (Partition<usize>, bool);

    fn check_part((Partition { start, end }, _): &Part) -> bool {
        start.zip(*end).is_none_or(|(s, e)| e >= s)
    }

    fn prop_idx() -> impl Strategy<Value = usize> { 0..MAX }

    fn to_open<T>(opt: Option<T>) -> ops::Bound<T> {
        opt.map_or(ops::Bound::Unbounded, ops::Bound::Excluded)
    }

    fn to_closed<T>(opt: Option<T>) -> ops::Bound<T> {
        opt.map_or(ops::Bound::Unbounded, ops::Bound::Included)
    }

    fn prop_part() -> impl Strategy<Value = Part> {
        (
            prop::option::of(prop_idx()),
            prop::option::of(prop_idx()),
            any::<bool>(),
        )
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

    fn dense_map(start: bool, parts: &[Part]) -> [bool; MAX] {
        let mut arr = [start; MAX];

        for (Partition { start, end }, value) in parts {
            arr[(to_closed(*start), to_open(*end))]
                .iter_mut()
                .for_each(|v| *v = *value);
        }

        arr
    }

    fn test_unop<F: FnOnce(&mut RangeSet<usize>), G: Fn(bool) -> bool>(
        s: bool,
        v: Vec<Part>,
        op: F,
        dense_op: G,
    ) {
        let dense = dense_map(s, &v);

        let mut set = RangeSet::new(s);
        (*set).extend(v);
        op(&mut set);

        for (i, el) in dense.into_iter().enumerate() {
            assert_eq!(*set.sample(&i), dense_op(el), "i = {i:?}, set = {set:?}");
        }
    }

    fn test_binop<F: FnOnce(&mut RangeSet<usize>, &RangeSet<usize>), G: Fn(bool, bool) -> bool>(
        ls: bool,
        lv: Vec<Part>,
        rs: bool,
        rv: Vec<Part>,
        op: F,
        dense_op: G,
    ) {
        let ld = dense_map(ls, &lv);
        let rd = dense_map(rs, &rv);

        let mut lhs = RangeSet::new(ls);
        let mut rhs = RangeSet::new(rs);
        (*lhs).extend(lv);
        (*rhs).extend(rv);
        let mut out = lhs.clone();
        op(&mut out, &rhs);

        for i in 0..MAX {
            assert_eq!(
                *out.sample(&i),
                dense_op(ld[i], rd[i]),
                "i = {i:?}, lhs = {lhs:?}, rhs = {rhs:?}, out = {out:?}"
            );
        }
    }

    fn part<B: super::PartitionBounds<K>, K, V>(b: B, v: V) -> (Partition<K>, V) { (b.into(), v) }

    #[test]
    fn inter_sanity() {
        test_binop(
            false,
            vec![part(1.., true)],
            false,
            vec![part(..1, true)],
            RangeSet::intersect,
            |l, r| l && r,
        );
        test_binop(
            false,
            vec![part(..1, true)],
            false,
            vec![part(1.., true)],
            RangeSet::intersect,
            |l, r| l && r,
        );
    }

    proptest! {
        #[test]
        fn test_union(
            ls in any::<bool>(),
            lv in prop::collection::vec(prop_part(), 0..512),
            rs in any::<bool>(),
            rv in prop::collection::vec(prop_part(), 0..512),
        ) {
            test_binop(ls, lv, rs, rv, RangeSet::union, |l, r| l || r);
        }

        #[test]
        fn test_intersect(
            ls in any::<bool>(),
            lv in prop::collection::vec(prop_part(), 0..512),
            rs in any::<bool>(),
            rv in prop::collection::vec(prop_part(), 0..512),
        ) {
            test_binop(ls, lv, rs, rv, RangeSet::intersect, |l, r| l && r);
        }

        #[test]
        fn test_invert(
            s in any::<bool>(),
            v in prop::collection::vec(prop_part(), 0..512),
        ) {
            test_unop(s, v, RangeSet::invert, |b| !b);
        }
    }
}
