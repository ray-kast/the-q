use super::{
    field_kind::FieldKind,
    field_type::{FieldType, FieldTypeContext},
    oneof,
    primitive::{VarIntMode, WireType},
    qual_name::MemberQualName,
    record::{RecordContext, RecordExtra, RecordValue},
    ty::{TypeCheckKind, TypeContext},
};
use crate::{
    check_compat::{CheckCompat, CompatError, CompatLog},
    compat_pair::{CompatPair, Side},
};

#[derive(Debug)]
pub struct Field {
    name: String,
    ty: FieldType,
    kind: FieldKind,
    oneof: Option<oneof::OneofId>,
}

#[derive(Debug)]
pub struct FieldExtra {
    oneofs: Vec<oneof::Oneof>,
}

impl Field {
    #[inline]
    pub const fn new(
        name: String,
        ty: FieldType,
        kind: FieldKind,
        oneof: Option<oneof::OneofId>,
    ) -> Self {
        Self {
            name,
            ty,
            kind,
            oneof,
        }
    }

    fn warn_non_zigzag(&self, ctx: &TypeContext<'_>, side: Side, log: &mut CompatLog) {
        let Ok(wire) = self
            .ty
            .wire_format(self.kind, |n| ctx.types.get(n))
        else {
            return;
        };

        if wire == WireType::VarInt(VarIntMode::Signed) {
            CompatError::new(
                side.then(ctx.kind.type_name().member(&self.name).to_owned())
                    .into(),
                "Non-zigzag signed field",
            )
            .warn(log);
        }
    }
}

impl FieldExtra {
    #[inline]
    pub fn new(oneofs: Vec<oneof::Oneof>) -> Self { Self { oneofs } }
}

impl CheckCompat for Field {
    type Context<'a> = RecordContext<'a>;

    fn check_compat(
        ck: CompatPair<&'_ Self>,
        cx: CompatPair<Self::Context<'_>>,
        log: &mut CompatLog,
    ) {
        let id = cx.as_ref().map(|c| c.id).unwrap_eq();

        let qual_names = cx
            .as_ref()
            .zip(ck.map(|f| &f.name))
            .map(|(c, n)| c.ty.kind.type_name().member(n));

        let cx = qual_names
            .as_ref()
            .map(MemberQualName::borrowed)
            .zip(cx.map(|c| c.ty.types))
            .zip(ck.map(|f| f.kind))
            .map(|((field, types), kind)| FieldTypeContext { field, types, kind });

        if ck.as_ref().map(|f| &f.name).try_unwrap_eq().is_err() {
            CompatError::new(
                cx.as_ref().map(|c| c.field.to_owned()).into(),
                format!("Field name mismatch for ID {id}"),
            )
            .warn(log);
        }

        let (types, kinds) = ck.map(|f| (&f.ty, f.kind)).unzip();

        types.check(cx, log);
        kinds.as_ref().check(qual_names, log);
    }
}

impl RecordExtra for Field {
    type Extra = FieldExtra;
}

impl<'a> RecordValue<'a> for Field {
    type Names = std::iter::Once<&'a str>;

    fn names(&'a self) -> Self::Names { std::iter::once(&self.name) }

    fn missing_id(&self, cx: &CompatPair<TypeContext<'a>>, id: Side<i32>, log: &mut CompatLog) {
        CompatError::new(
            cx.as_ref().map(|c| c.kind.to_owned()).into(),
            format!(
                "Field {} (ID {}) missing and not reserved on {}",
                self.name,
                id.display(),
                id.kind().opposite().pretty(),
            ),
        )
        .warn(log);
    }

    fn id_conflict(
        cx: &CompatPair<TypeContext<'a>>,
        name: &str,
        ids: CompatPair<i32>,
        log: &mut CompatLog,
    ) {
        CompatError::new(
            cx.as_ref().map(|c| c.kind.to_owned()).into(),
            format!("Field {name} has ID {}", ids.display()),
        )
        .warn(log);
    }

    fn missing_name(
        cx: &CompatPair<TypeContext<'a>>,
        name: &str,
        id: Side<i32>,
        log: &mut CompatLog,
    ) {
        CompatError::new(
            cx.as_ref().map(|c| c.kind.to_owned()).into(),
            format!(
                "Field name {name} (ID {}) missing and not reserved on {}",
                id.display(),
                id.kind().opposite().pretty()
            ),
        )
        .warn(log);
    }

    fn check_extra(
        ck: CompatPair<std::collections::hash_map::Iter<'_, i32, Self>>,
        cx: &CompatPair<TypeContext<'a>>,
        extra: CompatPair<&FieldExtra>,
        log: &mut CompatLog,
    ) where
        Self: Sized,
    {
        ck.clone().zip(cx.as_ref()).for_each(|side| {
            let (side, (ck, cx)) = side.split();

            if matches!(cx.kind, TypeCheckKind::ByName { .. }) {
                ck.for_each(|(_, v)| v.warn_non_zigzag(cx, side, log));
            }
        });

        oneof::check(
            ck.map(|i| i.map(|(k, v)| (*k, &*v.name, v.oneof))),
            cx,
            extra.map(|e| &e.oneofs),
            log,
        );
    }
}
