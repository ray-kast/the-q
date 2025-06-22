use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    hash::Hash,
    mem,
};

use hashbrown::{HashMap, HashSet};

use super::{
    prelude::*,
    test_tools::EGraphParts,
    trace::{self, SnapshotEGraph},
    ClassNodes, EGraphTrace, EGraphWriteTrace, ENode,
};
use crate::{
    dot,
    union_find::{ClassId, NoNode, UnionFind, Unioned},
};

struct EClassData<F, C> {
    parents: BTreeMap<ENode<F, C>, ClassId<C>>,
}

impl<F: fmt::Debug, C> fmt::Debug for EClassData<F, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { parents } = self;
        f.debug_struct("EClassData")
            .field("parents", parents)
            .finish()
    }
}

impl<F, C> Clone for EClassData<F, C> {
    fn clone(&self) -> Self {
        Self {
            parents: self.parents.clone(),
        }
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
    node_classes: BTreeMap<ENode<F, C>, ClassId<C>>,
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

impl<F, C> Clone for EGraph<F, C> {
    fn clone(&self) -> Self {
        Self {
            uf: self.uf.clone(),
            class_data: self.class_data.clone(),
            node_classes: self.node_classes.clone(),
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

impl<F: Ord + Hash, C> From<EGraph<F, C>> for EGraphParts<F, C> {
    fn from(eg: EGraph<F, C>) -> Self {
        let EGraph {
            uf,
            class_data: _,
            node_classes,
        } = eg;

        let node_classes = node_classes
            .into_iter()
            .map(|(n, c)| (n, uf.find(c).unwrap()))
            .collect();

        EGraphParts { uf, node_classes }
    }
}

impl<F: Ord, C> EGraphCore for EGraph<F, C> {
    type Class = C;
    type FuncSymbol = F;

    fn add(&mut self, mut node: ENode<F, C>) -> Result<ClassId<C>, NoNode> {
        node.canonicalize_classes(&self.uf)?;
        Ok(if let Some(&class) = self.node_classes.get(&node) {
            assert_eq!(class, self.uf.find(class).unwrap());
            class
        } else {
            let class = self.uf.add();
            assert!(self.class_data.insert(class, EClassData::new()).is_none());

            for &arg in node.args() {
                assert!(self
                    .class_data
                    .get_mut(&arg)
                    .unwrap()
                    .parents
                    .insert(node.clone(), class)
                    .is_none_or(|c| class == c));
            }

            self.node_classes.insert(node, class);
            class
        })
    }
}

impl<F: Ord + Hash, C> EGraphRead for EGraph<F, C> {
    type Hasher = hashbrown::DefaultHashBuilder;

    #[inline]
    fn find(&self, class: ClassId<C>) -> Result<ClassId<C>, NoNode> { self.uf.find(class) }

    #[inline]
    fn canonicalize(&self, node: &mut ENode<F, C>) -> Result<bool, NoNode> {
        node.canonicalize_classes(&self.uf)
    }

    #[inline]
    fn is_canonical(&self, node: &ENode<F, C>) -> Result<bool, NoNode> {
        node.classes_canonical(&self.uf)
    }

    fn class_nodes(&self) -> ClassNodes<Self, Self::Hasher> {
        self.node_classes
            .iter()
            .fold(HashMap::new(), |mut m: HashMap<_, HashSet<_>>, (n, &c)| {
                assert!(n.is_canonical(self).unwrap());
                assert!(m.entry(self.uf.find(c).unwrap()).or_default().insert(n));
                m
            })
    }

    #[inline]
    fn dot<M: trace::dot::Formatter<Self::FuncSymbol>>(&self, f: M) -> dot::Graph<'static> {
        trace::dot_graph(f, self.class_nodes())
    }
}

impl<F: Ord + Hash, C> EGraphWriteTrace for EGraph<F, C> {
    fn merge_trace<T: EGraphTrace<F, C>>(
        &mut self,
        a: ClassId<C>,
        b: ClassId<C>,
        t: &mut T,
    ) -> Result<Unioned<C>, NoNode> {
        let ret = self.merge_impl(a, b, &mut self.uf.clone(), t);
        self.assert_invariants(true);
        ret
    }
}

impl<F: Ord + Hash, C> EGraph<F, C> {
    #[inline]
    fn trace<T: EGraphTrace<F, C>, G: FnOnce() -> I, I: IntoIterator<Item = ClassId<C>>>(
        &self,
        t: &mut T,
        current: G,
    ) {
        t.graph(|g| {
            let nodes = trace::snapshot_graph(
                g,
                self.node_classes.iter().fold(
                    BTreeMap::new(),
                    |mut m: BTreeMap<_, BTreeSet<_>>, (n, &c)| {
                        assert!(m.entry(self.uf.find(c).unwrap()).or_default().insert(n));
                        m
                    },
                ),
            );

            let mut seen = BTreeSet::new();
            for class in current() {
                let data = self.class_data.get(&class).unwrap();
                for (parent, par_id) in &data.parents {
                    let parent = parent.to_canonical(self).unwrap();

                    if !seen.insert((class, parent.clone(), par_id)) {
                        continue;
                    }

                    g.parent_edge(
                        nodes.class_reps.get(&class).unwrap(),
                        nodes.node_ids.get(&parent).unwrap(),
                        nodes.class_reps.get(par_id),
                        None,
                    );
                }
            }
        });
    }

    fn merge_impl<T: EGraphTrace<F, C>>(
        &mut self,
        a: ClassId<C>,
        b: ClassId<C>,
        old_uf: &mut UnionFind<C>,
        t: &mut T,
    ) -> Result<Unioned<C>, NoNode> {
        self.trace(t, || [self.uf.find(a).unwrap(), self.uf.find(b).unwrap()]);

        old_uf.clone_from(&self.uf);
        let union = self.uf.union(a, b)?;
        let Unioned { root, unioned } = union;
        t.hl_class(root);

        if let Some(unioned) = unioned {
            t.hl_class(unioned);

            let EClassData { parents } = self.class_data.remove(&unioned).unwrap();
            let root_data = self.class_data.get_mut(&root).unwrap();

            let mut to_merge = vec![];
            // let mut new_parents = HashMap::with_capacity(root_data.parents.len());
            // let mut new_nodes = HashMap::with_capacity(root_data.parents.len() + parents.len());
            let mut new_parents = BTreeMap::new();
            let mut new_nodes = BTreeMap::new();
            for (mut old_par, par_class) in
                parents.into_iter().chain(mem::take(&mut root_data.parents))
            {
                use std::collections::btree_map::Entry;

                assert!(old_par.args().iter().any(|&c| c == unioned || c == root));
                old_par.canonicalize_classes(old_uf).unwrap();

                let par_class = self.uf.find(par_class).unwrap();
                let _old = self.node_classes.remove(&old_par);

                old_par.canonicalize_classes(&self.uf).unwrap();
                let new_par = old_par;

                match new_parents.entry(new_par.clone()) {
                    Entry::Occupied(mut o) => {
                        let other_par_class = o.insert(par_class);
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

            self.trace(t, || [root]);
            t.hl_class(root);
            t.hl_merges(to_merge.iter().copied());

            for (a, b) in to_merge {
                self.merge_impl(a, b, old_uf, t).unwrap();
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
        }

        for (&class, EClassData { parents }) in &self.class_data {
            assert_eq!(class, self.uf.find(class).unwrap());

            if merged {
                let mut seen = BTreeMap::new();
                for (par, &class) in parents {
                    let canon = par.to_canonical(self).unwrap();
                    let class = self.uf.find(class).unwrap();

                    assert_eq!(
                        class,
                        self.uf
                            .find(*self.node_classes.get(&canon).unwrap())
                            .unwrap(),
                        "Node parent class is different than registered node class"
                    );

                    if let Some(&other) = seen.get(&canon) {
                        assert_eq!(
                            other, class,
                            "Class referents canonicalize to multiple duplicate classes"
                        );
                    } else {
                        seen.insert(canon, class);
                    }
                }
            }
        }
    }
}
