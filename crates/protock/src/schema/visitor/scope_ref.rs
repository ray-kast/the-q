use std::{
    borrow::{Borrow, Cow},
    hash::Hash,
    rc::Rc,
};

use super::scope::{GlobalScope, Scope, ScopeKind};
use crate::schema::qual_name::QualName;

#[derive(Debug, Clone)]
pub struct ScopeRef<'a> {
    pub(super) global: &'a GlobalScope<'a>,
    pub(super) parent: Option<Rc<ScopeRef<'a>>>,
    pub(super) scope: &'a Scope<'a>,
}

impl<'a> ScopeRef<'a> {
    #[inline]
    pub fn new(global: &'a GlobalScope<'a>) -> Self {
        Self {
            global,
            parent: None,
            scope: &global.0,
        }
    }

    #[inline]
    pub fn global(&self) -> &'a GlobalScope<'a> { self.global }

    #[inline]
    pub fn parent(&self) -> Option<&ScopeRef<'a>> { self.parent.as_deref() }

    pub fn child<Q: Eq + Hash + ?Sized>(self, name: &Q) -> Option<ScopeRef<'a>>
    where &'a str: Borrow<Q> {
        self.scope.children.get(name).map(|scope| ScopeRef {
            global: self.global,
            parent: Some(self.into()),
            scope,
        })
    }

    pub fn qualify<'b, Q: Eq + Hash + ?Sized + 'b>(
        &self,
        path: impl IntoIterator<Item = &'b Q>,
    ) -> Option<QualName<'a>>
    where
        &'a str: Borrow<Q>,
    {
        let mut parts = vec![];
        let mut curr = self;
        let package = loop {
            match curr
                .scope
                .kind
                .expect("Invalid scope tree (missing ancestor kind)")
            {
                ScopeKind::Package(p) => break p,
                ScopeKind::Type(t) => parts.push(Cow::Borrowed(t)),
            }

            curr = curr
                .parent
                .as_deref()
                .expect("Invalid scope tree (missing package)");
        };
        parts.reverse();

        let mut curr = self.scope;
        for part in path {
            let child = curr.children.get(part)?;
            let Some(ScopeKind::Type(ty)) = child.kind else {
                panic!("Invalid scope tree (descendant was not a type)");
            };
            parts.push(Cow::Borrowed(ty));
            curr = child;
        }

        Some(QualName::new(package.map(Into::into), parts))
    }

    pub fn search<'b, Q: Eq + Hash + ?Sized + 'b>(
        &self,
        path: impl IntoIterator<Item = &'b Q>,
    ) -> Option<QualName<'a>>
    where
        &'a str: Borrow<Q>,
    {
        let mut path = path.into_iter();
        let Some(first) = path.next() else {
            return self.qualify([]);
        };
        let mut curr = self;
        let owner = loop {
            if let Some(owner) = curr.clone().child(first) {
                break owner;
            }

            curr = curr.parent.as_deref()?;
        };
        owner.qualify(path)
    }
}
