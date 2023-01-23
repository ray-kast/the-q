use prost_types::field_descriptor_proto::Type;

use super::{field_kind::FieldKind, field_type::FieldTypeContext};
use crate::{
    check_compat::{CheckCompat, CompatError, CompatLog},
    compat_pair::CompatPair,
};

#[derive(Debug, Clone, Copy)]
pub enum PrimitiveType {
    F64,    // Double
    F32,    // Float
    VarI64, // Int64
    VarU64, // Uint64
    VarI32, // Int32
    FixU64, // Fixed64
    FixU32, // Fixed32
    Bool,   // Bool
    String, // String
    Bytes,  // Bytes
    VarU32, // Uint32
    FixI32, // Sfixed32
    FixI64, // Sfixed64
    VarZ32, // Sint32
    VarZ64, // Sint64
}

impl PrimitiveType {
    pub fn new(ty: Type) -> Option<Self> {
        Some(match ty {
            Type::Double => Self::F64,
            Type::Float => Self::F32,
            Type::Int64 => Self::VarI64,
            Type::Uint64 => Self::VarU64,
            Type::Int32 => Self::VarI32,
            Type::Fixed64 => Self::FixU64,
            Type::Fixed32 => Self::FixU32,
            Type::Bool => Self::Bool,
            Type::String => Self::String,
            Type::Bytes => Self::Bytes,
            Type::Uint32 => Self::VarU32,
            Type::Sfixed32 => Self::FixI32,
            Type::Sfixed64 => Self::FixI64,
            Type::Sint32 => Self::VarZ32,
            Type::Sint64 => Self::VarZ64,
            Type::Group | Type::Message | Type::Enum => return None,
        })
    }

    pub fn wire_format(self, kind: FieldKind) -> WireType {
        match self {
            Self::F64 => WireType::Fix64(FixIntMode::Float),
            Self::F32 => WireType::Fix32(FixIntMode::Float),
            Self::VarI64 | Self::VarI32 => WireType::VarInt(VarIntMode::Signed),
            Self::VarU64 | Self::VarU32 | Self::Bool => WireType::VarInt(VarIntMode::Unsigned),
            Self::FixU64 => WireType::Fix64(FixIntMode::Unsigned),
            Self::FixU32 => WireType::Fix32(FixIntMode::Unsigned),
            Self::String => WireType::Bytes(BytesMode::Utf8),
            Self::Bytes => WireType::Bytes(BytesMode::Bytes),
            Self::FixI32 => WireType::Fix32(FixIntMode::Signed),
            Self::FixI64 => WireType::Fix64(FixIntMode::Signed),
            Self::VarZ32 | Self::VarZ64 => WireType::VarInt(VarIntMode::ZigZag),
        }
        .adjust_for_kind(kind)
    }
}

type NumericWireType = WireType<std::convert::Infallible>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireType<B = BytesMode> {
    VarInt(VarIntMode),
    Fix32(FixIntMode),
    Fix64(FixIntMode),
    Bytes(B),
}

impl WireType {
    fn to_numeric(self) -> Option<NumericWireType> {
        match self {
            Self::VarInt(m) => Some(WireType::VarInt(m)),
            Self::Fix32(m) => Some(WireType::Fix32(m)),
            Self::Fix64(m) => Some(WireType::Fix64(m)),
            Self::Bytes(_) => None,
        }
    }

    pub fn adjust_for_kind(self, kind: FieldKind) -> Self {
        match (self.to_numeric(), kind) {
            (
                Some(n),
                FieldKind::Repeated {
                    packed: None | Some(true),
                },
            ) => Self::Bytes(BytesMode::Packed(n)),
            (..) => self,
        }
    }
}

impl CheckCompat for WireType {
    type Context<'a> = FieldTypeContext<'a>;

    fn check_compat(
        ck: CompatPair<&'_ Self>,
        cx: CompatPair<Self::Context<'_>>,
        log: &mut CompatLog,
    ) {
        match ck.into_inner() {
            (WireType::VarInt(ref reader), WireType::VarInt(ref writer)) => {
                CompatPair::new(reader, writer).check(cx, log);
            },
            (WireType::Fix32(ref reader), WireType::Fix32(ref writer))
            | (WireType::Fix64(ref reader), WireType::Fix64(ref writer)) => {
                CompatPair::new(reader, writer).check(cx, log);
            },
            (WireType::Bytes(ref reader), WireType::Bytes(ref writer)) => {
                CompatPair::new(reader, writer).check(cx, log);
            },
            (rd, wr) => CompatError::new(
                cx.map(|c| c.field.to_owned()).into(),
                format!(
                    "Fields have incompatible wire formats ({rd:?} for reader, {wr:?} for writer)"
                ),
            )
            .err(log),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarIntMode {
    Signed,
    Unsigned,
    ZigZag,
    Enum,
}

impl CheckCompat for VarIntMode {
    type Context<'a> = FieldTypeContext<'a>;

    fn check_compat(
        ck: CompatPair<&'_ Self>,
        cx: CompatPair<Self::Context<'_>>,
        log: &mut CompatLog,
    ) {
        match ck.into_inner() {
            (a, b) if a == b => (),
            (rd @ (Self::Signed | Self::Unsigned), wr @ (Self::Signed | Self::Unsigned)) => {
                CompatError::new(
                    cx.map(|c| c.field.to_owned()).into(),
                    format!(
                        "Varint sign difference ({:?})",
                        CompatPair::new(rd, wr).display()
                    ),
                )
                .warn(log);
            },
            (rd @ (Self::Signed | Self::Unsigned), wr @ Self::Enum)
            | (rd @ Self::Enum, wr @ (Self::Signed | Self::Unsigned)) => CompatError::new(
                cx.map(|c| c.field.to_owned()).into(),
                format!(
                    "Enum type punning ({:?})",
                    CompatPair::new(rd, wr).display()
                ),
            )
            .warn(log),
            (rd, wr) => CompatError::new(
                cx.map(|c| c.field.to_owned()).into(),
                format!(
                    "Incompatible varint formats ({:?})",
                    CompatPair::new(rd, wr).display()
                ),
            )
            .err(log),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixIntMode {
    Signed,
    Unsigned,
    Float,
}

impl CheckCompat for FixIntMode {
    type Context<'a> = FieldTypeContext<'a>;

    fn check_compat(
        ck: CompatPair<&'_ Self>,
        cx: CompatPair<Self::Context<'_>>,
        log: &mut CompatLog,
    ) {
        match ck.into_inner() {
            (a, b) if a == b => (),
            (rd @ (Self::Signed | Self::Unsigned), wr @ (Self::Signed | Self::Unsigned)) => {
                CompatError::new(
                    cx.map(|c| c.field.to_owned()).into(),
                    format!(
                        "Sign difference in fixint fields ({:?})",
                        CompatPair::new(rd, wr).display(),
                    ),
                )
                .warn(log);
            },
            (rd, wr) => CompatError::new(
                cx.map(|c| c.field.to_owned()).into(),
                format!(
                    "Incompatible fixint formats ({:?})",
                    CompatPair::new(rd, wr).display(),
                ),
            )
            .err(log),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BytesMode {
    Bytes,
    Utf8,
    Message,
    Packed(NumericWireType),
}

impl CheckCompat for BytesMode {
    type Context<'a> = FieldTypeContext<'a>;

    fn check_compat(
        ck: CompatPair<&'_ Self>,
        cx: CompatPair<Self::Context<'_>>,
        log: &mut CompatLog,
    ) {
        match ck.into_inner() {
            (a, b) if a == b => (),
            (rd @ (Self::Bytes | Self::Utf8), wr @ (Self::Bytes | Self::Utf8)) => CompatError::new(
                cx.map(|c| c.field.to_owned()).into(),
                format!("UTF-8 type punning ({:?})", CompatPair::new(rd, wr)),
            )
            .warn(log),
            (rd @ (Self::Bytes | Self::Message), wr @ (Self::Bytes | Self::Message)) => {
                CompatError::new(
                    cx.map(|c| c.field.to_owned()).into(),
                    format!(
                        "Embedded message type punning ({:?})",
                        CompatPair::new(rd, wr).display(),
                    ),
                )
                .warn(log);
            },
            (rd, wr) => CompatError::new(
                cx.map(|c| c.field.to_owned()).into(),
                format!(
                    "Incompatible byte formats ({:?})",
                    CompatPair::new(rd, wr).display(),
                ),
            )
            .err(log),
        }
    }
}
