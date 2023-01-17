use std::{borrow::Borrow, collections::HashMap, hash::Hash};

use prost_types::{DescriptorProto, EnumDescriptorProto, FileDescriptorProto, FileDescriptorSet};

use super::scope_ref::ScopeRef;
use crate::schema::qual_name::QualName;

#[derive(Debug)]
pub struct GlobalScope<'a> {
    packages: HashMap<Option<&'a str>, Scope<'a>>,
}

impl<'a> GlobalScope<'a> {
    pub fn new(fildes_set: &'a FileDescriptorSet) -> Self {
        Self {
            packages: fildes_set
                .file
                .iter()
                .map(|f| (f.package.as_deref(), Scope::package(f)))
                .collect(),
        }
    }

    pub fn package<Q: Eq + Hash + ?Sized>(&'a self, package: &Q) -> Option<ScopeRef<'a>>
    where Option<&'a str>: Borrow<Q> {
        self.packages.get(package).map(|scope| {
            assert!(matches!(scope, Scope::Package { .. }));
            ScopeRef {
                global: self,
                parent: None,
                scope,
            }
        })
    }

    pub fn resolve_one(&'a self, name: &'a str) -> Option<(Option<&'a str>, ScopeRef<'a>)> {
        let package = self.packages.get(&Some(name));
        let anon = self
            .packages
            .get(&None)
            .and_then(|p| p.items().get(name).map(|c| (p, c)));

        let (package, scope) = match (package, anon) {
            (None, None) => return None,
            (Some(pkg), None) => (pkg, pkg),
            (None, Some((pkg, scope))) => (pkg, scope),
            (Some(_), Some(_)) => {
                panic!("Conflict for {name:?} between package and anon-packaged type")
            },
        };

        let Scope::Package { name, .. } = *package else { panic!("Invalid global scope") };

        Some((name, ScopeRef {
            global: self,
            parent: None,
            scope,
        }))
    }

    pub fn resolve(&'a self, path: impl IntoIterator<Item = &'a str>) -> Option<QualName<'a>> {
        let mut path = path.into_iter();
        let base = path.next().expect("Invalid fully-qualified path");

        let (package, ScopeRef { mut scope, .. }) = self.resolve_one(base)?;

        Some(QualName::new(
            package.map(Into::into),
            std::iter::from_fn(|| {
                let Some(child) = scope.items().get(path.next()?) else { return Some(None) };
                scope = child;
                let Scope::Type { name, .. } = *child else { panic!("Invalid scope") };
                Some(Some(name.into()))
            })
            .collect::<Option<_>>()?,
        ))
    }
}

type ScopeItems<'a> = HashMap<&'a str, Scope<'a>>;

fn scope_items<'a>(
    msgs: impl IntoIterator<Item = &'a DescriptorProto>,
    enums: impl IntoIterator<Item = &'a EnumDescriptorProto>,
) -> ScopeItems<'a> {
    msgs.into_iter()
        .map(|m| (m.name.as_deref().unwrap(), Scope::message(m)))
        .chain(
            enums
                .into_iter()
                .map(|e| (e.name.as_deref().unwrap(), Scope::enumeration(e))),
        )
        .collect()
}

#[derive(Debug)]
pub enum Scope<'a> {
    Package {
        name: Option<&'a str>,
        items: ScopeItems<'a>,
    },
    Type {
        name: &'a str,
        nested: ScopeItems<'a>,
    },
}

impl<'a> Scope<'a> {
    fn package(fildes: &'a FileDescriptorProto) -> Self {
        Self::Package {
            name: fildes.package.as_deref(),
            items: scope_items(&fildes.message_type, &fildes.enum_type),
        }
    }

    fn message(msg: &'a DescriptorProto) -> Self {
        Self::Type {
            name: msg.name.as_deref().unwrap(),
            nested: scope_items(&msg.nested_type, &msg.enum_type),
        }
    }

    fn enumeration(num: &'a EnumDescriptorProto) -> Self {
        Self::Type {
            name: num.name.as_deref().unwrap(),
            nested: scope_items([], []),
        }
    }

    pub fn items(&self) -> &ScopeItems<'a> {
        match self {
            Self::Package { items, .. } | Self::Type { nested: items, .. } => items,
        }
    }
}
