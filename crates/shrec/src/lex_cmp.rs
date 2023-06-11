use std::{
    cmp::Ordering,
    ops::{Range, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive},
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Lexicographic<T>(T);

impl<T: LexicographicOrd> Ord for Lexicographic<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering { self.0.lex_cmp(&other.0) }
}

impl<T: LexicographicOrd> PartialOrd for Lexicographic<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

pub trait LexicographicOrd: Eq {
    fn lex_cmp(&self, rhs: &Self) -> Ordering;
}

macro_rules! ord_impl {
    () => {};

    ($t:ty $(, $($tt:tt)*)?) => {
        impl LexicographicOrd for $t {
            #[inline]
            fn lex_cmp(&self, rhs: &Self) -> Ordering { self.cmp(rhs) }
        }

        ord_impl!($($($tt)*)?);
    };
}

ord_impl!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, char);

impl<T: Ord> LexicographicOrd for Range<T> {
    fn lex_cmp(&self, rhs: &Self) -> Ordering {
        let Self {
            start: l_start,
            end: l_end,
        } = self;
        let Self {
            start: r_start,
            end: r_end,
        } = rhs;
        l_start.cmp(r_start).then_with(|| l_end.cmp(r_end))
    }
}

impl<T: Ord> LexicographicOrd for RangeFrom<T> {
    fn lex_cmp(&self, rhs: &Self) -> Ordering {
        let Self { start: l_start } = self;
        let Self { start: r_start } = rhs;
        l_start.cmp(r_start)
    }
}

impl LexicographicOrd for RangeFull {
    fn lex_cmp(&self, rhs: &Self) -> Ordering {
        debug_assert_eq!(self, rhs);
        Ordering::Equal
    }
}

impl<T: Ord> LexicographicOrd for RangeInclusive<T> {
    fn lex_cmp(&self, rhs: &Self) -> Ordering {
        self.start()
            .cmp(rhs.start())
            .then_with(|| self.end().cmp(rhs.end()))
    }
}

impl<T: Ord> LexicographicOrd for RangeTo<T> {
    fn lex_cmp(&self, rhs: &Self) -> Ordering {
        let Self { end: l_end } = self;
        let Self { end: r_end } = rhs;
        l_end.cmp(r_end)
    }
}

impl<T: Ord> LexicographicOrd for RangeToInclusive<T> {
    fn lex_cmp(&self, rhs: &Self) -> Ordering {
        let Self { end: l_end } = self;
        let Self { end: r_end } = rhs;
        l_end.cmp(r_end)
    }
}
