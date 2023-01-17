use std::collections::{BTreeSet, HashMap, HashSet};

use prost_types::{
    descriptor_proto::ReservedRange,
    enum_descriptor_proto::EnumReservedRange,
    field_descriptor_proto::{Label, Type as TypeDesc},
    DescriptorProto, EnumDescriptorProto, EnumOptions, EnumValueDescriptorProto,
    FieldDescriptorProto, FieldOptions, FileDescriptorProto, FileDescriptorSet, MessageOptions,
    OneofDescriptorProto,
};

use super::{scope::GlobalScope, scope_ref::ScopeRef};
use crate::schema::{
    field::Field, field_kind::FieldKind, field_type::FieldType, primitive::PrimitiveType,
    record::Record, reserved::ReservedMap, ty::Type, variant::Variant, Schema,
};

pub struct Visitor<'a>(&'a mut Schema);

impl<'a> From<&'a mut Schema> for Visitor<'a> {
    fn from(val: &'a mut Schema) -> Self { Self(val) }
}

impl<'a> Visitor<'a> {
    pub fn fildes_set(&mut self, desc: &FileDescriptorSet) {
        let scope = GlobalScope::new(desc);

        let FileDescriptorSet { file } = desc;

        file.iter().for_each(|f| self.fildes(&scope, f));
    }

    #[inline]
    fn descend(
        &mut self,
        scope: &ScopeRef<'_>,
        msgs: &[DescriptorProto],
        enums: &[EnumDescriptorProto],
    ) {
        for m in msgs {
            self.desc(&scope.clone().item(m.name.as_deref().unwrap()).unwrap(), m);
        }

        for e in enums {
            self.enum_desc(&scope.clone().item(e.name.as_deref().unwrap()).unwrap(), e);
        }
    }

    fn fildes(&mut self, scope: &GlobalScope<'_>, desc: &FileDescriptorProto) {
        let FileDescriptorProto {
            name: _,
            package,
            dependency,
            public_dependency,
            weak_dependency,
            message_type,
            enum_type,
            service,
            extension,
            options,
            source_code_info,
            syntax,
        } = desc;

        assert!(dependency.is_empty());
        assert!(public_dependency.is_empty());
        assert!(weak_dependency.is_empty());
        assert!(service.is_empty());
        assert!(extension.is_empty());
        assert!(options.is_none());
        assert!(source_code_info.is_none());
        assert_eq!(syntax.as_deref(), Some("proto3"));

        let scope = scope.package(&package.as_deref()).unwrap();

        self.descend(&scope, message_type, enum_type);
    }

    fn desc(&mut self, scope: &ScopeRef<'_>, desc: &DescriptorProto) {
        let DescriptorProto {
            name,
            field,
            extension,
            nested_type,
            enum_type,
            extension_range,
            oneof_decl,
            options,
            reserved_range,
            reserved_name,
        } = desc;

        let name = name.as_ref().unwrap();
        assert!(extension.is_empty());
        assert!(extension_range.is_empty());

        let qual_name = scope
            .parent()
            .and_then(|p| p.qualify([&**name]))
            .expect("Invalid message name");

        let (deprecated, is_for_map) = if let Some(opts) = options {
            let MessageOptions {
                message_set_wire_format,
                no_standard_descriptor_accessor,
                deprecated,
                map_entry,
                uninterpreted_option,
            } = opts;

            assert!(message_set_wire_format.is_none());
            assert!(no_standard_descriptor_accessor.is_none());
            assert!(uninterpreted_option.is_empty());

            (deprecated.unwrap_or(false), map_entry.unwrap_or(false))
        } else {
            (false, false)
        };

        let mut numbers = HashMap::new();

        for field in field {
            Self::field(&mut numbers, scope, field);
        }

        for oneof in oneof_decl {
            let OneofDescriptorProto { name: _, options } = oneof;

            assert!(options.is_none());
        }

        let reserved = if deprecated {
            ReservedMap::deprecated()
        } else {
            ReservedMap::new(
                reserved_range
                    .iter()
                    .map(|ReservedRange { start, end }| start.unwrap().into()..end.unwrap().into()),
            )
        };

        let reserved_names: HashSet<_> = reserved_name.iter().cloned().collect();
        assert!(reserved_names.len() == reserved_name.len());

        assert!(
            self.0
                .types
                .insert(
                    qual_name.into_owned(),
                    Type::message(Record::new(numbers, reserved, reserved_names, is_for_map))
                )
                .is_none()
        );

        self.descend(scope, nested_type, enum_type);
    }

    #[inline]
    fn field(
        numbers: &mut HashMap<i32, Field>,
        scope: &ScopeRef<'_>,
        field: &FieldDescriptorProto,
    ) {
        let FieldDescriptorProto {
            name,
            number,
            label,
            r#type,
            type_name,
            extendee,
            default_value: _,
            oneof_index,
            json_name: _,
            options,
            proto3_optional,
        } = field;

        let name = name.as_ref().unwrap();
        let number = number.unwrap();
        let label = label.and_then(Label::from_i32).unwrap();
        let ty = r#type.and_then(TypeDesc::from_i32);
        let type_name = type_name.as_ref();
        assert!(extendee.is_none());

        let packed = if let Some(opts) = options {
            let FieldOptions {
                ctype,
                packed,
                jstype,
                lazy,
                deprecated,
                weak,
                uninterpreted_option,
            } = opts;

            assert!(ctype.is_none());
            assert!(jstype.is_none());
            assert!(lazy.is_none());
            assert!(deprecated.is_none());
            assert!(weak.is_none());
            assert!(uninterpreted_option.is_empty());

            *packed
        } else {
            None
        };

        let field = Field::new(
            name.into(),
            if let Some(ty) = ty.and_then(PrimitiveType::new) {
                assert!(type_name.is_none());
                FieldType::Primitive(ty)
            } else {
                let type_name = type_name.unwrap();

                let qual = if let Some(type_name) = type_name.strip_prefix('.') {
                    scope
                        .global()
                        .resolve(type_name.split('.'))
                        .expect("Couldn't resolve fully-qualified type name")
                        .to_owned()
                } else {
                    scope
                        .search(type_name.split('.'))
                        .expect("Couldn't find valid scope for name")
                        .to_owned()
                };

                FieldType::Named(qual)
            },
            FieldKind::new(label, packed, *proto3_optional),
            *oneof_index,
        );

        assert!(numbers.insert(number, field).is_none());
    }

    fn enum_desc(&mut self, scope: &ScopeRef<'_>, desc: &EnumDescriptorProto) {
        let EnumDescriptorProto {
            name,
            value,
            options,
            reserved_range,
            reserved_name,
        } = desc;

        let name = name.as_ref().unwrap();
        let qual_name = scope
            .parent()
            .and_then(|p| p.qualify([&**name]))
            .expect("Invalid message name");

        let mut numbers: HashMap<i32, BTreeSet<String>> = HashMap::new();

        let (aliasing, deprecated) = if let Some(opts) = options {
            let EnumOptions {
                allow_alias,
                deprecated,
                uninterpreted_option,
            } = opts;

            assert!(uninterpreted_option.is_empty());

            (allow_alias.unwrap_or(false), deprecated.unwrap_or(false))
        } else {
            (false, false)
        };

        for value in value {
            let EnumValueDescriptorProto {
                name,
                number,
                options,
            } = value;

            let name = name.as_ref().unwrap();
            let number = number.unwrap();
            assert!(options.is_none());

            if aliasing {
                assert!(numbers.entry(number).or_default().insert(name.into()));
            } else {
                assert!(
                    numbers
                        .insert(number, [name.into()].into_iter().collect())
                        .is_none()
                );
            }
        }

        let reserved = if deprecated {
            ReservedMap::deprecated()
        } else {
            ReservedMap::new(
                reserved_range
                    .iter()
                    .map(|EnumReservedRange { start, end }| {
                        start.unwrap().into()
                            ..end.and_then(|i| i64::from(i).checked_add(1)).unwrap()
                    }),
            )
        };

        let reserved_names: HashSet<_> = reserved_name.iter().cloned().collect();
        assert!(reserved_names.len() == reserved_name.len());

        assert!(
            self.0
                .types
                .insert(
                    qual_name.into_owned(),
                    Type::enumeration(Record::new(
                        numbers
                            .into_iter()
                            .map(|(k, v)| (k, Variant::new(v)))
                            .collect(),
                        reserved,
                        reserved_names,
                        false
                    ))
                )
                .is_none()
        );
    }
}
