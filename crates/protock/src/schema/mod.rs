mod field;
mod field_kind;
mod field_type;
mod primitive;
mod qual_name;
mod record;
mod reserved;
mod ty;
mod variant;

pub use imp::{Schema, SchemaContext, TypeMap};

#[path = ""]
mod imp {
    mod visitor;

    use std::collections::HashMap;

    use prost_types::FileDescriptorSet;

    use super::{
        qual_name::QualName,
        ty::{Type, TypeCheckKind, TypeContext},
    };
    use crate::{
        check_compat::{CheckCompat, CompatError, CompatResult},
        compat_pair::{CompatPair, Side},
    };

    // TODO: wrap this to assert lookup fallibility (e.g. externs)
    pub type TypeMap = HashMap<QualName<'static>, Type>;

    #[derive(Debug)]
    pub struct Schema {
        types: TypeMap,
    }

    impl Schema {
        pub fn new(desc: &FileDescriptorSet) -> Self {
            let mut me = Self {
                types: HashMap::new(),
            };

            visitor::Visitor::from(&mut me).fildes_set(desc);
            tracing::trace!("{me:#?}");

            me
        }
    }

    pub struct SchemaContext<'a> {
        pub name: &'a str,
    }

    impl CheckCompat for Schema {
        type Context<'a> = SchemaContext<'a>;

        fn check_compat(
            ck: CompatPair<&'_ Schema>,
            cx: CompatPair<Self::Context<'_>>,
        ) -> CompatResult {
            let type_maps = ck.map(|s| &s.types);
            type_maps.check_joined(
                &type_maps,
                |types, name| TypeContext {
                    kind: TypeCheckKind::ByName(name.borrowed()),
                    types,
                },
                |k, v| {
                    if let Some(writer) = v.visit(Side::Writer(())) {
                        if writer.internal() {
                            Ok(())
                        } else {
                            Err(CompatError::new(
                                cx.as_ref().map(|c| c.name.to_owned()).into(),
                                format!(
                                    "Missing {} type {k:?} present in writer",
                                    writer.var_pretty()
                                ),
                            ))
                        }
                    } else {
                        Ok(())
                    }
                },
            )
        }
    }
}
