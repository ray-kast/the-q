use std::{mem, ops::RangeFrom};

// TODO: Step trait when
pub trait Succ {
    #[must_use]
    fn succ(self) -> Self;
}

impl<T> Succ for T
where RangeFrom<T>: Iterator<Item = T>
{
    fn succ(self) -> Self { (self..).next().unwrap() }
}

#[derive(Debug, Default)]
#[repr(transparent)]
pub struct Free<T>(T);

impl<T> From<T> for Free<T> {
    fn from(val: T) -> Self { Self(val) }
}

impl<T: Clone + Succ> Free<T> {
    #[must_use]
    pub fn fresh(&mut self) -> T {
        let succ = self.0.clone().succ();
        mem::replace(&mut self.0, succ)
    }
}
