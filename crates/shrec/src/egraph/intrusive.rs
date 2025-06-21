use std::{
    borrow,
    collections::{BTreeMap, BTreeSet},
    fmt, mem, ops,
    sync::Arc,
};

use super::{
    prelude::*, test_tools::EGraphParts, trace, ClassNodes, EGraphTrace, EGraphWriteTrace, ENode,
};
use crate::{
    dot,
    union_find::{
        linked_arc::{AsNode, LinkedNode, NoRank},
        ClassId, NoNode, UnionFind, Unioned,
    },
};

#[derive(Default, PartialEq, Eq, PartialOrd, Ord)]
struct ArcKey<T>(Arc<T>);

impl<T: fmt::Debug> fmt::Debug for ArcKey<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { fmt::Debug::fmt(&self.0, f) }
}

impl<T> From<T> for ArcKey<T> {
    #[inline]
    fn from(value: T) -> Self { Self(value.into()) }
}

impl<T> From<Arc<T>> for ArcKey<T> {
    #[inline]
    fn from(value: Arc<T>) -> Self { Self(value) }
}

impl<F, C> ops::Deref for ArcKey<CongrNode<F, C>> {
    type Target = ENode<F, C>;

    #[inline]
    fn deref(&self) -> &ENode<F, C> { &self.0 }
}

impl<F, C> AsRef<ENode<F, C>> for ArcKey<CongrNode<F, C>> {
    #[inline]
    fn as_ref(&self) -> &ENode<F, C> { self }
}

impl<F, C> borrow::Borrow<ENode<F, C>> for ArcKey<CongrNode<F, C>> {
    #[inline]
    fn borrow(&self) -> &ENode<F, C> { self }
}

// TODO: it would be nice if this wasn't always stored as Arc
struct CongrNode<F, C> {
    node: ENode<F, C>,
    uf: LinkedNode<Self>,
}

impl<F: fmt::Debug, C> fmt::Debug for CongrNode<F, C> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { node, uf } = self;
        f.debug_tuple("CongrNode").field(node).field(uf).finish()
    }
}

impl<F: Eq, C> Eq for CongrNode<F, C> {}

impl<F: PartialEq, C> PartialEq for CongrNode<F, C> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        let Self {
            node: l_node,
            uf: _,
        } = self;
        let Self {
            node: r_node,
            uf: _,
        } = other;
        l_node.eq(r_node)
    }
}

impl<F: Ord, C> Ord for CongrNode<F, C> {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let Self {
            node: l_node,
            uf: _,
        } = self;
        let Self {
            node: r_node,
            uf: _,
        } = other;
        l_node.cmp(r_node)
    }
}

impl<F: PartialOrd, C> PartialOrd for CongrNode<F, C> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let Self {
            node: l_node,
            uf: _,
        } = self;
        let Self {
            node: r_node,
            uf: _,
        } = other;
        l_node.partial_cmp(r_node)
    }
}

// TODO: remove if not used
// impl<F: Hash, C> Hash for CongrNode<F, C> {
//     fn hash<H: Hasher>(&self, state: &mut H) {
//         let Self { node, uf: _ } = self;
//         node.hash(state);
//     }
// }

impl<F, C> ops::Deref for CongrNode<F, C> {
    type Target = ENode<F, C>;

    #[inline]
    fn deref(&self) -> &Self::Target { &self.node }
}

impl<F, C> ops::DerefMut for CongrNode<F, C> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.node }
}

impl<F, C> AsRef<ENode<F, C>> for CongrNode<F, C> {
    fn as_ref(&self) -> &ENode<F, C> { self }
}

impl<F, C> AsMut<ENode<F, C>> for CongrNode<F, C> {
    fn as_mut(&mut self) -> &mut ENode<F, C> { self }
}

impl<F, C> borrow::Borrow<ENode<F, C>> for CongrNode<F, C> {
    fn borrow(&self) -> &ENode<F, C> { self }
}

impl<F, C> borrow::BorrowMut<ENode<F, C>> for CongrNode<F, C> {
    fn borrow_mut(&mut self) -> &mut ENode<F, C> { self }
}

impl<F, C> AsNode for CongrNode<F, C> {
    type Extra = ();
    type Rank = NoRank;

    #[inline]
    fn as_node(&self) -> &LinkedNode<Self> { &self.uf }
}

struct EClassData<F, C> {
    parents: BTreeMap<ArcKey<CongrNode<F, C>>, ClassId<C>>,
}

impl<F: fmt::Debug, C> fmt::Debug for EClassData<F, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { parents } = self;
        f.debug_struct("EClassData")
            .field("parents", parents)
            .finish()
    }
}

impl<F: Ord, C> EClassData<F, C> {
    fn new() -> Self {
        Self {
            parents: BTreeMap::new(),
        }
    }
}

pub struct EGraph<F, C> {
    uf: UnionFind<C>,
    class_data: BTreeMap<ClassId<C>, EClassData<F, C>>,
    node_classes: BTreeMap<ArcKey<CongrNode<F, C>>, ClassId<C>>,
}

impl<F: fmt::Debug, C> fmt::Debug for EGraph<F, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            uf,
            class_data,
            node_classes,
        } = self;
        f.debug_struct("EGraph")
            .field("uf", uf)
            .field("class_data", class_data)
            .field("node_classes", node_classes)
            .finish()
    }
}

impl<F: Ord, C> Clone for EGraph<F, C> {
    fn clone(&self) -> Self {
        #[expect(clippy::mutable_key_type, reason = "We don't compare that bit")]
        let node_classes: BTreeMap<_, _> = self
            .node_classes
            .iter()
            .map(|(k, v)| {
                (
                    ArcKey(LinkedNode::from_id(*k.0.uf.id(), |uf| CongrNode {
                        node: k.as_ref().clone(),
                        uf,
                    })),
                    *v,
                )
            })
            .collect();

        let class_data = self
            .class_data
            .iter()
            .map(|(k, EClassData { parents })| {
                (*k, EClassData {
                    parents: parents
                        .iter()
                        .map(|(k, v)| {
                            let k = k.0.uf.root().canon().unwrap();
                            (
                                Arc::clone(&node_classes.get_key_value(&k.node).unwrap().0 .0)
                                    .into(),
                                *v,
                            )
                        })
                        .collect(),
                })
            })
            .collect();

        Self {
            uf: self.uf.clone(),
            class_data,
            node_classes,
        }
    }
}

impl<F, C> Default for EGraph<F, C> {
    fn default() -> Self { Self::new() }
}

impl<F, C> EGraph<F, C> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            uf: UnionFind::new(),
            class_data: BTreeMap::new(),
            node_classes: BTreeMap::new(),
        }
    }
}

impl<F: Ord, C> From<EGraph<F, C>> for EGraphParts<F, C> {
    fn from(eg: EGraph<F, C>) -> Self {
        let EGraph {
            uf,
            class_data: _,
            node_classes,
        } = eg;

        let node_classes = node_classes
            .into_iter()
            .map(|(k, v)| (k.as_ref().clone(), uf.find(v).unwrap()))
            .collect();

        EGraphParts { uf, node_classes }
    }
}

impl<F: Ord, C> EGraphCore for EGraph<F, C> {
    type Class = C;
    type FuncSymbol = F;

    fn add(&mut self, mut node: ENode<F, C>) -> Result<ClassId<C>, NoNode> {
        node.canonicalize_classes(&self.uf)?;
        Ok(if let Some(&klass) = self.node_classes.get(&node) {
            assert_eq!(klass, self.uf.find(klass).unwrap());
            klass
        } else {
            let klass = self.uf.add();
            let node = LinkedNode::new_arc(|uf| CongrNode { node, uf });
            assert!(self.class_data.insert(klass, EClassData::new()).is_none());

            for &arg in node.args() {
                assert!(self
                    .class_data
                    .get_mut(&arg)
                    .unwrap()
                    .parents
                    .insert(Arc::clone(&node).into(), klass)
                    .is_none_or(|c| klass == c));
            }

            self.node_classes.insert(node.into(), klass);
            klass
        })
    }
}

impl<F: Ord, C> EGraphRead for EGraph<F, C> {
    #[inline]
    fn find(&self, klass: ClassId<C>) -> Result<ClassId<C>, NoNode> { self.uf.find(klass) }

    #[inline]
    fn canonicalize(&self, node: &mut ENode<F, C>) -> Result<bool, NoNode> {
        node.canonicalize_classes(&self.uf)
    }

    #[inline]
    fn is_canonical(&self, node: &ENode<F, C>) -> Result<bool, NoNode> {
        node.classes_canonical(&self.uf)
    }

    #[must_use]
    fn class_nodes(&self) -> ClassNodes<Self> {
        self.node_classes.iter().fold(
            BTreeMap::new(),
            |mut m: BTreeMap<_, BTreeSet<_>>, (n, &c)| {
                assert!(n.is_canonical(self).unwrap());
                assert!(m.entry(self.uf.find(c).unwrap()).or_default().insert(n));
                m
            },
        )
    }

    #[inline]
    #[must_use]
    fn dot<M: trace::dot::Formatter<Self::FuncSymbol>>(&self, f: M) -> dot::Graph<'static> {
        trace::dot_graph(f, self.class_nodes())
    }
}

impl<F: Ord, C> EGraphWriteTrace for EGraph<F, C> {
    fn merge_trace<T: EGraphTrace<F, C>>(
        &mut self,
        a: ClassId<C>,
        b: ClassId<C>,
        t: &mut T,
    ) -> Result<Unioned<C>, NoNode> {
        let ret = self.merge_impl(a, b, t);
        self.assert_invariants(true);
        ret
    }
}

impl<F: Ord, C> EGraph<F, C> {
    #[inline]
    fn trace<T: EGraphTrace<F, C>>(&self, t: &mut T) {
        t.graph(|g| {
            trace::snapshot_graph(g, {
                self.node_classes.iter().fold(
                    BTreeMap::new(),
                    |mut m: BTreeMap<_, BTreeSet<_>>, (n, &c)| {
                        assert!(m
                            .entry(self.uf.find(c).unwrap())
                            .or_default()
                            .insert(n.as_ref()));
                        m
                    },
                )
            });
        });
    }

    fn merge_impl<T: EGraphTrace<F, C>>(
        &mut self,
        a: ClassId<C>,
        b: ClassId<C>,
        t: &mut T,
    ) -> Result<Unioned<C>, NoNode> {
        self.trace(t);

        let union = self.uf.union(a, b)?;
        let Unioned { root, unioned } = union;
        t.hl_class(root);

        if let Some(unioned) = unioned {
            t.hl_class(unioned);

            let EClassData { parents } = self.class_data.remove(&unioned).unwrap();
            let root_data = self.class_data.get_mut(&root).unwrap();

            let mut to_merge = vec![];
            // TODO: if you bring these back, remove the mem::take's
            // let mut new_parents = HashMap::new(root_data.parents.len());
            // let mut new_nodes = HashMap::with_capacity(root_data.parents.len() + parents.len());
            #[expect(clippy::mutable_key_type, reason = "We don't compare that bit")]
            let mut new_parents = BTreeMap::new();
            #[expect(clippy::mutable_key_type, reason = "We don't compare that bit")]
            let mut new_nodes = BTreeMap::new();
            for (old_par, par_class) in parents.into_iter().chain(mem::take(&mut root_data.parents))
            {
                use std::collections::btree_map::Entry;

                assert!(old_par.args().iter().any(|&c| c == unioned || c == root));

                let old_par = ArcKey(old_par.0.uf.root().canon().unwrap());

                let par_class = self.uf.find(par_class).unwrap();
                let _old = self.node_classes.remove(&old_par);

                let mut new_par = old_par.as_ref().clone();
                let was_not_canon = new_par.canonicalize_classes(&self.uf).unwrap();

                debug_assert_eq!(was_not_canon, new_par != *old_par);

                let new_par = if was_not_canon {
                    let new_par = LinkedNode::new_arc(|uf| CongrNode { node: new_par, uf });

                    old_par.0.uf.root().merge_into(&new_par.uf.root());
                    debug_assert_eq!(
                        Arc::as_ptr(&old_par.0.uf.root()),
                        Arc::as_ptr(&new_par.uf.root())
                    );

                    drop(old_par);
                    new_par.into()
                } else {
                    old_par
                };

                match new_parents.entry(ArcKey(Arc::clone(&new_par.0))) {
                    Entry::Occupied(mut o) => {
                        let other_par_class = o.insert(par_class);

                        new_par.0.uf.root().merge_into(&o.key().0.uf.root());
                        to_merge.push((par_class, other_par_class));

                        assert!(
                            new_nodes.contains_key(&new_par)
                                || self.node_classes.contains_key(&new_par)
                        );
                    },
                    Entry::Vacant(v) => {
                        v.insert(par_class);

                        assert!(new_nodes.insert(new_par, par_class).is_none());
                    },
                }
            }

            assert!(mem::replace(&mut root_data.parents, new_parents).is_empty());
            let expected_len = self.node_classes.len() + new_nodes.len();
            self.node_classes.extend(new_nodes);
            assert_eq!(self.node_classes.len(), expected_len);

            self.assert_invariants(false);

            self.trace(t);
            t.hl_class(root);
            t.hl_merges(to_merge.iter().copied());

            for (a, b) in to_merge {
                self.merge_impl(a, b, t).unwrap();
            }
        } else {
            self.assert_invariants(false);
        }

        Ok(union)
    }

    #[cfg(not(any(test, feature = "test")))]
    #[inline]
    fn assert_invariants(&self, _: bool) { let _ = self; }

    #[cfg(any(test, feature = "test"))]
    fn assert_invariants(&self, merged: bool) {
        for node in self.node_classes.keys() {
            assert!(node.classes_canonical(&self.uf).unwrap());

            assert!(node.0 == node.0.uf.root().canon().unwrap_or_else(|| unreachable!()));
        }

        for (&klass, EClassData { parents }) in &self.class_data {
            assert_eq!(klass, self.uf.find(klass).unwrap());

            for par in parents.keys() {
                // Verify this exists
                assert!(par.0.uf.root().canon().is_some());
            }

            if merged {
                let mut seen = BTreeMap::new();
                for (par, &klass) in parents {
                    let canon = par.to_canonical(self).unwrap();
                    let klass = self.uf.find(klass).unwrap();

                    assert_eq!(
                        klass,
                        self.uf
                            .find(*self.node_classes.get(&canon).unwrap())
                            .unwrap(),
                        "Node parent class is different than registered node class"
                    );

                    if let Some(&other) = seen.get(&canon) {
                        assert_eq!(
                            other, klass,
                            "Class referents canonicalize to multiple duplicate classes"
                        );
                    } else {
                        seen.insert(canon, klass);
                    }
                }
            }
        }
    }
}
