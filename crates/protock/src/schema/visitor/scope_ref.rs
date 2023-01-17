use std::{
    borrow::{Borrow, Cow},
    hash::Hash,
    rc::Rc,
};

use super::scope::{GlobalScope, Scope};
use crate::schema::qual_name::QualName;

#[derive(Debug, Clone)]
pub struct ScopeRef<'a> {
    pub(super) global: &'a GlobalScope<'a>,
    pub(super) parent: Option<Rc<ScopeRef<'a>>>,
    pub(super) scope: &'a Scope<'a>,
}

impl<'a> ScopeRef<'a> {
    #[inline]
    pub fn global(&self) -> &GlobalScope<'a> { self.global }

    #[inline]
    pub fn parent(&self) -> Option<&ScopeRef<'a>> { self.parent.as_deref() }

    pub fn item<Q: Eq + Hash + ?Sized>(self, name: &Q) -> Option<ScopeRef<'a>>
    where &'a str: Borrow<Q> {
        self.scope.items().get(name).map(|scope| ScopeRef {
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
        let mut curr = Some(self);
        let mut package = None;
        let up = std::iter::from_fn(|| {
            let me = curr?;
            curr = me.parent.as_deref();
            match *me.scope {
                Scope::Package { name, .. } => {
                    package = name;
                    assert!(curr.is_none());
                    None
                },
                Scope::Type { name, .. } => Some(name),
            }
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|s| Some(s.into()));

        let mut path = path.into_iter();
        let mut curr = self.scope;
        let down = std::iter::from_fn(|| {
            let Some(child) = curr.items().get(path.next()?) else { return Some(None) };
            curr = child;
            let Scope::Type { name, .. } = *child else { panic!("Invalid scope") };
            Some(Some(name.into()))
        });

        Some(QualName::new(
            package.map(Into::into),
            up.chain(down).collect::<Option<_>>()?,
        ))
    }

    fn search_one(&self, name: &'a str) -> Option<Cow<'_, ScopeRef<'a>>> {
        if self.scope.items().contains_key(name) {
            Some(Cow::Borrowed(self))
        } else if let Some(ref parent) = self.parent {
            parent.search_one(name)
        } else {
            let Scope::Package { name: my_name, .. } = self.scope else {
                panic!("Invalid scope reference");
            };

            if my_name.map_or(false, |n| n == name) {
                Some(Cow::Borrowed(self))
            } else {
                self.global.resolve_one(name).map(|(_, s)| Cow::Owned(s))
            }
        }
    }

    #[inline]
    pub fn search(&self, path: impl IntoIterator<Item = &'a str>) -> Option<QualName<'a>> {
        let mut path = path.into_iter();
        let base = path.next().expect("Invalid type name");
        let owner = self.search_one(base);
        owner?.qualify(path)
    }
}
