use std::{
    collections::HashMap,
    fmt,
    hash::Hash,
    ops::{Deref, DerefMut},
};

use crate::check_compat::{CheckCompat, CompatLog};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompatPair<T> {
    reader: T,
    writer: T,
}

impl From<()> for CompatPair<()> {
    fn from((): ()) -> Self {
        Self {
            reader: (),
            writer: (),
        }
    }
}

impl<T> CompatPair<T> {
    #[inline]
    pub const fn new(reader: T, writer: T) -> Self { Self { reader, writer } }

    #[inline]
    pub fn as_ref(&self) -> CompatPair<&T> {
        let Self { reader, writer } = self;
        CompatPair { reader, writer }
    }

    #[inline]
    pub const fn display(&self) -> Display<CompatPair<T>> { Display(self) }

    pub fn into_inner(self) -> (T, T) {
        let Self { reader, writer } = self;
        (reader, writer)
    }

    #[inline]
    pub fn visit(self, side: Side) -> T {
        match side {
            Side::Reader(()) => self.reader,
            Side::Writer(()) => self.writer,
        }
    }

    pub fn for_each(self, mut f: impl FnMut(Side<T>)) {
        let Self { reader, writer } = self;
        f(Side::Reader(reader));
        f(Side::Writer(writer));
    }

    pub fn map<U>(self, f: impl Fn(T) -> U) -> CompatPair<U> {
        let Self { reader, writer } = self;
        CompatPair {
            reader: f(reader),
            writer: f(writer),
        }
    }

    pub fn try_map<U, E>(self, f: impl Fn(T) -> Result<U, E>) -> Result<CompatPair<U>, Side<E>> {
        let Self { reader, writer } = self;
        Ok(CompatPair {
            reader: f(reader).map_err(Side::Reader)?,
            writer: f(writer).map_err(Side::Writer)?,
        })
    }

    pub fn filter_map<U>(self, f: impl Fn(T) -> Option<U>) -> Option<CompatPair<U>> {
        let Self { reader, writer } = self;
        Some(CompatPair {
            reader: f(reader)?,
            writer: f(writer)?,
        })
    }

    pub fn zip<U>(self, other: CompatPair<U>) -> CompatPair<(T, U)> {
        let Self {
            reader: r1,
            writer: w1,
        } = self;
        let CompatPair {
            reader: r2,
            writer: w2,
        } = other;
        CompatPair {
            reader: (r1, r2),
            writer: (w1, w2),
        }
    }
}

impl<'a, T: Copy> CompatPair<&'a T> {
    #[inline]
    pub const fn copied(self) -> CompatPair<T> {
        let Self {
            reader: &reader,
            writer: &writer,
        } = self;
        CompatPair { reader, writer }
    }
}

impl<T, U> CompatPair<(T, U)> {
    pub fn unzip(self) -> (CompatPair<T>, CompatPair<U>) {
        let Self {
            reader: (r1, r2),
            writer: (w1, w2),
        } = self;
        (
            CompatPair {
                reader: r1,
                writer: w1,
            },
            CompatPair {
                reader: r2,
                writer: w2,
            },
        )
    }
}

impl<T: Eq + std::fmt::Debug> CompatPair<T> {
    pub fn unwrap_eq(self) -> T {
        let Self { reader, writer } = self;
        assert_eq!(reader, writer);
        reader
    }

    pub fn try_unwrap_eq(self) -> Result<T, Self> {
        let Self { reader, writer } = self;
        if reader == writer {
            Ok(reader)
        } else {
            Err(Self { reader, writer })
        }
    }
}

impl<'a, T: CheckCompat> CompatPair<&'a T> {
    #[inline]
    pub fn check(self, cx: CompatPair<T::Context<'_>>, log: &mut CompatLog) {
        CheckCompat::check_compat(self, cx, log);
    }
}

impl<T: Iterator> CompatPair<T> {
    pub fn iter(self) -> impl Iterator<Item = Side<T::Item>> {
        let Self { reader, writer } = self;
        reader.map(Side::Reader).chain(writer.map(Side::Writer))
    }
}

impl<'a, K: Eq + Hash, V> CompatPair<&'a HashMap<K, V>> {
    pub fn iter_joined(self) -> impl Iterator<Item = (&'a K, SideInclusive<&'a V>)> {
        let Self { reader, writer } = self;

        reader
            .iter()
            .map(|(key, reader)| {
                (
                    key,
                    writer.get(key).map_or_else(
                        || Side::Reader(reader).into(),
                        |writer| CompatPair { reader, writer }.into(),
                    ),
                )
            })
            .chain(writer.iter().filter_map(|(key, writer)| {
                (!reader.contains_key(key)).then_some((key, Side::Writer(writer).into()))
            }))
    }
}

impl<'a, K: Eq + Hash, V: CheckCompat> CompatPair<&'a HashMap<K, V>> {
    pub fn check_joined<'b, E>(
        self,
        extra: &'b CompatPair<E>,
        log: &mut CompatLog,
        cx: impl Fn(&'b E, &'b K) -> V::Context<'b>,
        missing_val: impl Fn(&K, Side<&V>, &mut CompatLog),
    ) where
        'a: 'b,
    {
        for (key, side) in self.iter_joined() {
            match side {
                SideInclusive::One(s) => missing_val(key, s, log),
                SideInclusive::Both(pair) => pair.check(
                    CompatPair {
                        reader: cx(&extra.reader, key),
                        writer: cx(&extra.writer, key),
                    },
                    log,
                ),
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Side<T = ()> {
    Reader(T),
    Writer(T),
}

impl<T> Side<T> {
    #[inline]
    pub const fn kind(&self) -> Side {
        match self {
            Self::Reader(_) => Side::Reader(()),
            Self::Writer(_) => Side::Writer(()),
        }
    }

    #[inline]
    pub const fn as_ref(&self) -> Side<&T> {
        match self {
            Self::Reader(r) => Side::Reader(r),
            Self::Writer(w) => Side::Writer(w),
        }
    }

    #[inline]
    pub const fn display(&self) -> Display<Side<T>> { Display(self) }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Side<U> {
        match self {
            Self::Reader(r) => Side::Reader(f(r)),
            Self::Writer(w) => Side::Writer(f(w)),
        }
    }

    #[inline]
    pub fn inner(self) -> T {
        match self {
            Self::Reader(v) | Self::Writer(v) => v,
        }
    }

    #[inline]
    pub fn split(self) -> (Side, T) {
        let side = self.kind();
        let inner = self.inner();
        (side, inner)
    }

    pub fn visit(self, kind: Side) -> Option<T> {
        match (self, kind) {
            (Self::Reader(v), Side::Reader(())) | (Self::Writer(v), Side::Writer(())) => Some(v),
            (Self::Reader(_), Side::Writer(())) | (Self::Writer(_), Side::Reader(())) => None,
        }
    }
}

impl<'a, T: Copy> Side<&'a T> {
    #[inline]
    pub const fn copied(&self) -> Side<T> {
        match self {
            Self::Reader(&r) => Side::Reader(r),
            Self::Writer(&w) => Side::Writer(w),
        }
    }
}

impl<T> Side<Option<T>> {
    #[inline]
    pub fn transpose(self) -> Option<Side<T>> {
        match self {
            Self::Reader(Some(r)) => Some(Side::Reader(r)),
            Self::Writer(Some(w)) => Some(Side::Writer(w)),
            Self::Reader(None) | Self::Writer(None) => None,
        }
    }
}

impl Side {
    #[inline]
    pub const fn then<T>(self, val: T) -> Side<T> {
        match self {
            Self::Reader(()) => Side::Reader(val),
            Self::Writer(()) => Side::Writer(val),
        }
    }

    #[inline]
    pub fn project<T>(self, pair: CompatPair<T>) -> Side<T> { self.then(pair.visit(self)) }

    #[inline]
    pub const fn pretty(self) -> &'static str {
        match self {
            Self::Reader(()) => "reader",
            Self::Writer(()) => "writer",
        }
    }

    #[inline]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Reader(()) => Self::Writer(()),
            Self::Writer(()) => Self::Reader(()),
        }
    }
}

impl<T> Deref for Side<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        match self {
            Self::Reader(r) => r,
            Self::Writer(w) => w,
        }
    }
}

impl<T> DerefMut for Side<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        match self {
            Self::Reader(r) => r,
            Self::Writer(w) => w,
        }
    }
}

pub struct NoneError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SideInclusive<T = ()> {
    One(Side<T>),
    Both(CompatPair<T>),
}

impl<T> TryFrom<CompatPair<Option<T>>> for SideInclusive<T> {
    type Error = NoneError;

    fn try_from(pair: CompatPair<Option<T>>) -> Result<Self, Self::Error> {
        let CompatPair { reader, writer } = pair;

        Ok(match (reader, writer) {
            (None, None) => return Err(NoneError),
            (Some(r), None) => Self::One(Side::Reader(r)),
            (None, Some(w)) => Self::One(Side::Writer(w)),
            (Some(reader), Some(writer)) => Self::Both(CompatPair { reader, writer }),
        })
    }
}

impl<T> From<Side<T>> for SideInclusive<T> {
    #[inline]
    fn from(val: Side<T>) -> Self { Self::One(val) }
}

impl<T> From<CompatPair<T>> for SideInclusive<T> {
    #[inline]
    fn from(val: CompatPair<T>) -> Self { Self::Both(val) }
}

impl<T> SideInclusive<T> {
    #[inline]
    pub fn as_ref(&self) -> SideInclusive<&T> {
        match self {
            Self::One(s) => SideInclusive::One(s.as_ref()),
            Self::Both(p) => SideInclusive::Both(p.as_ref()),
        }
    }

    #[inline]
    pub const fn display(&self) -> Display<SideInclusive<T>> { Display(self) }

    pub fn map<U>(self, f: impl Fn(T) -> U) -> SideInclusive<U> {
        match self {
            Self::One(s) => SideInclusive::One(s.map(f)),
            Self::Both(p) => SideInclusive::Both(p.map(f)),
        }
    }
}

#[repr(transparent)]
pub struct Display<'a, T>(&'a T);

impl<'a, T: fmt::Display> fmt::Display for Display<'a, CompatPair<T>> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let CompatPair { reader, writer } = self.0;
        write!(f, "{reader} in reader, {writer} in writer")
    }
}

impl<'a, T: fmt::Debug> fmt::Debug for Display<'a, CompatPair<T>> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let CompatPair { reader, writer } = self.0;
        write!(f, "{reader:?} in reader, {writer:?} in writer")
    }
}

impl<'a, T: fmt::Display> fmt::Display for Display<'a, Side<T>> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (side, val) = self.0.as_ref().split();
        write!(f, "{val} in {}", side.pretty())
    }
}

impl<'a, T: fmt::Debug> fmt::Debug for Display<'a, Side<T>> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (side, val) = self.0.as_ref().split();
        write!(f, "{val:?} in {}", side.pretty())
    }
}

impl<'a, T: fmt::Display> fmt::Display for Display<'a, SideInclusive<T>> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            SideInclusive::One(s) => write!(f, "{}", s.display()),
            SideInclusive::Both(p) => write!(f, "{}", p.display()),
        }
    }
}

impl<'a, T: fmt::Debug> fmt::Debug for Display<'a, SideInclusive<T>> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            SideInclusive::One(s) => write!(f, "{:?}", s.display()),
            SideInclusive::Both(p) => write!(f, "{:?}", p.display()),
        }
    }
}
