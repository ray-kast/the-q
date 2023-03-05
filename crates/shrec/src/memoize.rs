use std::{borrow::Borrow, hash::Hash, ops::Deref};

use hashbrown::HashMap;

#[derive(Debug)]
pub struct Memoize<T>(HashMap<T, ()>);

impl<T> Default for Memoize<T> {
    fn default() -> Self { Self(HashMap::new()) }
}

impl<T: Clone + Eq + Hash + Deref + Borrow<T::Target>> Memoize<T>
where T::Target: Eq + Hash + Into<T>
{
    pub fn memoize(&mut self, val: T::Target) -> T {
        self.0
            .raw_entry_mut()
            .from_key(&val)
            .or_insert_with(|| (val.into(), ()))
            .0
            .clone()
    }

    pub fn memoize_owned<Q: Eq + Hash + ToOwned<Owned = T::Target> + ?Sized>(
        &mut self,
        val: &Q,
    ) -> T
    where
        T: Borrow<Q>,
    {
        self.0
            .raw_entry_mut()
            .from_key(val)
            .or_insert_with(|| (val.to_owned().into(), ()))
            .0
            .clone()
    }
}
