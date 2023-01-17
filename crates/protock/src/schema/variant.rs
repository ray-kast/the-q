use std::{borrow::Cow, collections::BTreeSet};

use super::{
    record::{RecordContext, RecordValue},
    ty::TypeContext,
};
use crate::{
    check_compat::{CheckCompat, CompatError, CompatResult},
    compat_pair::{CompatPair, Side},
};

#[derive(Debug, Default)]
#[repr(transparent)]
pub struct Variant(BTreeSet<String>);

impl Variant {
    #[inline]
    pub const fn new(names: BTreeSet<String>) -> Self { Self(names) }

    fn name_pretty(&self, compact: bool) -> Cow<'_, str> {
        let mut it = self.0.iter();
        let mut ret = Cow::Borrowed(&**it.next().unwrap());

        for part in it {
            let s = ret.to_mut();
            s.push_str(if compact { "|" } else { ", " });
            s.push_str(part);
        }

        ret
    }
}

impl CheckCompat for Variant {
    type Context<'a> = RecordContext<'a>;

    fn check_compat(ck: CompatPair<&'_ Self>, cx: CompatPair<Self::Context<'_>>) -> CompatResult {
        let qual_names = ck
            .zip(cx.as_ref())
            .map(|(v, c)| c.ty.kind.type_name().member(v.name_pretty(true)));

        let id = cx.map(|c| c.id).unwrap_eq();

        let ck = ck.map(|v| &v.0);
        let Err(_names) = ck.try_unwrap_eq() else { return Ok(()) };

        match ck.map(BTreeSet::len).into_inner() {
            (0, _) | (_, 0) => unreachable!(),
            (1, 1) => {
                CompatError::new(
                    qual_names.map(|n| n.to_owned()).into(),
                    format!("Enum variant name mismatch for value {id}"),
                )
                .warn();
            },
            (..) => {
                let (reader, writer) = ck.into_inner();
                let mut rd_only = reader.difference(writer).peekable();
                let mut wr_only = writer.difference(reader).peekable();

                if rd_only.peek().is_some() && wr_only.peek().is_some() {
                    let mut s = format!("Mismatched enum alias(es) for value {id}");
                    let mut any = false;

                    for name in rd_only {
                        if any {
                            s.push_str(", ");
                        } else {
                            any = true;
                            s.push_str(": ");
                        }

                        s.push_str(name);
                    }

                    if any {
                        s.push_str(" for reader");
                    }

                    let prev_any = any;
                    let mut any = false;

                    for name in wr_only {
                        if any {
                            s.push_str(", ");
                        } else {
                            any = true;
                            s.push_str(if prev_any { "; " } else { ": " });
                        }

                        s.push_str(name);
                    }

                    if any {
                        s.push_str(" for writer");
                    }

                    CompatError::new(qual_names.map(|n| n.to_owned()).into(), s).warn();
                }
            },
        }

        Ok(())
    }
}

impl<'a> RecordValue<'a> for Variant {
    type Names = std::iter::Map<std::collections::btree_set::Iter<'a, String>, fn(&String) -> &str>;

    fn names(&'a self) -> Self::Names { self.0.iter().map(AsRef::as_ref) }

    fn missing_id(&self, cx: &CompatPair<TypeContext<'a>>, id: Side<i32>) -> CompatResult {
        let (side, id) = id.split();
        CompatError::new(
            cx.as_ref().map(|c| c.kind.to_owned()).into(),
            format!(
                "Enum variant(s) {} (value {id}) missing and not reserved on {}",
                self.name_pretty(false),
                side.opposite().pretty()
            ),
        )
        .warn();

        Ok(())
    }

    fn id_conflict(
        cx: &CompatPair<TypeContext<'a>>,
        name: &str,
        ids: CompatPair<i32>,
    ) -> CompatResult {
        let (reader, writer) = ids.into_inner();
        Err(CompatError::new(
            cx.as_ref().map(|c| c.kind.to_owned()).into(),
            format!(
                "Enum variant {name} has value {} on reader and {} on writer",
                reader, writer
            ),
        ))
    }

    fn missing_name(cx: &CompatPair<TypeContext<'a>>, name: &str, id: Side<i32>) -> CompatResult {
        let (side, id) = id.split();
        CompatError::new(
            cx.as_ref().map(|c| c.kind.to_owned()).into(),
            format!(
                "Enum variant name {name} (ID {id} on {}) missing and not reserved on {}",
                side.pretty(),
                side.opposite().pretty()
            ),
        )
        .warn();

        Ok(())
    }

    fn check_extra(
        ck: CompatPair<std::collections::hash_map::Iter<'_, i32, Self>>,
        cx: &CompatPair<TypeContext<'a>>,
    ) -> CompatResult
    where
        Self: Sized,
    {
        for side in ck.iter() {
            let (side, (value, var)) = side.split();
            CompatError::new(
                side.then(
                    cx.as_ref()
                        .visit(side)
                        .kind
                        .type_name()
                        .member(var.name_pretty(true))
                        .to_owned(),
                )
                .into(),
                format!("Negative enum value {value}"),
            )
            .warn();
        }

        Ok(())
    }
}
