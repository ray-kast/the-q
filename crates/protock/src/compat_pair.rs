use std::{
    collections::HashMap,
    hash::Hash,
    ops::{Deref, DerefMut},
};

use tracing::Value;

use crate::check_compat::{CheckCompat, CompatResult};

pub trait SpanExt {
    fn record_pair<V: Value>(&self, pair: &CompatPair<V>) -> &Self;
}

impl SpanExt for tracing::Span {
    fn record_pair<V: Value>(&self, pair: &CompatPair<V>) -> &Self {
        self.record("reader", &pair.reader)
            .record("writer", &pair.writer)
    }
}

#[derive(Clone, Copy)]
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

    pub fn into_inner(self) -> (T, T) {
        let Self { reader, writer } = self;
        (reader, writer)
    }

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
    pub fn check(self, cx: CompatPair<T::Context<'_>>) -> CompatResult {
        CheckCompat::check_compat(self, cx)
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
                        || SideInclusive::Reader(reader),
                        |writer| SideInclusive::Both { reader, writer },
                    ),
                )
            })
            .chain(writer.iter().filter_map(|(key, writer)| {
                (!reader.contains_key(key)).then_some((key, SideInclusive::Writer(writer)))
            }))
    }
}

impl<'a, K: Eq + Hash, V: CheckCompat> CompatPair<&'a HashMap<K, V>> {
    pub fn check_joined<'b, E>(
        self,
        extra: &'b CompatPair<E>,
        cx: impl Fn(&'b E, &'b K) -> V::Context<'b>,
        missing_val: impl Fn(&K, Side<&V>) -> CompatResult,
    ) -> CompatResult
    where
        'a: 'b,
    {
        for (key, side) in self.iter_joined() {
            match side {
                SideInclusive::Both { reader, writer } => {
                    CompatPair { reader, writer }.check(CompatPair {
                        reader: cx(&extra.reader, key),
                        writer: cx(&extra.writer, key),
                    })?;
                },
                SideInclusive::Reader(r) => missing_val(key, Side::Reader(r))?,
                SideInclusive::Writer(w) => missing_val(key, Side::Writer(w))?,
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side<T = ()> {
    Reader(T),
    Writer(T),
}

impl<T> Side<T> {
    pub const fn then<U>(&self, val: U) -> Side<U> {
        match self {
            Self::Reader(_) => Side::Reader(val),
            Self::Writer(_) => Side::Writer(val),
        }
    }

    #[inline]
    pub const fn kind(&self) -> Side { self.then(()) }

    #[inline]
    pub const fn as_ref(&self) -> Side<&T> {
        match self {
            Self::Reader(r) => Side::Reader(r),
            Self::Writer(w) => Side::Writer(w),
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

impl Side {
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

    #[inline]
    pub const fn reader(self) -> bool { matches!(self, Self::Reader(())) }

    #[inline]
    pub const fn writer(self) -> bool { matches!(self, Self::Writer(())) }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SideInclusive<T = ()> {
    // TODO: use Side here?
    Reader(T),
    Writer(T),
    // TODO: use CompatPair here?
    Both { reader: T, writer: T },
}

impl<T> From<Side<T>> for SideInclusive<T> {
    #[inline]
    fn from(val: Side<T>) -> Self {
        match val {
            Side::Reader(r) => Self::Reader(r),
            Side::Writer(w) => Self::Writer(w),
        }
    }
}

impl<T> From<CompatPair<T>> for SideInclusive<T> {
    #[inline]
    fn from(val: CompatPair<T>) -> Self {
        let CompatPair { reader, writer } = val;
        Self::Both { reader, writer }
    }
}

impl<T> SideInclusive<T> {
    pub fn map<U>(self, f: impl Fn(T) -> U) -> SideInclusive<U> {
        match self {
            Self::Reader(r) => SideInclusive::Reader(f(r)),
            Self::Writer(w) => SideInclusive::Writer(f(w)),
            Self::Both { reader, writer } => SideInclusive::Both {
                reader: f(reader),
                writer: f(writer),
            },
        }
    }
}
