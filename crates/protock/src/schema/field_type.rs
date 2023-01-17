use super::{
    field_kind::FieldKind,
    primitive::{PrimitiveType, WireType},
    qual_name::{MemberQualName, QualName},
    ty::Type,
    TypeMap,
};
use crate::{
    check_compat::{CheckCompat, CompatResult},
    compat_pair::CompatPair,
    schema::ty::{TypeCheckKind, TypeContext},
};

#[derive(Debug)]
pub enum FieldType {
    Primitive(PrimitiveType),
    Named(QualName<'static>),
}

impl FieldType {
    pub fn wire_format<'a>(
        &'a self,
        kind: FieldKind,
        // TODO: this should be fallible
        ty: impl Fn(&'a QualName<'a>) -> &'a Type,
    ) -> WireType {
        match self {
            &Self::Primitive(p) => p.wire_format(kind),
            Self::Named(n) => ty(n).wire_format(kind),
        }
    }
}

pub struct FieldTypeContext<'a> {
    pub field: MemberQualName<'a>,
    pub types: &'a TypeMap,
    pub kind: FieldKind,
}

impl CheckCompat for FieldType {
    type Context<'a> = FieldTypeContext<'a>;

    fn check_compat(ck: CompatPair<&'_ Self>, cx: CompatPair<Self::Context<'_>>) -> CompatResult {
        let names = cx.as_ref().map(|c| c.field.to_owned());
        let type_maps = cx.as_ref().map(|c| c.types);

        let wire_formats = ck
            .zip(cx.as_ref())
            .map(|(t, c)| t.wire_format(c.kind, |n| c.types.get(n).unwrap()));

        wire_formats.as_ref().check(cx)?;

        if let Some(type_names) = ck.filter_map(|t| {
            if let Self::Named(t) = t {
                Some(t)
            } else {
                None
            }
        }) {
            let _wire = wire_formats.unwrap_eq();

            let types = type_names.zip(type_maps).map(|(n, t)| t.get(n).unwrap());

            let cx = names
                .zip(type_names)
                .zip(type_maps)
                .map(|((field, ty), types)| TypeContext {
                    kind: TypeCheckKind::ForField {
                        field,
                        ty: ty.borrowed(),
                    },
                    types,
                });

            types.check(cx)?;
        }

        Ok(())
    }
}
