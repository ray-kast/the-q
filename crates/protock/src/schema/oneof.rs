use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};

use shrec::union_find::{ClassId, UnionFind};

use super::ty::TypeContext;
use crate::{
    check_compat::{CompatError, CompatLog},
    compat_pair::{CompatPair, Side, SideInclusive},
};

#[derive(Debug)]
pub struct Oneof {
    name: String,
}

impl Oneof {
    #[inline]
    pub fn new(name: String) -> Self { Self { name } }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct OneofId(usize);

impl From<usize> for OneofId {
    #[inline]
    fn from(val: usize) -> Self { Self(val) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Group<U = ClassId> {
    Uniq(U),
    Oneof(OneofId),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct FieldInfo<'a> {
    name: &'a str,
    group: Group,
}

pub fn check<'a>(
    field_info: CompatPair<impl Iterator<Item = (i32, &'a str, Option<OneofId>)>>,
    cx: &CompatPair<TypeContext<'a>>,
    oneofs: CompatPair<&Vec<Oneof>>,
    log: &mut CompatLog,
) {
    let mut uf: UnionFind = UnionFind::new();
    let mut field_classes: BTreeMap<i32, ClassId> = BTreeMap::new();
    let mut class_fields: BTreeMap<ClassId, BTreeMap<Side, FieldInfo>> = BTreeMap::new();
    let mut group_classes: BTreeMap<Side<Group>, ClassId> = BTreeMap::new();

    for side in field_info.iter() {
        use std::collections::btree_map::Entry;

        let (side, (id, name, oneof)) = side.split();
        let klass = match field_classes.entry(id) {
            Entry::Occupied(o) => *o.get(),
            Entry::Vacant(v) => {
                let klass = uf.add();
                v.insert(klass);
                klass
            },
        };
        let group = oneof.map_or(Group::Uniq(klass), Group::Oneof);

        assert!(class_fields
            .entry(klass)
            .or_default()
            .insert(side, FieldInfo { name, group })
            .is_none());

        if let Some(prev) = group_classes.insert(side.then(group), klass) {
            assert!(!matches!(group, Group::Uniq(_)));
            uf.union(prev, klass).unwrap();
        }
    }

    let mut clashes: BTreeMap<usize, BTreeSet<Side<Group>>> = BTreeMap::new();

    for (&klass, fields) in &class_fields {
        let root = uf.find(klass).unwrap();

        for (side, field) in fields {
            clashes
                .entry(root.id())
                .or_default()
                .insert(side.then(field.group));
        }
    }

    clashes.retain(|_, g| {
        const PEDANTIC: bool = false;
        let (r, w) = g.iter().fold((0, 0), |(r, w), s| match s.kind() {
            Side::Reader(()) => (r + 1, w),
            Side::Writer(()) => (r, w + 1),
        });
        // Rationale: in a single partition, only a single value can safely be
        //            read.  If w > 1, multiple constraint groups on the writer
        //            are included in this partition, and thus multiple values
        //            may be on the wire
        w > 1 || (PEDANTIC && r > 1)
    });

    let uf_id_len = field_classes.len();
    let field_ids: BTreeMap<ClassId, i32> =
        field_classes.into_iter().map(|(k, v)| (v, k)).collect();
    assert_eq!(uf_id_len, field_ids.len());

    let fields: BTreeMap<ClassId, (i32, SideInclusive<FieldInfo>)> = class_fields
        .into_iter()
        .map(|(i, mut f)| {
            assert!((1..=2).contains(&f.len()));
            (
                i,
                (
                    *field_ids.get(&i).unwrap(),
                    CompatPair::new(f.remove(&Side::Reader(())), f.remove(&Side::Writer(())))
                        .try_into()
                        .unwrap_or_else(|_| unreachable!()),
                ),
            )
        })
        .collect();

    for groups in clashes.into_values() {
        let groups: BTreeSet<Side<Group<_>>> = groups
            .into_iter()
            .map(|g| {
                g.kind().then(match *g {
                    Group::Uniq(i) => Group::Uniq(fields.get(&i).unwrap()),
                    Group::Oneof(o) => Group::Oneof(o),
                })
            })
            .collect();

        let mut s = "Oneof group clash between ".to_owned();
        let last = groups.len().checked_sub(1).unwrap();

        for (i, group) in groups.into_iter().enumerate() {
            match i {
                0 => (),
                _ if last == 1 => s.push_str(" and "),
                i if i == last => s.push_str(", and "),
                _ => s.push_str(", "),
            }

            match *group {
                Group::Oneof(o) => {
                    let oneof = group.kind().project(oneofs).map(|e| &e[o.0]);
                    write!(s, "group {}", oneof.map(|o| &o.name).display())
                },
                Group::Uniq((i, f)) => {
                    write!(s, "field ID {i} ({})", f.as_ref().map(|f| f.name).display())
                },
            }
            .unwrap();
        }

        CompatError::new(cx.as_ref().map(|c| c.kind.to_owned()).into(), s).err(log);
    }
}
