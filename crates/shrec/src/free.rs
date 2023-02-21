use std::mem;

pub trait Succ {
    fn succ(self) -> Self;
}

impl<T: From<u8> + std::ops::Add<T, Output = T>> Succ for T {
    fn succ(self) -> Self { self + 1.into() }
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
