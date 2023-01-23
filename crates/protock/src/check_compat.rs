use std::fmt;

use crate::compat_pair::{CompatPair, SideInclusive};

#[derive(Debug, Default)]
pub struct CompatLog {
    errors: Vec<CompatError>,
    warnings: Vec<CompatError>,
}

impl CompatLog {
    pub fn run<E>(f: impl FnOnce(&mut Self), e: impl FnOnce() -> E) -> Result<(), E> {
        let mut me = Self::default();
        f(&mut me);
        me.finish(e)
    }

    pub fn finish<E>(self, error: impl FnOnce() -> E) -> Result<(), E> {
        let Self { errors, warnings } = self;

        for warn in warnings {
            tracing::warn!("{warn}");
        }

        let err = !errors.is_empty();
        for err in errors {
            tracing::error!("{err}");
        }

        err.then(|| Err(error())).unwrap_or(Ok(()))
    }
}

pub trait CheckCompat {
    type Context<'a>;

    fn check_compat(
        ck: CompatPair<&'_ Self>,
        cx: CompatPair<Self::Context<'_>>,
        log: &mut CompatLog,
    );
}

#[derive(Debug)]
pub struct CompatError {
    cx: SideInclusive<Box<dyn fmt::Debug>>,
    message: String,
}

impl fmt::Display for CompatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:?}) {}", self.cx.display(), self.message)
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
    pub fn err(self, log: &mut CompatLog) { log.errors.push(self); }

    #[inline]
    pub fn warn(self, log: &mut CompatLog) { log.warnings.push(self); }
}
