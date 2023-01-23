use super::{
    field_kind::FieldKind,
    primitive::{PrimitiveType, WireType},
    qual_name::{MemberQualName, QualName},
    ty::Type,
    TypeError, TypeMap,
};
use crate::{
    check_compat::{CheckCompat, CompatError, CompatLog},
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
        ty: impl Fn(&'a QualName<'a>) -> Result<&'a Type, TypeError<'a, QualName<'a>>>,
    ) -> Result<WireType, TypeError<'a, QualName<'a>>> {
        Ok(match self {
            &Self::Primitive(p) => p.wire_format(kind),
            Self::Named(n) => ty(n)?.wire_format(kind),
        })
    }
}

pub struct FieldTypeContext<'a> {
    pub field: MemberQualName<'a>,
    pub types: &'a TypeMap,
    pub kind: FieldKind,
}

impl CheckCompat for FieldType {
    type Context<'a> = FieldTypeContext<'a>;

    fn check_compat(
        ck: CompatPair<&'_ Self>,
        cx: CompatPair<Self::Context<'_>>,
        log: &mut CompatLog,
    ) {
        let names = cx.as_ref().map(|c| c.field.to_owned());
        let type_maps = cx.as_ref().map(|c| c.types);

        let wire_formats = match ck
            .zip(cx.as_ref())
            .try_map(|(t, c)| t.wire_format(c.kind, |n| c.types.get(n)))
        {
            Ok(f) => f,
            Err(e) => {
                CompatError::new(
                    e.kind().project(names).into(),
                    format!("Type resolution failure for {:?}", e.map(|e| e.0).display()),
                )
                .err(log);
                return;
            },
        };

        wire_formats.as_ref().check(cx, log);

        if let Some(type_names) = ck.filter_map(|t| {
            if let Self::Named(t) = t {
                Some(t)
            } else {
                None
            }
        }) {
            let _wire = wire_formats.unwrap_eq();

            let types = match type_names.zip(type_maps).try_map(|(n, t)| t.get(n)) {
                Ok(t) => t,
                Err(e) => {
                    CompatError::new(
                        e.kind().project(names).into(),
                        format!("Type resolution failure for {:?}", e.map(|e| e.0).display()),
                    )
                    .err(log);
                    return;
                },
            };

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

            types.check(cx, log);
        }
    }
}
