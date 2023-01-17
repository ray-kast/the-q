use std::collections::{BTreeSet, HashMap, HashSet};

use super::{
    field_kind::FieldKind,
    field_type::{FieldType, FieldTypeContext},
    primitive::{VarIntMode, WireType},
    record::{RecordContext, RecordValue},
    ty::{TypeCheckKind, TypeContext},
};
use crate::{
    check_compat::{CheckCompat, CompatError, CompatResult},
    compat_pair::{CompatPair, Side},
    union_find::UnionFind,
};

#[derive(Debug)]
pub struct Field {
    name: String,
    ty: FieldType,
    kind: FieldKind,
    oneof: Option<i32>,
}

impl Field {
    #[inline]
    pub const fn new(name: String, ty: FieldType, kind: FieldKind, oneof: Option<i32>) -> Self {
        Self {
            name,
            ty,
            kind,
            oneof,
        }
    }

    fn warn_non_zigzag(&self, ctx: &TypeContext<'_>, side: Side) {
        let wire = self
            .ty
            .wire_format(self.kind, |n| ctx.types.get(n).unwrap());

        if wire == WireType::VarInt(VarIntMode::Signed) {
            CompatError::new(
                side.then(ctx.kind.type_name().member(&self.name).to_owned())
                    .into(),
                "Non-zigzag signed field",
            )
            .warn();
        }
    }
}

impl CheckCompat for Field {
    type Context<'a> = RecordContext<'a>;

    fn check_compat(ck: CompatPair<&'_ Self>, cx: CompatPair<Self::Context<'_>>) -> CompatResult {
        let id = cx.as_ref().map(|c| c.id).unwrap_eq();

        let qual_names = cx
            .as_ref()
            .zip(ck.map(|f| &f.name))
            .map(|(c, n)| c.ty.kind.type_name().member(n));

        let cx = qual_names
            .as_ref()
            .map(|q| q.borrowed())
            .zip(cx.map(|c| c.ty.types))
            .zip(ck.map(|f| f.kind))
            .map(|((field, types), kind)| FieldTypeContext { field, types, kind });

        if ck.as_ref().map(|f| &f.name).try_unwrap_eq().is_err() {
            CompatError::new(
                cx.as_ref().map(|c| c.field.to_owned()).into(),
                format!("Field name mismatch for ID {id}"),
            )
            .warn();
        }

        let (types, kinds) = ck.map(|f| (&f.ty, f.kind)).unzip();

        types.check(cx)?;
        kinds.as_ref().check(qual_names)?;

        Ok(())
    }
}

impl<'a> RecordValue<'a> for Field {
    type Names = std::iter::Once<&'a str>;

    fn names(&'a self) -> Self::Names { std::iter::once(&self.name) }

    fn missing_id(&self, cx: &CompatPair<TypeContext<'a>>, id: Side<i32>) -> CompatResult {
        let (side, id) = id.split();
        CompatError::new(
            cx.as_ref().map(|c| c.kind.to_owned()).into(),
            format!(
                "Field {} (ID {id}) missing and not reserved on {}",
                self.name,
                side.opposite().pretty(),
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
        CompatError::new(
            cx.as_ref().map(|c| c.kind.to_owned()).into(),
            format!("Field {name} has id {reader} on reader and {writer} on writer"),
        )
        .warn();

        Ok(())
    }

    fn missing_name(cx: &CompatPair<TypeContext<'a>>, name: &str, id: Side<i32>) -> CompatResult {
        let (side, id) = id.split();
        CompatError::new(
            cx.as_ref().map(|c| c.kind.to_owned()).into(),
            format!(
                "Field name {name} (ID {id} on {}) missing and not reserved on {}",
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
        use std::collections::hash_map::Entry;

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        enum Group {
            Uniq(usize),
            Oneof(i32),
        }

        #[derive(Debug, PartialEq, Eq, Hash)]
        struct FieldInfo<'a> {
            name: &'a str,
            group: Group,
        }

        ck.clone().zip(cx.as_ref()).for_each(|side| {
            let (side, (ck, cx)) = side.split();

            if matches!(cx.kind, TypeCheckKind::ByName { .. }) {
                ck.for_each(|(_, v)| v.warn_non_zigzag(&cx, side));
            }
        });

        let mut uf_ids: HashMap<i32, usize> = HashMap::new();
        let mut fields: HashMap<usize, HashSet<Side<FieldInfo>>> = HashMap::new();
        let mut group_reps: HashMap<Side<Group>, usize> = HashMap::new();
        let mut uf: UnionFind = UnionFind::default();
        let mut next_uniq = 0_usize;

        for side in ck.iter() {
            let (side, (key, val)) = side.split();
            let group = val.oneof.map_or_else(
                || {
                    let next = next_uniq + 1;
                    Group::Uniq(std::mem::replace(&mut next_uniq, next))
                },
                Group::Oneof,
            );

            let uf_id = match uf_ids.entry(*key) {
                Entry::Occupied(o) => *o.get(),
                Entry::Vacant(v) => {
                    let uf_id = uf.put();
                    v.insert(uf_id);
                    uf_id
                },
            };

            let field = FieldInfo {
                name: &val.name,
                group,
            };

            assert!(fields.entry(uf_id).or_default().insert(side.then(field)));

            if let Some(prev) = group_reps.insert(side.then(group), uf_id) {
                assert!(!matches!(group, Group::Uniq(_)));
                uf.union(prev, uf_id).unwrap();
            }
        }

        let mut clashes: HashMap<Side<usize>, BTreeSet<usize>> = HashMap::new();

        for &uf_id in uf_ids.values() {
            let fields = fields.get(&uf_id).unwrap();
            let root = uf.find(uf_id).unwrap();

            for field in fields {
                clashes.entry(field.then(root)).or_default().insert(uf_id);
            }
        }

        let clashes_rev: HashMap<BTreeSet<usize>, HashSet<Side<usize>>> =
            clashes
                .into_iter()
                .fold(HashMap::default(), |mut map, (k, v)| {
                    assert!(map.entry(v).or_default().insert(k));
                    map
                });

        for (clash, rep) in clashes_rev {
            if clash.len() < 2 {
                continue;
            }

            let clash_fields: HashSet<&Side<FieldInfo>> =
                clash.iter().flat_map(|k| fields.get(k).unwrap()).collect();
            let rep_fields: HashSet<&Side<FieldInfo>> = rep
                .iter()
                .flat_map(|v| fields.get(&v.inner()).unwrap())
                .collect();
            // TODO: identify relevant oneof decls

            let mut s = "Oneof group clash - fields involved: ".to_owned();

            for (i, field) in clash_fields.iter().enumerate() {
                use std::fmt::Write;

                if i != 0 {
                    s.push_str(", ");
                }

                let (side, field) = field.as_ref().split();
                write!(s, "{} on {}", field.name, side.pretty()).unwrap();
            }

            // TODO: continue on non-fatal errors
            return Err(CompatError::new(
                cx.as_ref().map(|c| c.kind.to_owned()).into(),
                s,
            ));
        }

        Ok(())
    }
}
