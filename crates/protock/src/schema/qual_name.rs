use std::{borrow::Cow, fmt};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct QualName<'a> {
    package: Option<Cow<'a, str>>,
    path: Vec<Cow<'a, str>>,
}

impl fmt::Debug for QualName<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(package) = &self.package {
            write!(f, "'{package}'")?;
        }

        for id in &self.path {
            write!(f, ".{id}")?;
        }

        Ok(())
    }
}

impl<'a> QualName<'a> {
    #[inline]
    pub const fn new(package: Option<Cow<'a, str>>, path: Vec<Cow<'a, str>>) -> Self {
        Self { package, path }
    }

    pub fn borrowed(&self) -> QualName<'_> {
        let Self { package, path } = self;

        QualName {
            package: package.as_ref().map(|p| p.as_ref().into()),
            path: path.iter().map(|p| p.as_ref().into()).collect(),
        }
    }

    pub fn to_owned(&self) -> QualName<'static> {
        let Self { package, path } = self;

        QualName {
            package: package.as_ref().map(|p| p.as_ref().to_owned().into()),
            path: path.iter().map(|p| p.as_ref().to_owned().into()).collect(),
        }
    }

    pub fn into_owned(self) -> QualName<'static> {
        let Self { package, path } = self;

        QualName {
            package: package.map(|p| p.into_owned().into()),
            path: path.into_iter().map(|p| p.into_owned().into()).collect(),
        }
    }

    pub fn member<'b>(&'b self, memb: impl Into<Cow<'b, str>>) -> MemberQualName<'b> {
        MemberQualName {
            ty: self.borrowed(),
            memb: memb.into(),
        }
    }
}

#[derive(Clone)]
pub struct MemberQualName<'a> {
    ty: QualName<'a>,
    memb: Cow<'a, str>,
}

impl fmt::Debug for MemberQualName<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}::{}", self.ty, self.memb)
    }
}

impl MemberQualName<'_> {
    pub fn borrowed(&self) -> MemberQualName<'_> {
        let Self { ty, memb } = self;

        MemberQualName {
            ty: ty.borrowed(),
            memb: memb.as_ref().into(),
        }
    }

    pub fn to_owned(&self) -> MemberQualName<'static> {
        let Self { ty, memb } = self;

        MemberQualName {
            ty: ty.to_owned(),
            memb: memb.as_ref().to_owned().into(),
        }
    }
}
