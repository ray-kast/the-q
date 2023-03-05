use std::{
    collections::BTreeMap,
    fmt,
    ops::{Bound, Range},
};

#[derive(Clone)]
enum MapState<T> {
    Ranges(BTreeMap<T, bool>),
    Full,
}

#[derive(Clone)]
#[repr(transparent)]
pub struct RangeMap<T>(MapState<T>);

impl<T: fmt::Debug> fmt::Debug for RangeMap<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_tuple("RangeMap");

        match self.0 {
            MapState::Ranges(ref ranges) => {
                let mut start = None;
                for (idx, &enabled) in ranges {
                    if enabled {
                        assert!(std::mem::replace(&mut start, Some(idx)).is_none());
                    } else if let Some(start) = start.take() {
                        d.field(&(start..idx));
                    } else {
                        d.field(&(..idx));
                    }
                }

                if let Some(start) = start {
                    d.field(&(start..));
                }
            },
            MapState::Full => {
                d.field(&(..));
            },
        }

        d.finish()
    }
}

impl<T> RangeMap<T> {
    pub const EMPTY: Self = Self::new();
    pub const FULL: Self = Self::full();

    #[must_use]
    #[inline]
    pub const fn full() -> Self { Self(MapState::Full) }

    #[must_use]
    #[inline]
    pub const fn new() -> Self { Self(MapState::Ranges(BTreeMap::new())) }
}

impl<T: Ord> RangeMap<T> {
    pub fn contains(&self, val: &T) -> bool {
        let map = match self.0 {
            MapState::Ranges(ref m) => m,
            MapState::Full => return true,
        };

        let left = map.range(..=val).next_back();
        let right = map.range((Bound::Excluded(val), Bound::Unbounded)).next();

        debug_assert!(left.map_or(true, |(i, _)| i <= val));
        debug_assert!(right.map_or(true, |(i, _)| i > val));

        match (left, right) {
            (None, None) => {
                debug_assert!(map.is_empty());
                false
            },
            (None, Some((_, &next_start))) => !next_start,
            (Some((_, &prev_end)), None) => prev_end,
            (Some((_, &prev_end)), Some((_, &next_start))) => {
                assert!(prev_end != next_start);
                prev_end
            },
        }
    }
}

impl<T: Clone + Ord> Extend<Range<T>> for RangeMap<T> {
    fn extend<I: IntoIterator<Item = Range<T>>>(&mut self, it: I) {
        let map = match self.0 {
            MapState::Ranges(ref mut m) => m,
            MapState::Full => return,
        };
        let mut over = vec![];

        for range in it {
            let Range { start, end } = range;

            debug_assert!(over.is_empty());
            over.extend(map.range(&start..=&end).map(|(i, e)| (i.clone(), *e)));

            let start = over.first().map_or(true, |(_, e)| *e).then_some(start);
            let end = over.last().map_or(true, |(_, e)| !e).then_some(end);

            debug_assert!(over.len() % 2 == usize::from(start.is_some() != end.is_some()));

            for (idx, _) in over.drain(..) {
                assert!(map.remove(&idx).is_some());
            }

            if let Some(start) = start {
                assert!(map.insert(start, true).is_none());
            }

            if let Some(end) = end {
                assert!(map.insert(end, false).is_none());
            }
        }
    }
}

impl<T: Clone + Ord> FromIterator<Range<T>> for RangeMap<T> {
    fn from_iter<I: IntoIterator<Item = Range<T>>>(it: I) -> Self {
        let mut me = Self::new();
        me.extend(it);
        me
    }
}
