use std::collections::HashMap;

use super::{reserved::ReservedMap, ty::TypeContext};
use crate::{
    check_compat::{CheckCompat, CompatResult},
    compat_pair::{CompatPair, Side, SideInclusive},
};

pub struct RecordContext<'a> {
    pub ty: &'a TypeContext<'a>,
    pub id: i32,
}

pub trait RecordValue<'a>: CheckCompat<Context<'a> = RecordContext<'a>> {
    type Names: Iterator<Item = &'a str> + ExactSizeIterator;

    fn names(&'a self) -> Self::Names;

    // self (with id ID) only exists on the side given - ID should be reserved on the other side
    fn missing_id(&self, cx: &CompatPair<TypeContext<'a>>, id: Side<i32>) -> CompatResult;

    // Shared name has conflicting IDs
    fn id_conflict(
        cx: &CompatPair<TypeContext<'a>>,
        name: &str,
        ids: CompatPair<i32>,
    ) -> CompatResult;

    // Shared name only has an ID on the side given - name should be reserved on the other side
    fn missing_name(cx: &CompatPair<TypeContext<'a>>, name: &str, id: Side<i32>) -> CompatResult;

    fn check_extra(
        ck: CompatPair<std::collections::hash_map::Iter<'_, i32, Self>>,
        cx: &CompatPair<TypeContext<'a>>,
    ) -> CompatResult
    where
        Self: Sized;
}

#[derive(Debug)]
pub struct Record<T> {
    numbers: HashMap<i32, T>,
    /// `None` indicates a reserved name
    names: HashMap<String, Option<i32>>,
    reserved: ReservedMap,
    internal: bool,
}

impl<T: for<'a> RecordValue<'a>> Record<T> {
    pub fn new<R: IntoIterator<Item = String>>(
        numbers: HashMap<i32, T>,
        reserved: ReservedMap,
        reserved_names: R,
        internal: bool,
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
        }
    }

    #[inline]
    pub const fn internal(&self) -> bool { self.internal }
}

impl<T: for<'a> RecordValue<'a>> CheckCompat for Record<T> {
    type Context<'a> = TypeContext<'a>;

    fn check_compat(ck: CompatPair<&'_ Self>, cx: CompatPair<Self::Context<'_>>) -> CompatResult {
        // TODO: convert pair tomfuckery into map-unzip ops

        let (ck, reserved) = ck
            .map(
                |Record {
                     numbers,
                     names,
                     reserved,
                     internal: _,
                 }| ((numbers, names), reserved),
            )
            .unzip();
        let (numbers, names) = ck.unzip();

        numbers.check_joined(
            &cx,
            |ty, &id| RecordContext { ty, id },
            |&k, v| {
                if reserved.visit(v.kind()).contains(k.into()) {
                    Ok(())
                } else {
                    let id = v.then(k);
                    v.missing_id(&cx, id)
                }
            },
        )?;

        for (key, side) in names.iter_joined() {
            match side {
                SideInclusive::Both { reader, writer } => match (*reader, *writer) {
                    (Some(reader), Some(writer)) if reader != writer => {
                        T::id_conflict(&cx, key, CompatPair::new(reader, writer))
                    },
                    (..) => Ok(()),
                },
                SideInclusive::Reader(&Some(r)) => T::missing_name(&cx, key, Side::Reader(r)),
                SideInclusive::Writer(&Some(w)) => T::missing_name(&cx, key, Side::Writer(w)),
                SideInclusive::Reader(None) | SideInclusive::Writer(None) => Ok(()),
            }?;
        }

        T::check_extra(numbers.map(HashMap::iter), &cx)
    }
}
