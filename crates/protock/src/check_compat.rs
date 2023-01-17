use std::fmt;

use crate::compat_pair::{CompatPair, SideInclusive};

pub type CompatResult = std::result::Result<(), CompatError>;

pub trait CheckCompat {
    type Context<'a>;

    fn check_compat(ck: CompatPair<&'_ Self>, cx: CompatPair<Self::Context<'_>>) -> CompatResult;
}

pub struct CompatError {
    cx: SideInclusive<Box<dyn fmt::Debug>>,
    message: String,
}

impl fmt::Display for CompatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.cx {
            SideInclusive::Reader(r) => write!(f, "({r:?} in reader) "),
            SideInclusive::Writer(w) => write!(f, "({w:?} in writer) "),
            SideInclusive::Both { reader, writer } => {
                write!(f, "({reader:?} in reader, {writer:?} in writer) ")
            },
        }?;

        write!(f, "{}", self.message)
    }
}

impl CompatError {
    // TODO: choose a better context type than dyn Debug
    pub fn new(pair: SideInclusive<impl fmt::Debug + 'static>, message: impl fmt::Display) -> Self {
        Self {
            cx: pair.map(|s| Box::new(s) as Box<(dyn fmt::Debug + 'static)>),
            message: message.to_string(),
        }
    }

    #[inline]
    #[deprecated]
    pub fn warn(self) {
        tracing::warn!("{self}");
    }
}
