use std::{collections::BTreeMap, fmt, ops::Bound};

enum MapState {
    Reserved(BTreeMap<i64, bool>),
    Deprecated,
}

#[repr(transparent)]
pub struct ReservedMap(MapState);

impl fmt::Debug for ReservedMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_tuple("ReservedMap");

        match self.0 {
            MapState::Reserved(ref res) => {
                let mut start = None;
                for (idx, &enabled) in res {
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
            MapState::Deprecated => {
                d.field(&(..));
            },
        }

        d.finish()
    }
}

impl ReservedMap {
    #[inline]
    pub const fn deprecated() -> Self { Self(MapState::Deprecated) }

    pub fn new(it: impl IntoIterator<Item = std::ops::Range<i64>>) -> Self {
        let mut map: BTreeMap<i64, bool> = BTreeMap::new();
        let mut over = vec![];

        for range in it {
            tracing::trace!(?map, ?range, "About to check overlap");
            let start = range.start;
            let end = range.end;

            debug_assert!(over.is_empty());
            over.extend(map.range(start..=end).map(|(i, e)| (*i, *e)));

            let start = over.first().map_or(true, |(_, e)| *e).then_some(start);
            let end = over.last().map_or(true, |(_, e)| !e).then_some(end);

            tracing::trace!(?over, ?start, ?end, "Overlap check done");
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

        Self(MapState::Reserved(map))
    }

    pub fn contains(&self, val: i64) -> bool {
        let map = match self.0 {
            MapState::Reserved(ref m) => m,
            MapState::Deprecated => return true,
        };

        let left = map.range(..=val).next_back();
        let right = map.range((Bound::Excluded(val), Bound::Unbounded)).next();

        debug_assert!(left.map_or(true, |(i, _)| *i <= val));
        debug_assert!(right.map_or(true, |(i, _)| *i > val));

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
