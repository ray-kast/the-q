use std::fmt;

use super::{
    field::Field,
    field_kind::FieldKind,
    primitive::{BytesMode, VarIntMode, WireType},
    qual_name::{MemberQualName, QualName},
    record::Record,
    variant::Variant,
    TypeMap,
};
use crate::{
    check_compat::{CheckCompat, CompatError, CompatResult},
    compat_pair::CompatPair,
};

#[derive(Debug)]
enum Kind {
    Message(Record<Field>),
    Enum(Record<Variant>),
}

impl Kind {
    const fn var_pretty(&self) -> &'static str {
        match self {
            Self::Message(_) => "message",
            Self::Enum(_) => "enum",
        }
    }
}

#[derive(Debug)]
pub struct Type(Kind);

impl Type {
    #[inline]
    pub const fn message(rec: Record<Field>) -> Self { Self(Kind::Message(rec)) }

    #[inline]
    pub const fn enumeration(rec: Record<Variant>) -> Self { Self(Kind::Enum(rec)) }

    #[inline]
    pub const fn var_pretty(&self) -> &'static str { self.0.var_pretty() }

    #[inline]
    pub const fn internal(&self) -> bool {
        match self.0 {
            Kind::Message(ref m) => m.internal(),
            Kind::Enum(ref e) => e.internal(),
        }
    }

    pub fn wire_format(&self, kind: FieldKind) -> WireType {
        match self.0 {
            Kind::Message(_) => WireType::Bytes(BytesMode::Message),
            Kind::Enum(_) => WireType::VarInt(VarIntMode::Enum),
        }
        .adjust_for_kind(kind)
    }
}

pub enum TypeCheckKind<'a> {
    ByName(QualName<'a>),
    ForField {
        field: MemberQualName<'a>,
        ty: QualName<'a>,
    },
}

impl<'a> fmt::Debug for TypeCheckKind<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ByName(q) => write!(f, "{q:?}"),
            Self::ForField { field, ty } => write!(f, "{field:?}::<{ty:?}>"),
        }
    }
}

impl<'a> TypeCheckKind<'a> {
    pub fn to_owned(&self) -> TypeCheckKind<'static> {
        match self {
            Self::ByName(q) => TypeCheckKind::ByName(q.to_owned()),
            Self::ForField { field, ty } => TypeCheckKind::ForField {
                field: field.to_owned(),
                ty: ty.to_owned(),
            },
        }
    }
}

impl<'a> TypeCheckKind<'a> {
    pub fn type_name(&self) -> &QualName<'a> {
        match self {
            Self::ByName(q) => q,
            Self::ForField { ty, .. } => ty,
        }
    }
}

pub struct TypeContext<'a> {
    pub kind: TypeCheckKind<'a>,
    pub types: &'a TypeMap,
}

impl CheckCompat for Type {
    type Context<'a> = TypeContext<'a>;

    fn check_compat(ck: CompatPair<&'_ Type>, cx: CompatPair<Self::Context<'_>>) -> CompatResult {
        match ck.map(|t| &t.0).into_inner() {
            (Kind::Message(ref reader), Kind::Message(ref writer)) => {
                CompatPair::new(reader, writer).check(cx)
            },
            (Kind::Enum(ref reader), Kind::Enum(ref writer)) => {
                CompatPair::new(reader, writer).check(cx)
            },
            (rd, wr) => Err(CompatError::new(
                cx.map(|c| c.kind.to_owned()).into(),
                // TODO: DRY the "x in reader, y in writer" stuff
                format!(
                    "Type mismatch: {} in reader, {} in writer",
                    rd.var_pretty(),
                    wr.var_pretty()
                ),
            )),
        }
    }
}
