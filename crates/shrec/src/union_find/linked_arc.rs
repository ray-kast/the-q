use std::{
    cmp::Ordering,
    mem,
    sync::{Arc, Weak},
};

use spin::mutex::SpinMutex;

use super::disjoint_set::{NoNode, RankedUnion};
use crate::free::Free;

#[derive(Debug, Clone, Copy)]
pub struct LinkedArc;

const DROP_ERROR: &str = "Node dropped while still in use";

#[derive(Debug)]
struct RootInner<T: ?Sized> {
    canon: Weak<T>,
    rank: u64,
    next: Option<Arc<RootRef<T>>>,
}

#[derive(Debug)]
pub struct RootRef<T: ?Sized> {
    id: u64,
    inner: SpinMutex<RootInner<T>>,
}

impl<T: ?Sized> RootRef<T> {
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
}

pub trait LinkedNodeExtra<T> {
    fn new(node: &Weak<T>) -> Self;

    fn merge(root: &Self, merged: &Self);
}

impl<T> LinkedNodeExtra<T> for () {
    fn new(_: &Weak<T>) {}

    fn merge((): &(), (): &()) {}
}

#[derive(Debug)]
pub struct CircularList<T> {
    next: SpinMutex<Weak<T>>,
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

#[derive(Debug)]
pub struct LinkedNode<T: ?Sized, X = ()> {
    root: SpinMutex<Arc<RootRef<T>>>,
    extra: X,
}

pub type CircularNode<T> = LinkedNode<T, CircularList<T>>;

impl<T, X: LinkedNodeExtra<T>> LinkedNode<T, X> {
    #[must_use]
    pub fn new_arc<F: FnOnce(Self) -> T>(ids: &mut Free<u64>, f: F) -> Arc<T> {
        Arc::new_cyclic(|n| {
            let id = ids.fresh();
            f(Self {
                root: Arc::new(RootRef {
                    id,
                    inner: RootInner {
                        canon: Weak::clone(n),
                        rank: 1,
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

impl<T: ?Sized, X> LinkedNode<T, X> {
    fn root(&self) -> Arc<RootRef<T>> {
        let mut root = self.root.lock();
        root.find();
        Arc::clone(&root)
    }
}

pub trait AsNode {
    type Extra;

    fn as_node(&self) -> &LinkedNode<Self, Self::Extra>;
}

pub trait NodeRef {
    type Value: AsNode<Extra = Self::Extra>;
    type Extra;

    fn node_ref(&self) -> &LinkedNode<Self::Value, Self::Extra>;
}

impl<T: AsNode> NodeRef for T {
    type Extra = T::Extra;
    type Value = T;

    #[inline]
    fn node_ref(&self) -> &LinkedNode<T, T::Extra> { self.as_node() }
}

impl<T: AsNode> NodeRef for &LinkedNode<T, T::Extra> {
    type Extra = T::Extra;
    type Value = T;

    #[inline]
    fn node_ref(&self) -> &LinkedNode<T, T::Extra> { self }
}

impl<'a, T: NodeRef<Extra: LinkedNodeExtra<T>>> RankedUnion<&'a T> for LinkedArc {
    type Rank = u64;
    type Root = Arc<RootRef<T::Value>>;

    #[inline]
    fn find(&self, key: &'a T) -> Result<Self::Root, NoNode<&'a T>> { Ok(key.node_ref().root()) }

    #[inline]
    fn cmp_roots(&self, a: &Self::Root, b: &Self::Root) -> Ordering { a.id.cmp(&b.id) }

    #[inline]
    fn rank(&self, root: &Self::Root) -> Option<u64> { Some(root.inner.lock().rank) }

    fn merge(&mut self, root: &Self::Root, merged: &Self::Root) {
        let root_rc = Arc::clone(root);
        let mut root = root.inner.lock();
        let mut merged = merged.inner.lock();

        let root_canon = root.canon.upgrade().expect(DROP_ERROR);
        let merged_canon = merged.canon.upgrade().expect(DROP_ERROR);

        T::Extra::merge(&root_canon.as_node().extra, &merged_canon.as_node().extra);

        root.rank = root.rank.checked_add(merged.rank).unwrap();
        merged.next = Some(root_rc);
    }
}

#[cfg(test)]
#[allow(dead_code)]
fn assert_impls() {
    use crate::union_find::disjoint_set;

    #[derive(Debug)]
    struct MyNode(LinkedNode<MyNode>);

    impl AsNode for MyNode {
        type Extra = ();

        #[inline]
        fn as_node(&self) -> &LinkedNode<Self> { &self.0 }
    }

    fn node() -> MyNode { todo!() }

    disjoint_set::ranked_union(&mut LinkedArc, &node(), &node()).unwrap();
}
