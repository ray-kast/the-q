use std::{
    collections::HashMap,
    fmt,
    hash::Hash,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use crate::check_compat::{CheckCompat, CompatLog};

mod private {
    pub trait Sealed {}

    impl Sealed for super::Covariant {}
    impl Sealed for super::Contravariant {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Covariant;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Contravariant;

pub trait Variance:
    private::Sealed + fmt::Debug + Clone + Copy + Eq + Ord + Hash + 'static
{
    type FLIPPED: Variance;
}

impl Variance for Covariant {
    type FLIPPED = Contravariant;
}

impl Variance for Contravariant {
    type FLIPPED = Covariant;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompatPair<T, V: Variance = Covariant> {
    reader: T,
    writer: T,
    _p: PhantomData<V>,
}

impl<V: Variance> From<()> for CompatPair<(), V> {
    fn from((): ()) -> Self {
        Self {
            reader: (),
            writer: (),
            _p: PhantomData,
        }
    }
}

impl<T> CompatPair<T, Covariant> {
    #[inline]
    pub const fn new(reader: T, writer: T) -> Self {
        Self {
            reader,
            writer,
            _p: PhantomData,
        }
    }
}

impl<T, V: Variance> CompatPair<T, V> {
    #[inline]
    pub const fn new_var(reader: T, writer: T) -> Self {
        Self {
            reader,
            writer,
            _p: PhantomData,
        }
    }

    #[inline]
    pub unsafe fn force_covar(self) -> CompatPair<T> {
        let Self {
            reader,
            writer,
            _p: PhantomData,
        } = self;
        CompatPair {
            reader,
            writer,
            _p: PhantomData,
        }
    }

    #[inline]
    pub fn as_ref(&self) -> CompatPair<&T, V> {
        let Self {
            reader,
            writer,
            _p: PhantomData,
        } = self;
        CompatPair {
            reader,
            writer,
            _p: PhantomData,
        }
    }

    #[inline]
    pub const fn display(&self) -> Display<Self> { Display(self) }

    pub fn into_inner(self) -> (T, T) {
        let Self {
            reader,
            writer,
            _p: PhantomData,
        } = self;
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
        let Self {
            reader,
            writer,
            _p: PhantomData,
        } = self;
        f(Side::Reader(reader));
        f(Side::Writer(writer));
    }

    pub fn map<U>(self, f: impl Fn(T) -> U) -> CompatPair<U, V> {
        let Self {
            reader,
            writer,
            _p: PhantomData,
        } = self;
        CompatPair {
            reader: f(reader),
            writer: f(writer),
            _p: PhantomData,
        }
    }

    // TODO: try_trait_v2 wen eta son

    pub fn try_map<U, E>(self, f: impl Fn(T) -> Result<U, E>) -> Result<CompatPair<U, V>, Side<E>> {
        let Self {
            reader,
            writer,
            _p: PhantomData,
        } = self;
        Ok(CompatPair {
            reader: f(reader).map_err(Side::Reader)?,
            writer: f(writer).map_err(Side::Writer)?,
            _p: PhantomData,
        })
    }

    pub fn filter_map<U>(self, f: impl Fn(T) -> Option<U>) -> Option<CompatPair<U, V>> {
        let Self {
            reader,
            writer,
            _p: PhantomData,
        } = self;
        Some(CompatPair {
            reader: f(reader)?,
            writer: f(writer)?,
            _p: PhantomData,
        })
    }

    pub fn zip<U>(self, other: CompatPair<U, V>) -> CompatPair<(T, U), V> {
        let Self {
            reader: r1,
            writer: w1,
            _p: PhantomData,
        } = self;
        let CompatPair {
            reader: r2,
            writer: w2,
            _p: PhantomData,
        } = other;
        CompatPair {
            reader: (r1, r2),
            writer: (w1, w2),
            _p: PhantomData,
        }
    }

    #[inline]
    pub fn flip(self) -> CompatPair<T, V::FLIPPED> {
        let Self {
            reader,
            writer,
            _p: PhantomData,
        } = self;
        CompatPair {
            reader: writer,
            writer: reader,
            _p: PhantomData,
        }
    }
}

impl<T: Copy, V: Variance> CompatPair<&T, V> {
    #[inline]
    pub const fn copied(self) -> CompatPair<T, V> {
        let Self {
            reader: &reader,
            writer: &writer,
            _p: PhantomData,
        } = self;
        CompatPair {
            reader,
            writer,
            _p: PhantomData,
        }
    }
}

impl<T, U, V: Variance> CompatPair<(T, U), V> {
    pub fn unzip(self) -> (CompatPair<T, V>, CompatPair<U, V>) {
        let Self {
            reader: (r1, r2),
            writer: (w1, w2),
            _p: PhantomData,
        } = self;
        (
            CompatPair {
                reader: r1,
                writer: w1,
                _p: PhantomData,
            },
            CompatPair {
                reader: r2,
                writer: w2,
                _p: PhantomData,
            },
        )
    }
}

impl<T: Eq + std::fmt::Debug, V: Variance> CompatPair<T, V> {
    pub fn unwrap_eq(self) -> T {
        let Self {
            reader,
            writer,
            _p: PhantomData,
        } = self;
        assert_eq!(reader, writer);
        reader
    }
}

impl<T: Eq, V: Variance> CompatPair<T, V> {
    pub fn try_unwrap_eq(self) -> Result<T, Self> {
        let Self {
            reader,
            writer,
            _p: PhantomData,
        } = self;
        if reader == writer {
            Ok(reader)
        } else {
            Err(Self {
                reader,
                writer,
                _p: PhantomData,
            })
        }
    }
}

impl<T: CheckCompat, V: Variance> CompatPair<&T, V> {
    #[inline]
    pub fn check(self, cx: CompatPair<T::Context<'_>, V>, log: &mut CompatLog) {
        CheckCompat::check_compat(self, cx, log);
    }
}

impl<T: Iterator, V: Variance> CompatPair<T, V> {
    pub fn iter(self) -> impl Iterator<Item = Side<T::Item>> {
        let Self {
            reader,
            writer,
            _p: PhantomData,
        } = self;
        reader.map(Side::Reader).chain(writer.map(Side::Writer))
    }
}

impl<'a, K: Eq + Hash, W, V: Variance> CompatPair<&'a HashMap<K, W>, V> {
    pub fn iter_joined(self) -> impl Iterator<Item = (&'a K, SideInclusive<&'a W, V>)> {
        let Self {
            reader,
            writer,
            _p: PhantomData,
        } = self;

        reader
            .iter()
            .map(|(key, reader)| {
                (
                    key,
                    writer.get(key).map_or_else(
                        || Side::Reader(reader).into(),
                        |writer| {
                            CompatPair {
                                reader,
                                writer,
                                _p: PhantomData,
                            }
                            .into()
                        },
                    ),
                )
            })
            .chain(writer.iter().filter_map(|(key, writer)| {
                (!reader.contains_key(key)).then_some((key, Side::Writer(writer).into()))
            }))
    }
}

impl<'a, K: Eq + Hash, W: CheckCompat, V: Variance> CompatPair<&'a HashMap<K, W>, V> {
    pub fn check_joined<'b, E>(
        self,
        extra: &'b CompatPair<E, V>,
        log: &mut CompatLog,
        cx: impl Fn(&'b E, &'b K) -> W::Context<'b>,
        missing_val: impl Fn(&K, Side<&W>, &mut CompatLog),
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
                        _p: PhantomData,
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

impl<T: Copy> Side<&T> {
    #[inline]
    pub const fn copied(&self) -> Side<T> {
        match *self {
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
    pub fn project<T, V: Variance>(self, pair: CompatPair<T, V>) -> Side<T> {
        self.then(pair.visit(self))
    }

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
pub enum SideInclusive<T = (), V: Variance = Covariant> {
    One(Side<T>),
    Both(CompatPair<T, V>),
}

impl<T, V: Variance> TryFrom<CompatPair<Option<T>, V>> for SideInclusive<T, V> {
    type Error = NoneError;

    fn try_from(pair: CompatPair<Option<T>, V>) -> Result<Self, Self::Error> {
        let CompatPair {
            reader,
            writer,
            _p: PhantomData,
        } = pair;

        Ok(match (reader, writer) {
            (None, None) => return Err(NoneError),
            (Some(r), None) => Self::One(Side::Reader(r)),
            (None, Some(w)) => Self::One(Side::Writer(w)),
            (Some(reader), Some(writer)) => Self::Both(CompatPair {
                reader,
                writer,
                _p: PhantomData,
            }),
        })
    }
}

impl<T, V: Variance> From<Side<T>> for SideInclusive<T, V> {
    #[inline]
    fn from(val: Side<T>) -> Self { Self::One(val) }
}

impl<T, V: Variance> From<CompatPair<T, V>> for SideInclusive<T, V> {
    #[inline]
    fn from(val: CompatPair<T, V>) -> Self { Self::Both(val) }
}

impl<T, V: Variance> SideInclusive<T, V> {
    #[inline]
    pub unsafe fn force_covar(self) -> SideInclusive<T> {
        match self {
            Self::One(s) => SideInclusive::One(s),
            Self::Both(p) => SideInclusive::Both(unsafe { p.force_covar() }),
        }
    }

    #[inline]
    pub fn as_ref(&self) -> SideInclusive<&T, V> {
        match self {
            Self::One(s) => SideInclusive::One(s.as_ref()),
            Self::Both(p) => SideInclusive::Both(p.as_ref()),
        }
    }

    #[inline]
    pub const fn display(&self) -> Display<SideInclusive<T, V>> { Display(self) }

    pub fn map<U>(self, f: impl Fn(T) -> U) -> SideInclusive<U, V> {
        match self {
            Self::One(s) => SideInclusive::One(s.map(f)),
            Self::Both(p) => SideInclusive::Both(p.map(f)),
        }
    }
}

#[repr(transparent)]
pub struct Display<'a, T>(&'a T);

impl<T: fmt::Display, V: Variance> fmt::Display for Display<'_, CompatPair<T, V>> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let CompatPair {
            reader,
            writer,
            _p: PhantomData,
        } = self.0;
        write!(f, "{reader} in reader, {writer} in writer")
    }
}

impl<T: fmt::Debug, V: Variance> fmt::Debug for Display<'_, CompatPair<T, V>> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let CompatPair {
            reader,
            writer,
            _p: PhantomData,
        } = self.0;
        write!(f, "{reader:?} in reader, {writer:?} in writer")
    }
}

impl<T: fmt::Display> fmt::Display for Display<'_, Side<T>> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (side, val) = self.0.as_ref().split();
        write!(f, "{val} in {}", side.pretty())
    }
}

impl<T: fmt::Debug> fmt::Debug for Display<'_, Side<T>> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (side, val) = self.0.as_ref().split();
        write!(f, "{val:?} in {}", side.pretty())
    }
}

impl<T: fmt::Display, V: Variance> fmt::Display for Display<'_, SideInclusive<T, V>> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            SideInclusive::One(s) => write!(f, "{}", s.display()),
            SideInclusive::Both(p) => write!(f, "{}", p.display()),
        }
    }
}

impl<T: fmt::Debug, V: Variance> fmt::Debug for Display<'_, SideInclusive<T, V>> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            SideInclusive::One(s) => write!(f, "{:?}", s.display()),
            SideInclusive::Both(p) => write!(f, "{:?}", p.display()),
        }
    }
}
