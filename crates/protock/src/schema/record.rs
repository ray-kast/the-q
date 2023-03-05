use std::collections::HashMap;

use shrec::range_map::RangeMap;

use super::ty::TypeContext;
use crate::{
    check_compat::{CheckCompat, CompatLog},
    compat_pair::{CompatPair, Side, SideInclusive},
};

pub struct RecordContext<'a> {
    pub ty: &'a TypeContext<'a>,
    pub id: i32,
}

pub trait RecordExtra {
    type Extra;
}

pub trait RecordValue<'a>: RecordExtra + CheckCompat<Context<'a> = RecordContext<'a>> {
    type Names: Iterator<Item = &'a str> + ExactSizeIterator;

    fn names(&'a self) -> Self::Names;

    // self (with id ID) only exists on the side given - ID should be reserved on the other side
    fn missing_id(&self, cx: &CompatPair<TypeContext<'a>>, id: Side<i32>, log: &mut CompatLog);

    // Shared name has conflicting IDs
    fn id_conflict(
        cx: &CompatPair<TypeContext<'a>>,
        name: &str,
        ids: CompatPair<i32>,
        log: &mut CompatLog,
    );

    // Shared name only has an ID on the side given - name should be reserved on the other side
    fn missing_name(
        cx: &CompatPair<TypeContext<'a>>,
        name: &str,
        id: Side<i32>,
        log: &mut CompatLog,
    );

    fn check_extra(
        ck: CompatPair<std::collections::hash_map::Iter<'_, i32, Self>>,
        cx: &CompatPair<TypeContext<'a>>,
        extra: CompatPair<&Self::Extra>,
        log: &mut CompatLog,
    ) where
        Self: Sized;
}

#[derive(Debug)]
pub struct Record<T: RecordExtra> {
    numbers: HashMap<i32, T>,
    /// `None` indicates a reserved name
    names: HashMap<String, Option<i32>>,
    reserved: RangeMap<i64>,
    internal: bool,
    extra: T::Extra,
}

impl<T: for<'a> RecordValue<'a>> Record<T> {
    pub fn new<R: IntoIterator<Item = String>>(
        numbers: HashMap<i32, T>,
        reserved: RangeMap<i64>,
        reserved_names: R,
        internal: bool,
        extra: T::Extra,
    ) -> Self
    where
        R::IntoIter: ExactSizeIterator,
    {
        let reserved_names = reserved_names.into_iter();
        let reserved_name_len = reserved_names.len();
        let names: HashMap<_, _> = numbers
            .iter()
            .flat_map(|(i, v)| v.names().zip(std::iter::repeat(*i)))
            .map(|(v, i)| (v.into(), Some(i)))
            .chain(reserved_names.map(|r| (r, None)))
            .collect();

        assert_eq!(
            names.len(),
            numbers.values().map(|v| v.names().len()).sum::<usize>() + reserved_name_len
        );

        Self {
            numbers,
            names,
            reserved,
            internal,
            extra,
        }
    }

    #[inline]
    pub const fn internal(&self) -> bool { self.internal }
}

impl<T: for<'a> RecordValue<'a>> CheckCompat for Record<T> {
    type Context<'b> = TypeContext<'b>;

    fn check_compat(
        ck: CompatPair<&'_ Self>,
        cx: CompatPair<Self::Context<'_>>,
        log: &mut CompatLog,
    ) {
        // TODO: convert pair tomfuckery into map-unzip ops

        let ck = ck.map(
            |Record {
                 numbers,
                 names,
                 reserved,
                 internal: _,
                 extra,
             }| (((numbers, names), reserved), extra),
        );
        let (ck, extra) = ck.unzip();
        let (ck, reserved) = ck.unzip();
        let (numbers, names) = ck.unzip();

        numbers.check_joined(
            &cx,
            log,
            |ty, &id| RecordContext { ty, id },
            |&k, v, log| {
                if !reserved.visit(v.kind()).contains(&k.into()) {
                    v.missing_id(&cx, v.kind().then(k), log);
                }
            },
        );

        for (key, side) in names.iter_joined() {
            match side {
                SideInclusive::One(s) => {
                    if let Some(s) = s.copied().transpose() {
                        T::missing_name(&cx, key, s, log);
                    }
                },
                SideInclusive::Both(pair) => match pair.copied().into_inner() {
                    (Some(reader), Some(writer)) if reader != writer => {
                        T::id_conflict(&cx, key, CompatPair::new(reader, writer), log);
                    },
                    (..) => (),
                },
            }
        }

        T::check_extra(numbers.map(HashMap::iter), &cx, extra, log);
    }
}
