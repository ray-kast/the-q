mod field;
mod field_kind;
mod field_type;
mod oneof;
mod primitive;
mod qual_name;
mod record;
mod reserved;
mod ty;
mod variant;

pub use imp::{Schema, SchemaContext, TypeError, TypeMap};

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
        check_compat::{CheckCompat, CompatError, CompatLog},
        compat_pair::{CompatPair, Side},
    };

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct TypeMap(HashMap<QualName<'static>, Type>);

    pub struct TypeError<'a, Q: ?Sized>(pub &'a Q);

    impl TypeMap {
        #[inline]
        pub fn get<'a, 'b, Q: Eq + std::hash::Hash>(
            &'a self,
            key: &'b Q,
        ) -> Result<&Type, TypeError<'b, Q>>
        where
            QualName<'a>: std::borrow::Borrow<Q>,
        {
            self.0.get(key).ok_or(TypeError(key))
        }

        pub fn insert(&mut self, key: QualName<'static>, val: Type) -> Option<Type> {
            self.0.insert(key, val)
        }
    }

    #[derive(Debug)]
    pub struct Schema {
        types: TypeMap,
    }

    impl Schema {
        pub fn new(desc: &FileDescriptorSet) -> Self {
            let mut me = Self {
                types: TypeMap(HashMap::new()),
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
            log: &mut CompatLog,
        ) {
            let type_maps = ck.map(|s| &s.types);
            type_maps.map(|m| &m.0).check_joined(
                &type_maps,
                log,
                |types, name| TypeContext {
                    kind: TypeCheckKind::ByName(name.borrowed()),
                    types,
                },
                |k, v, log| {
                    if let Some(writer) = v.visit(Side::Writer(())) {
                        if !writer.internal() {
                            CompatError::new(
                                cx.as_ref().map(|c| c.name.to_owned()).into(),
                                format!(
                                    "Missing {} type {k:?} present in writer",
                                    writer.var_pretty()
                                ),
                            )
                            .err(log);
                        }
                    }
                },
            );
        }
    }
}
