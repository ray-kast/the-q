use std::{
    borrow::{Borrow, Cow},
    collections::HashMap,
    hash::Hash,
};

use prost_types::{
    DescriptorProto, EnumDescriptorProto, FileDescriptorProto, FileDescriptorSet,
    ServiceDescriptorProto,
};

use super::scope_ref::ScopeRef;
use crate::schema::qual_name::QualName;

#[derive(Debug, Default)]
#[repr(transparent)]
pub struct GlobalScope<'a>(pub(super) Scope<'a>);

#[inline]
fn split_package<'a, S: AsRef<str> + 'a>(
    package: Option<&'a S>,
) -> impl Iterator<Item = &'a str> + 'a {
    package.into_iter().flat_map(move |s| s.as_ref().split('.'))
}

impl<'a> GlobalScope<'a> {
    pub fn new(fildes_set: &'a FileDescriptorSet) -> Self {
        Self(fildes_set.file.iter().fold(Scope::default(), |mut p, f| {
            split_package(f.package.as_ref())
                .fold(&mut p, |n, p| n.children.entry(p).or_default())
                .package(f);
            p
        }))
    }

    #[inline]
    pub fn package_ref<S: AsRef<str>>(&self, package: Option<&S>) -> Option<ScopeRef> {
        let ret = split_package(package).try_fold(ScopeRef::new(self), ScopeRef::child)?;

        matches!(ret.scope.kind, Some(ScopeKind::Package(_))).then_some(ret)
    }

    pub fn resolve<'b, Q: Eq + Hash + ?Sized + 'b>(
        &self,
        path: impl IntoIterator<Item = &'b Q>,
    ) -> Option<QualName<'a>>
    where
        &'a str: Borrow<Q>,
    {
        let mut package = None;
        let mut parts = vec![];
        let mut curr = &self.0;
        let mut path = path.into_iter();

        loop {
            match curr.kind {
                Some(ScopeKind::Package(p)) => {
                    package = Some(p);
                    parts.clear();
                },
                Some(ScopeKind::Type(t)) => {
                    parts.push(Cow::Borrowed(t));
                },
                None => assert!(
                    package.is_none(),
                    "Invalid global scope tree (empty scope inside package)"
                ),
            }

            let Some(part) = path.next() else { break };
            curr = curr.children.get(part)?;
        }

        Some(QualName::new(
            package
                .expect("Invalid global scope tree (missing package)")
                .map(Into::into),
            parts,
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScopeKind<'a> {
    Package(Option<&'a str>),
    Type(&'a str),
}

type ScopeChildren<'a> = HashMap<&'a str, Scope<'a>>;

#[derive(Debug, Default)]
pub(super) struct Scope<'a> {
    pub kind: Option<ScopeKind<'a>>,
    pub children: ScopeChildren<'a>,
}

fn scope_children<'a>(
    msgs: impl IntoIterator<Item = &'a DescriptorProto>,
    enums: impl IntoIterator<Item = &'a EnumDescriptorProto>,
    svcs: impl IntoIterator<Item = &'a ServiceDescriptorProto>,
) -> impl Iterator<Item = (&'a str, Scope<'a>)> {
    msgs.into_iter()
        .map(|m| (m.name.as_deref().unwrap(), Scope::message(m)))
        .chain(
            enums
                .into_iter()
                .map(|e| (e.name.as_deref().unwrap(), Scope::enumeration(e))),
        )
        .chain(
            svcs.into_iter()
                .map(|s| (s.name.as_deref().unwrap(), Scope::service(s))),
        )
}

impl<'a> Scope<'a> {
    fn message(msg: &'a DescriptorProto) -> Self {
        Self {
            kind: Some(ScopeKind::Type(msg.name.as_deref().unwrap())),
            children: scope_children(&msg.nested_type, &msg.enum_type, []).collect(),
        }
    }

    fn enumeration(num: &'a EnumDescriptorProto) -> Self {
        Self {
            kind: Some(ScopeKind::Type(num.name.as_deref().unwrap())),
            children: HashMap::new(),
        }
    }

    fn service(svc: &'a ServiceDescriptorProto) -> Self {
        Self {
            kind: Some(ScopeKind::Type(svc.name.as_deref().unwrap())),
            children: HashMap::new(),
        }
    }

    fn package(&mut self, fildes: &'a FileDescriptorProto) {
        let kind = ScopeKind::Package(fildes.package.as_deref());
        let prev = self.kind.replace(kind);
        assert!(
            prev.is_none_or(|k| k == kind),
            "Package scope {:?} conflicts with existing definition {prev:?}",
            fildes.package
        );

        for (k, v) in scope_children(&fildes.message_type, &fildes.enum_type, &fildes.service) {
            assert!(
                self.children.insert(k, v).is_none(),
                "Duplicate declaration {k:?} in package {:?}",
                fildes.package
            );
        }
    }
}
