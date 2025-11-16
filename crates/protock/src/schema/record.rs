use std::collections::HashMap;

use mid_tools::range_set::RangeSet;

use super::ty::TypeContext;
use crate::{
    check_compat::{CheckCompat, CompatLog},
    compat_pair::{CompatPair, Side, SideInclusive, Variance},
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
    fn missing_id<V: Variance>(
        &self,
        cx: &CompatPair<TypeContext<'a>, V>,
        id: Side<i32>,
        log: &mut CompatLog,
    );

    // Shared name has conflicting IDs
    fn id_conflict<V: Variance>(
        cx: &CompatPair<TypeContext<'a>, V>,
        name: &str,
        ids: CompatPair<i32, V>,
        log: &mut CompatLog,
    );

    // Shared name only has an ID on the side given - name should be reserved on the other side
    fn missing_name<V: Variance>(
        cx: &CompatPair<TypeContext<'a>, V>,
        name: &str,
        id: Side<i32>,
        log: &mut CompatLog,
    );

    fn check_extra<V: Variance>(
        ck: CompatPair<std::collections::hash_map::Iter<'_, i32, Self>, V>,
        cx: &CompatPair<TypeContext<'a>, V>,
        extra: CompatPair<&Self::Extra, V>,
        log: &mut CompatLog,
    ) where
        Self: Sized;
}

#[derive(Debug)]
pub struct Record<T: RecordExtra> {
    numbers: HashMap<i32, T>,
    /// `None` indicates a reserved name
    names: HashMap<String, Option<i32>>,
    reserved: RangeSet<i64>,
    internal: bool,
    extra: T::Extra,
}

impl<T: for<'a> RecordValue<'a>> Record<T> {
    pub fn new<R: IntoIterator<Item = String, IntoIter: ExactSizeIterator>>(
        numbers: HashMap<i32, T>,
        reserved: RangeSet<i64>,
        reserved_names: R,
        internal: bool,
        extra: T::Extra,
    ) -> Self {
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

    fn check_compat<V: Variance>(
        ck: CompatPair<&'_ Self, V>,
        cx: CompatPair<Self::Context<'_>, V>,
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
                        T::id_conflict(&cx, key, CompatPair::new_var(reader, writer), log);
                    },
                    (..) => (),
                },
            }
        }

        T::check_extra(numbers.map(HashMap::iter), &cx, extra, log);
    }
}
