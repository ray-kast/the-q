use std::{
    fmt,
    hash::{Hash, Hasher},
    sync::Arc,
};

use super::EGraphRead;
use crate::union_find::{ClassId, NoNode, UnionFind};

// TODO: probably memoize this rather than use Arc
pub struct ENode<F, C>(Arc<F>, Arc<[ClassId<C>]>);

impl<F, C> ENode<F, C> {
    #[must_use]
    pub fn op(&self) -> &F { &self.0 }

    #[must_use]
    pub fn args(&self) -> &[ClassId<C>] { &self.1 }

    pub fn classes_canonical(&self, uf: &UnionFind<C>) -> Result<bool, NoNode> {
        for &arg in &*self.1 {
            if arg != uf.find(arg)? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    #[inline]
    pub fn is_canonial<G: EGraphRead<FuncSymbol = F, Class = C>>(
        &self,
        eg: &G,
    ) -> Result<bool, NoNode> {
        eg.is_canonical(self)
    }

    pub fn canonicalize_classes(&mut self, uf: &UnionFind<C>) -> Result<bool, NoNode> {
        enum State<'a, C> {
            Ref(&'a Arc<[ClassId<C>]>),
            Mut(&'a mut [ClassId<C>]),
        }

        let mut args = State::Ref(&self.1);
        for i in 0..self.1.len() {
            match args {
                State::Ref(a) => {
                    // SAFETY: i is bounded by self.1.len()
                    let arg = unsafe { a.get_unchecked(i) };
                    let root = uf.find(*arg)?;
                    if root != *arg {
                        let args_mut = Arc::make_mut(&mut self.1);
                        // SAFETY: i is bounded by self.1.len()
                        *unsafe { args_mut.get_unchecked_mut(i) } = root;
                        args = State::Mut(args_mut);
                    }
                },
                State::Mut(ref mut m) => {
                    // SAFETY: i is bounded by self.1.len()
                    let arg = unsafe { m.get_unchecked_mut(i) };
                    *arg = uf.find(*arg)?;
                },
            }
        }

        Ok(matches!(args, State::Mut(..)))
    }

    #[inline]
    pub fn canonicalize<G: EGraphRead<FuncSymbol = F, Class = C>>(
        &mut self,
        eg: &G,
    ) -> Result<bool, NoNode> {
        eg.canonicalize(self)
    }

    #[inline]
    pub fn to_canonical<G: EGraphRead<FuncSymbol = F, Class = C>>(
        &self,
        eg: &G,
    ) -> Result<Self, NoNode> {
        let mut ret = self.clone();
        ret.canonicalize(eg).map(|_: bool| ret)
    }
}

impl<F: fmt::Debug, C> fmt::Debug for ENode<F, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[repr(transparent)]
        struct Args<'a, C>(&'a Arc<[ClassId<C>]>);

        impl<C> fmt::Debug for Args<'_, C> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_list()
                    .entries(self.0.iter().map(|c| c.id()))
                    .finish()
            }
        }

        let Self(op, args) = self;
        f.debug_tuple("ENode")
            .field(&op)
            .field(&Args(args))
            .finish()
    }
}

impl<F, C> Clone for ENode<F, C> {
    fn clone(&self) -> Self { Self(Arc::clone(&self.0), Arc::clone(&self.1)) }
}

impl<F: PartialEq, C> PartialEq for ENode<F, C> {
    fn eq(&self, other: &Self) -> bool {
        let Self(l_op, l_args) = self;
        let Self(r_op, r_args) = other;
        l_op == r_op && l_args == r_args
    }
}

impl<F: Eq, C> Eq for ENode<F, C> {}

impl<F: Ord, C> Ord for ENode<F, C> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let Self(l_op, l_args) = self;
        let Self(r_op, r_args) = other;
        l_op.cmp(r_op).then_with(|| l_args.cmp(r_args))
    }
}

impl<F: PartialOrd, C> PartialOrd for ENode<F, C> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let Self(l_op, l_args) = self;
        let Self(r_op, r_args) = other;
        Some(l_op.partial_cmp(r_op)?.then_with(|| l_args.cmp(r_args)))
    }
}

impl<F: Hash, C> Hash for ENode<F, C> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let Self(op, args) = self;
        op.hash(state);
        args.hash(state);
    }
}

impl<F, C> ENode<F, C> {
    pub const fn new(op: Arc<F>, args: Arc<[ClassId<C>]>) -> Self { Self(op, args) }
}
