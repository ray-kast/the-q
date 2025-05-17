use core::fmt;
use std::{
    cmp::Ordering,
    mem,
    sync::{Arc, Weak},
};

use spin::mutex::SpinMutex;

use super::disjoint_set::{NoNode, RankedUnion};
use crate::free::{Free, Succ};

#[derive(Debug, Clone, Copy)]
pub struct LinkedArc;

const DROP_ERROR: &str = "Node dropped while still in use";

struct DebugStr(&'static str);

impl fmt::Debug for DebugStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.0) }
}

struct RootInner<T: ?Sized, R, X> {
    canon: Weak<T>,
    rank: R,
    next: Option<Arc<RootRef<T, R, X>>>,
}

pub struct RootRef<T: ?Sized, R, X> {
    id: R,
    inner: SpinMutex<RootInner<T, R, X>>,
}

impl<T: fmt::Debug, R: fmt::Debug, X> fmt::Debug for RootRef<T, R, X> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { id, inner } = self;

        if let Some(inner) = inner.try_lock() {
            let RootInner { canon, rank, next } = &*inner;
            let canon = canon.upgrade();
            f.debug_struct("RootRef")
                .field("id", &id)
                .field(
                    "canon",
                    if let Some(canon) = canon.as_deref() {
                        canon
                    } else {
                        &DebugStr("<dropped>")
                    },
                )
                .field("rank", &rank)
                .field("next", &next)
                .finish()
        } else {
            f.debug_tuple("RootRef")
                .field(&id)
                .field(&DebugStr("<locked>"))
                .finish()
        }
    }
}

impl<T: ?Sized, R, X> RootRef<T, R, X> {
    fn find(self: &mut Arc<Self>) -> Option<&mut Arc<Self>> {
        let mut inner = self.inner.lock();

        if let Some(root) = inner.next.as_mut() {
            root.find();

            let root = Arc::clone(root);
            drop(inner);

            *self = root;
            Some(self)
        } else {
            None
        }
    }

    #[inline]
    pub fn id(&self) -> &R { &self.id }

    #[inline]
    pub fn canon(&self) -> Option<Arc<T>> { self.inner.lock().canon.upgrade() }

    #[inline]
    pub fn is_root(&self) -> bool { self.inner.lock().next.is_none() }
}

impl<T: AsNode<Rank = R, Extra = X> + ?Sized, R: NodeRank, X: LinkedNodeExtra<T>> RootRef<T, R, X> {
    pub fn merge_into(self: &Arc<Self>, root: &Arc<Self>) -> bool {
        if Arc::ptr_eq(self, root) {
            return false;
        }

        let root_rc = Arc::clone(root);
        let mut root = root.inner.lock();
        let mut merged = self.inner.lock();

        assert!(root.next.is_none() && merged.next.is_none());

        let root_canon = root.canon.upgrade().expect(DROP_ERROR);
        let merged_canon = merged.canon.upgrade().expect(DROP_ERROR);

        X::merge(&root_canon.as_node().extra, &merged_canon.as_node().extra);

        root.rank = root.rank.join(merged.rank);
        merged.next = Some(root_rc);

        true
    }
}

pub trait LinkedNodeExtra<T: ?Sized> {
    fn new(node: &Weak<T>) -> Self;

    fn merge(root: &Self, merged: &Self);
}

impl<T> LinkedNodeExtra<T> for () {
    fn new(_: &Weak<T>) {}

    fn merge((): &(), (): &()) {}
}

pub struct CircularList<T> {
    next: SpinMutex<Weak<T>>,
}

impl<T> fmt::Debug for CircularList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CircularList")
    }
}

impl<T> LinkedNodeExtra<T> for CircularList<T> {
    fn new(node: &Weak<T>) -> Self {
        Self {
            next: Weak::clone(node).into(),
        }
    }

    fn merge(root: &Self, merged: &Self) {
        let mut root = root.next.lock();
        let mut merged = merged.next.lock();
        mem::swap(&mut *root, &mut *merged);
    }
}

pub trait NodeRank: Copy {
    const SINGLE: Self;

    #[must_use]
    fn join(self, other: Self) -> Self;
}

macro_rules! int_rank {
    () => {};
    ($ty:ty $(, $($tts:tt)*)?) => {
        impl NodeRank for $ty {
            const SINGLE: Self = 1;

            #[inline]
            fn join(self, other: Self) -> Self {
                self.checked_add(other).unwrap_or_else(|| unreachable!())
            }
        }

        $(int_rank!($($tts)*);)?
    };
}

int_rank!(u8, u16, u32, u64, u128, usize);

#[derive(Clone, Copy)]
pub struct NoRank;

impl fmt::Debug for NoRank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("∅") }
}

impl NodeRank for NoRank {
    const SINGLE: Self = Self;

    #[inline]
    fn join(self, other: Self) -> Self {
        let Self = self;
        let Self = other;
        Self
    }
}

pub struct LinkedNodeBase<T: ?Sized, R, X> {
    id: R,
    root: SpinMutex<Arc<RootRef<T, R, X>>>,
    extra: X,
}

pub type LinkedNode<T> = LinkedNodeBase<T, <T as AsNode>::Rank, <T as AsNode>::Extra>;

impl<T: fmt::Debug, R: fmt::Debug, X: fmt::Debug> fmt::Debug for LinkedNodeBase<T, R, X> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { id, root, extra } = self;
        let root = root.try_lock();

        f.debug_tuple("LinkedNode")
            .field(id)
            .field(if let Some(root) = root.as_deref() {
                root
            } else {
                &DebugStr("<locked>")
            })
            .field(extra)
            .finish()
    }
}

impl<T, R: NodeRank, X: LinkedNodeExtra<T>> LinkedNodeBase<T, R, X> {
    #[must_use]
    pub fn from_id<F: FnOnce(Self) -> T>(id: R, f: F) -> Arc<T> {
        Arc::new_cyclic(|n| {
            f(Self {
                id,
                root: Arc::new(RootRef {
                    id,
                    inner: RootInner {
                        canon: Weak::clone(n),
                        rank: R::SINGLE,
                        next: None,
                    }
                    .into(),
                })
                .into(),
                extra: X::new(n),
            })
        })
    }
}

impl<T, X: LinkedNodeExtra<T>> LinkedNodeBase<T, NoRank, X> {
    pub fn new_arc<F: FnOnce(Self) -> T>(f: F) -> Arc<T> { Self::from_id(NoRank, f) }
}

impl<T, R: NodeRank + Succ, X: LinkedNodeExtra<T>> LinkedNodeBase<T, R, X> {
    #[must_use]
    pub fn new_fresh<F: FnOnce(Self) -> T>(ids: &mut Free<R>, f: F) -> Arc<T> {
        Self::from_id(ids.fresh(), f)
    }
}

impl<T: ?Sized, R, X> LinkedNodeBase<T, R, X> {
    pub fn id(&self) -> &R { &self.id }

    pub fn root(&self) -> Arc<RootRef<T, R, X>> {
        let mut root = self.root.lock();
        root.find();
        Arc::clone(&root)
    }
}

pub trait AsNode {
    type Extra;
    type Rank;

    fn as_node(&self) -> &LinkedNodeBase<Self, Self::Rank, Self::Extra>;
}

pub trait NodeRef {
    type Value: AsNode<Extra = Self::Extra, Rank = Self::Rank>;
    type Extra;
    type Rank;

    fn node_ref(&self) -> &LinkedNodeBase<Self::Value, Self::Rank, Self::Extra>;
}

impl<T: AsNode> NodeRef for T {
    type Extra = T::Extra;
    type Rank = T::Rank;
    type Value = T;

    #[inline]
    fn node_ref(&self) -> &LinkedNodeBase<T, T::Rank, T::Extra> { self.as_node() }
}

impl<T: AsNode> NodeRef for Arc<T> {
    type Extra = T::Extra;
    type Rank = T::Rank;
    type Value = T;

    #[inline]
    fn node_ref(&self) -> &LinkedNodeBase<Self::Value, Self::Rank, Self::Extra> { self.as_node() }
}

impl<'a, T: NodeRef<Rank: Ord + NodeRank, Extra: LinkedNodeExtra<T::Value>>> RankedUnion<&'a T>
    for LinkedArc
{
    type Rank = T::Rank;
    type Root = Arc<RootRef<T::Value, T::Rank, T::Extra>>;

    #[inline]
    fn find(&self, key: &'a T) -> Result<Self::Root, NoNode<&'a T>> { Ok(key.node_ref().root()) }

    #[inline]
    fn cmp_roots(&self, a: &Self::Root, b: &Self::Root) -> Ordering { a.id.cmp(&b.id) }

    #[inline]
    fn rank(&self, root: &Self::Root) -> Option<T::Rank> { Some(root.inner.lock().rank) }

    #[inline]
    fn merge(&mut self, root: &Self::Root, merged: &Self::Root) { merged.merge_into(root); }
}

#[cfg(test)]
#[allow(dead_code)]
fn assert_impls() {
    use crate::union_find::disjoint_set;

    #[derive(Debug)]
    struct MyNode(LinkedNode<Self>);

    impl AsNode for MyNode {
        type Extra = ();
        type Rank = u64;

        #[inline]
        fn as_node(&self) -> &LinkedNode<Self> { &self.0 }
    }

    #[allow(unreachable_code)]
    fn node() -> Arc<MyNode> { LinkedNode::new_fresh(unreachable!(), MyNode) }

    disjoint_set::ranked_union(&mut LinkedArc, &node(), &node()).unwrap();
}
