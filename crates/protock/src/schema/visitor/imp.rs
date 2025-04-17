use std::collections::{BTreeSet, HashMap, HashSet};

use prost_types::{
    descriptor_proto::ReservedRange, enum_descriptor_proto::EnumReservedRange,
    file_options::OptimizeMode, method_options::IdempotencyLevel, DescriptorProto,
    EnumDescriptorProto, EnumOptions, EnumValueDescriptorProto, FieldDescriptorProto, FieldOptions,
    FileDescriptorProto, FileDescriptorSet, FileOptions, MessageOptions, MethodDescriptorProto,
    MethodOptions, OneofDescriptorProto, ServiceDescriptorProto, ServiceOptions,
};
use shrec::range_set::RangeSet;

use super::{scope::GlobalScope, scope_ref::ScopeRef};
use crate::schema::{
    field::{Field, FieldExtra},
    field_kind::FieldKind,
    field_type::FieldType,
    oneof::Oneof,
    primitive::PrimitiveType,
    qual_name::QualName,
    record::Record,
    service::{Method, Service},
    ty::Type,
    variant::Variant,
    Schema,
};

pub struct Visitor<'a>(&'a mut Schema);

impl<'a> From<&'a mut Schema> for Visitor<'a> {
    fn from(val: &'a mut Schema) -> Self { Self(val) }
}

fn resolve_type_name<'a>(name: &str, scope: &'a ScopeRef) -> QualName<'a> {
    if let Some(name) = name.strip_prefix('.') {
        scope
            .global()
            .resolve(name.split('.'))
            .expect("Couldn't resolve fully-qualified type name")
    } else {
        scope
            .search(name.split('.'))
            .expect("Couldn't find valid scope for name")
    }
}

impl Visitor<'_> {
    pub fn fildes_set(&mut self, desc: &FileDescriptorSet) {
        let scope = GlobalScope::new(desc);

        tracing::trace!("{scope:#?}");

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
            self.desc(
                &scope
                    .clone()
                    .child(m.name.as_deref().unwrap())
                    .expect("Missing message scope"),
                m,
            );
        }

        for e in enums {
            self.enum_desc(
                &scope
                    .clone()
                    .child(e.name.as_deref().unwrap())
                    .expect("Missing enum scope"),
                e,
            );
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

        assert!(dependency.iter().all(|d| d.starts_with("google/protobuf")));
        assert!(public_dependency.is_empty());
        assert!(weak_dependency.is_empty());
        assert!(extension.is_empty());
        assert!(source_code_info.is_none());
        assert_eq!(syntax.as_deref(), Some("proto3"));

        // TODO: maybe useful at some point
        let (_optimize, _deprecated) = if let Some(opts) = options {
            #[expect(
                deprecated,
                reason = "Explicitly ignoring java_generate_equals_and_hash"
            )]
            let FileOptions {
                java_package: _,
                java_outer_classname: _,
                java_multiple_files: _,
                java_generate_equals_and_hash: _,
                java_string_check_utf8: _,
                optimize_for,
                go_package: _,
                cc_generic_services: _,
                java_generic_services: _,
                py_generic_services: _,
                php_generic_services: _,
                deprecated,
                cc_enable_arenas: _,
                objc_class_prefix: _,
                csharp_namespace: _,
                swift_prefix: _,
                php_class_prefix: _,
                php_namespace: _,
                php_metadata_namespace: _,
                ruby_package: _,
                uninterpreted_option,
            } = opts;

            assert!(uninterpreted_option.is_empty());

            (
                optimize_for
                    .and_then(|o| o.try_into().ok())
                    .unwrap_or_default(),
                deprecated.unwrap_or(false),
            )
        } else {
            (OptimizeMode::default(), false)
        };

        let scope = scope.package_ref(package.as_ref()).unwrap();

        self.descend(&scope, message_type, enum_type);

        for s in service {
            self.svc(
                &scope
                    .clone()
                    .child(s.name.as_deref().unwrap())
                    .expect("Missing service scope"),
                s,
            );
        }
    }

    fn svc(&mut self, scope: &ScopeRef<'_>, desc: &ServiceDescriptorProto) {
        let ServiceDescriptorProto {
            name,
            method,
            options,
        } = desc;

        let name = name.as_ref().unwrap();

        let qual_name = scope
            .parent()
            .and_then(|p| p.qualify([&**name]))
            .expect("Invalid service name");

        let deprecated = if let Some(opts) = options {
            let ServiceOptions {
                deprecated,
                uninterpreted_option,
            } = opts;

            assert!(uninterpreted_option.is_empty());

            deprecated.unwrap_or(false)
        } else {
            false
        };

        let mut methods = HashMap::new();
        for method in method {
            let MethodDescriptorProto {
                name,
                input_type,
                output_type,
                options,
                client_streaming,
                server_streaming,
            } = method;

            let name = name.as_ref().unwrap();
            let in_ty = input_type.as_ref().unwrap();
            let out_ty = output_type.as_ref().unwrap();

            let in_qual = resolve_type_name(in_ty, scope).into_owned();
            let out_qual = resolve_type_name(out_ty, scope).into_owned();

            let (deprecated, idempotency_level) = if let Some(opts) = options {
                let all_deprecated = deprecated;
                let MethodOptions {
                    deprecated,
                    idempotency_level,
                    uninterpreted_option,
                } = opts;

                assert!(uninterpreted_option.is_empty());

                (
                    all_deprecated || deprecated.unwrap_or(false),
                    idempotency_level
                        .and_then(|l| l.try_into().ok())
                        .unwrap_or_default(),
                )
            } else {
                (deprecated, IdempotencyLevel::default())
            };

            assert!(methods
                .insert(name.into(), Method {
                    idempotency: idempotency_level,
                    deprecated,
                    input_type: in_qual,
                    input_stream: client_streaming.unwrap_or_default(),
                    output_type: out_qual,
                    output_stream: server_streaming.unwrap_or_default()
                })
                .is_none());
        }

        assert!(self
            .0
            .types
            .insert(qual_name.into_owned(), Type::service(Service::new(methods)))
            .is_none());
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
        let mut oneofs = vec![];

        for field in field {
            Self::field(&mut numbers, scope, field);
        }

        for oneof in oneof_decl {
            let OneofDescriptorProto { name, options } = oneof;

            let name = name.as_ref().unwrap();
            assert!(options.is_none());

            oneofs.push(Oneof::new(name.into()));
        }

        let reserved = if deprecated {
            RangeSet::full()
        } else {
            reserved_range
                .iter()
                .map(|ReservedRange { start, end }| start.unwrap().into()..end.unwrap().into())
                .collect()
        };

        let reserved_names: HashSet<_> = reserved_name.iter().cloned().collect();
        assert!(reserved_names.len() == reserved_name.len());

        assert!(self
            .0
            .types
            .insert(
                qual_name.into_owned(),
                Type::message(Record::new(
                    numbers,
                    reserved,
                    reserved_names,
                    is_for_map,
                    FieldExtra::new(oneofs)
                ))
            )
            .is_none());

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
        let label = label.and_then(|l| l.try_into().ok()).unwrap();
        let ty = r#type.and_then(|t| t.try_into().ok());
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
                FieldType::Named(resolve_type_name(type_name.unwrap(), scope).into_owned())
            },
            FieldKind::new(label, packed, *proto3_optional),
            oneof_index.map(|i| usize::try_from(i).unwrap().into()),
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
                assert!(numbers
                    .insert(number, [name.into()].into_iter().collect())
                    .is_none());
            }
        }

        let reserved = if deprecated {
            RangeSet::full()
        } else {
            reserved_range
                .iter()
                .map(|EnumReservedRange { start, end }| {
                    start.unwrap().into()..end.and_then(|i| i64::from(i).checked_add(1)).unwrap()
                })
                .collect()
        };

        let reserved_names: HashSet<_> = reserved_name.iter().cloned().collect();
        assert!(reserved_names.len() == reserved_name.len());

        assert!(self
            .0
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
                    false,
                    (),
                ))
            )
            .is_none());
    }
}
