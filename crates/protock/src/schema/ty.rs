use std::fmt;

use super::{
    field::Field,
    field_kind::FieldKind,
    primitive::{BytesMode, VarIntMode, WireType},
    qual_name::{MemberQualName, QualName},
    record::Record,
    service::Service,
    variant::Variant,
    TypeMap,
};
use crate::{
    check_compat::{CheckCompat, CompatError, CompatLog},
    compat_pair::{CompatPair, Variance},
};

#[derive(Debug)]
enum Kind {
    Service(Service),
    Message(Record<Field>),
    Enum(Record<Variant>),
}

impl Kind {
    const fn var_pretty(&self) -> &'static str {
        match self {
            Self::Service(_) => "service",
            Self::Message(_) => "message",
            Self::Enum(_) => "enum",
        }
    }
}

#[derive(Debug)]
pub struct Type(Kind);

impl Type {
    pub const fn service(svc: Service) -> Self { Self(Kind::Service(svc)) }

    #[inline]
    pub const fn message(rec: Record<Field>) -> Self { Self(Kind::Message(rec)) }

    #[inline]
    pub const fn enumeration(rec: Record<Variant>) -> Self { Self(Kind::Enum(rec)) }

    #[inline]
    pub const fn var_pretty(&self) -> &'static str { self.0.var_pretty() }

    #[inline]
    pub const fn internal(&self) -> bool {
        match self.0 {
            Kind::Service(_) => false,
            Kind::Message(ref m) => m.internal(),
            Kind::Enum(ref e) => e.internal(),
        }
    }

    pub fn wire_format(&self, kind: FieldKind) -> WireType {
        match self.0 {
            Kind::Service(_) => WireType::Bytes(BytesMode::Rpc),
            Kind::Message(_) => WireType::Bytes(BytesMode::Message),
            Kind::Enum(_) => WireType::VarInt(VarIntMode::Enum),
        }
        .adjust_for_kind(kind)
    }
}

#[derive(Clone)]
pub enum TypeCheckKind<'a> {
    ByName(QualName<'a>),
    ForField {
        field: MemberQualName<'a>,
        ty: QualName<'a>,
    },
}

impl fmt::Debug for TypeCheckKind<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ByName(q) => write!(f, "{q:?}"),
            Self::ForField { field, ty } => write!(f, "{field:?}::<{ty:?}>"),
        }
    }
}

impl TypeCheckKind<'_> {
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

#[derive(Clone)]
pub struct TypeContext<'a> {
    pub kind: TypeCheckKind<'a>,
    pub types: &'a TypeMap,
}

impl CheckCompat for Type {
    type Context<'a> = TypeContext<'a>;

    fn check_compat<V: Variance>(
        ck: CompatPair<&'_ Type, V>,
        cx: CompatPair<Self::Context<'_>, V>,
        log: &mut CompatLog,
    ) {
        match ck.map(|t| &t.0).into_inner() {
            (Kind::Service(ref reader), Kind::Service(ref writer)) => {
                CompatPair::new_var(reader, writer).check(cx, log);
            },
            (Kind::Message(ref reader), Kind::Message(ref writer)) => {
                CompatPair::new_var(reader, writer).check(cx, log);
            },
            (Kind::Enum(ref reader), Kind::Enum(ref writer)) => {
                CompatPair::new_var(reader, writer).check(cx, log);
            },
            (rd, wr) => CompatError::new_var(
                cx.map(|c| c.kind.to_owned()).into(),
                format!(
                    "Type mismatch: {}",
                    CompatPair::new(&rd, &wr).map(|t| t.var_pretty()).display(),
                ),
            )
            .err(log),
        }
    }
}
