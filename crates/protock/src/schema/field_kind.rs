use prost_types::field_descriptor_proto::Label;

use super::qual_name::MemberQualName;
use crate::{
    check_compat::{CheckCompat, CompatError, CompatLog},
    compat_pair::CompatPair,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Singular,
    Repeated { packed: Option<bool> },
    Optional,
}

impl FieldKind {
    pub fn new(label: Label, packed: Option<bool>, proto3_optional: Option<bool>) -> Self {
        if !matches!(label, Label::Repeated) {
            assert!(packed.is_none());
        }

        match (label, proto3_optional) {
            (Label::Optional, Some(false) | None) => Self::Singular,
            (Label::Required, None) => panic!("Unsupported required label found"),
            (Label::Repeated, None) => Self::Repeated { packed },
            (Label::Optional, Some(true)) => Self::Optional,
            (l, o) => panic!("Unexpected field kind ({l:?}, optional={o:?})"),
        }
    }
}

impl CheckCompat for FieldKind {
    type Context<'a> = MemberQualName<'a>;

    fn check_compat(
        ck: CompatPair<&'_ Self>,
        cx: CompatPair<Self::Context<'_>>,
        log: &mut CompatLog,
    ) {
        match ck.into_inner() {
            (a, b) if a == b => (),
            (Self::Singular | Self::Optional, Self::Singular | Self::Optional) => (),
            (Self::Repeated { packed: _ }, Self::Singular | Self::Optional) => {},
            (rd @ (Self::Singular | Self::Optional), wr @ Self::Repeated { packed: _ }) => {
                CompatError::new(
                    cx.map(|n| n.to_owned()).into(),
                    format!(
                        "Repeated/singular mismatch ({:?})",
                        CompatPair::new(rd, wr).display()
                    ),
                )
                .warn(log);
            },
            (rd, wr) => CompatError::new(
                cx.map(|n| n.to_owned()).into(),
                format!(
                    "Incompatible field kinds ({:?})",
                    CompatPair::new(rd, wr).display()
                ),
            )
            .err(log),
        }
    }
}
