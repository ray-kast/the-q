use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, mem,
};

use hashbrown::HashSet;

use super::{
    prelude::*,
    test_tools::EGraphParts,
    trace::{self, SnapshotEGraph},
    ClassNodes, EGraphTrace, ENode,
};
use crate::{
    dot,
    union_find::{ClassId, NoNode, UnionFind, Unioned},
};

struct EClassData<F, C> {
    parents: BTreeMap<ClassId<ENode<F, C>>, ClassId<C>>,
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

impl<F, C> EClassData<F, C> {
    #[inline]
    fn new() -> Self {
        Self {
            parents: BTreeMap::new(),
        }
    }
}

struct NodeData<F, C> {
    node: ENode<F, C>,
    class: ClassId<C>,
}

impl<F: fmt::Debug, C> fmt::Debug for NodeData<F, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { node, class } = self;
        f.debug_struct("NodeData")
            .field("node", node)
            .field("class", class)
            .finish()
    }
}

impl<F, C> Clone for NodeData<F, C> {
    fn clone(&self) -> Self {
        Self {
            node: self.node.clone(),
            class: self.class,
        }
    }
}

impl<F, C> NodeData<F, C> {
    #[inline]
    fn new(node: ENode<F, C>, class: ClassId<C>) -> Self { Self { node, class } }
}

pub struct EGraph<F, C> {
    eq_uf: UnionFind<C>,
    congr_uf: UnionFind<ENode<F, C>>,
    class_data: BTreeMap<ClassId<C>, EClassData<F, C>>,
    node_data: BTreeMap<ClassId<ENode<F, C>>, NodeData<F, C>>,
    node_congr_classes: BTreeMap<ENode<F, C>, ClassId<ENode<F, C>>>,
}

impl<F: fmt::Debug, C> fmt::Debug for EGraph<F, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            eq_uf,
            congr_uf,
            class_data,
            node_data,
            node_congr_classes,
        } = self;
        f.debug_struct("EGraph")
            .field("eq_uf", eq_uf)
            .field("congr_uf", congr_uf)
            .field("class_data", class_data)
            .field("node_data", node_data)
            .field("node_congr_classes", node_congr_classes)
            .finish()
    }
}

impl<F: Ord, C> Clone for EGraph<F, C> {
    fn clone(&self) -> Self {
        Self {
            eq_uf: self.eq_uf.clone(),
            congr_uf: self.congr_uf.clone(),
            class_data: self.class_data.clone(),
            node_data: self.node_data.clone(),
            node_congr_classes: self.node_congr_classes.clone(),
        }
    }
}

impl<F, C> Default for EGraph<F, C> {
    #[inline]
    fn default() -> Self { Self::new() }
}

impl<F, C> EGraph<F, C> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            eq_uf: UnionFind::new(),
            congr_uf: UnionFind::new(),
            class_data: BTreeMap::new(),
            node_data: BTreeMap::new(),
            node_congr_classes: BTreeMap::new(),
        }
    }
}

impl<F: Ord, C> From<EGraph<F, C>> for EGraphParts<F, C> {
    fn from(eg: EGraph<F, C>) -> Self {
        let EGraph {
            eq_uf,
            congr_uf: _,
            class_data: _,
            mut node_data,
            node_congr_classes,
        } = eg;

        assert_eq!(node_congr_classes.len(), node_data.len());

        let node_classes = node_congr_classes
            .into_iter()
            .map(|(k, v)| (k, eq_uf.find(node_data.remove(&v).unwrap().class).unwrap()))
            .collect();

        EGraphParts {
            uf: eq_uf,
            node_classes,
        }
    }
}

impl<F: Ord, C> EGraphCore for EGraph<F, C> {
    type Class = C;
    type FuncSymbol = F;

    fn add(&mut self, mut node: ENode<F, C>) -> Result<ClassId<C>, NoNode> {
        node.canonicalize_classes(&self.eq_uf)?;

        if let Some(c) = self.node_congr_classes.get(&node) {
            return Ok(self.node_data.get(c).unwrap().class);
        }

        let eq_class = self.eq_uf.add();
        let congr_class = self.congr_uf.add();

        assert!(self
            .node_data
            .insert(congr_class, NodeData::new(node.clone(), eq_class))
            .is_none());
        assert!(self
            .class_data
            .insert(eq_class, EClassData::new())
            .is_none());

        for &arg in node.args() {
            assert!(self
                .class_data
                .get_mut(&arg)
                .unwrap()
                .parents
                .insert(congr_class, eq_class)
                .is_none_or(|c| eq_class == c));
        }

        assert!(self.node_congr_classes.insert(node, congr_class).is_none());

        Ok(eq_class)
    }
}

impl<F: Ord, C> EGraphRead for EGraph<F, C> {
    #[inline]
    fn find(&self, class: ClassId<C>) -> Result<ClassId<C>, NoNode> { self.eq_uf.find(class) }

    #[inline]
    fn canonicalize(
        &self,
        node: &mut ENode<Self::FuncSymbol, Self::Class>,
    ) -> Result<bool, NoNode> {
        node.canonicalize_classes(&self.eq_uf)
    }

    #[inline]
    fn is_canonical(&self, node: &ENode<Self::FuncSymbol, Self::Class>) -> Result<bool, NoNode> {
        node.classes_canonical(&self.eq_uf)
    }

    #[must_use]
    fn class_nodes(&self) -> ClassNodes<Self> {
        self.node_data.values().fold(BTreeMap::new(), |mut m, d| {
            assert!(m
                .entry(self.eq_uf.find(d.class).unwrap())
                .or_default()
                .insert(&d.node));
            m
        })
    }

    #[inline]
    #[must_use]
    fn dot<M: trace::dot::Formatter<F>>(&self, f: M) -> dot::Graph<'static> {
        trace::dot_graph(f, self.class_nodes())
    }
}

impl<F: Ord, C> EGraphWrite for EGraph<F, C> {
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
    fn trace<T: EGraphTrace<F, C>, G: FnOnce() -> I, I: IntoIterator<Item = ClassId<C>>>(
        &self,
        t: &mut T,
        current: G,
    ) {
        t.graph(|g| {
            let nodes = trace::snapshot_graph(
                g,
                self.node_data.values().fold(
                    BTreeMap::new(),
                    |mut m: BTreeMap<ClassId<C>, BTreeSet<_>>, d| {
                        assert!(m
                            .entry(self.eq_uf.find(d.class).unwrap())
                            .or_default()
                            .insert(&d.node));
                        m
                    },
                ),
            );

            let mut seen = HashSet::new();
            for class in current() {
                let data = self.class_data.get(&class).unwrap();
                for (&parent, &par_id) in &data.parents {
                    let parent = self.congr_uf.find(parent).unwrap();
                    let par_id = self.eq_uf.find(par_id).unwrap();

                    if !seen.insert((class, parent, par_id)) {
                        continue;
                    }

                    g.parent_edge(
                        nodes.class_reps.get(&class).unwrap(),
                        nodes
                            .node_ids
                            .get(&self.node_data.get(&parent).unwrap().node)
                            .unwrap(),
                        Some(nodes.class_reps.get(&par_id).unwrap()),
                        Some(&format!("{}-{}", class.id(), parent.id())),
                    );
                }
            }

            // let mut uf = g.union_find("congruence");
            // let mut congr_ids = BTreeMap::new();

            // for id in self.congr_uf.classes() {
            //     let class = uf.class(format_args!("{}", id.id()));
            //     congr_ids.insert(id, class.id().clone());
            // }

            // for class in self.congr_uf.classes() {
            //     uf.parent(
            //         congr_ids.get(&class).unwrap(),
            //         congr_ids
            //             .get(&self.congr_uf.parent(class).unwrap())
            //             .unwrap(),
            //     );
            // }

            // for (node, class) in &self.node_congr_classes {
            //     uf.link_to_node(
            //         congr_ids.get(class).unwrap(),
            //         nodes.node_ids.get(&node).unwrap(),
            //     );
            // }

            // for class in current() {
            //     let data = self.class_data.get(&class).unwrap();
            //     for (parent, par_id) in &data.parents {
            //         uf.link_from_graph_class(
            //             nodes.class_reps.get(&class).unwrap(),
            //             congr_ids.get(parent).unwrap(),
            //         );
            //     }
            // }
        });
    }

    fn merge_impl<T: EGraphTrace<F, C>>(
        &mut self,
        a: ClassId<C>,
        b: ClassId<C>,
        t: &mut T,
    ) -> Result<Unioned<C>, NoNode> {
        use std::collections::btree_map::Entry;

        self.trace(t, || {
            [self.eq_uf.find(a).unwrap(), self.eq_uf.find(b).unwrap()]
        });

        let union = self.eq_uf.union(a, b)?;
        let Unioned { root, unioned } = union;
        t.hl_class(root);

        if let Some(unioned) = unioned {
            t.hl_class(unioned);

            let EClassData { parents } = self.class_data.remove(&unioned).unwrap();
            let root_data = self.class_data.get_mut(&root).unwrap();

            let mut new_parents = BTreeMap::new();
            let mut to_merge = vec![];
            for (old_congr_class, old_eq_class) in
                parents.into_iter().chain(mem::take(&mut root_data.parents))
            {
                let old_congr_class = self.congr_uf.find(old_congr_class).unwrap();
                let Entry::Occupied(mut old_par) = self.node_data.entry(old_congr_class) else {
                    panic!("Missing node_data entry for old node parent");
                };

                let mut new_par = old_par.get().node.clone();
                let was_not_canon = new_par.canonicalize_classes(&self.eq_uf).unwrap();

                let new_par_data;
                let new_congr_class;
                if was_not_canon {
                    let old_par = old_par.remove();

                    new_congr_class =
                        if let Some(other_congr_class) = self.node_congr_classes.remove(&new_par) {
                            assert!(self
                                .node_data
                                .remove(&self.congr_uf.find(other_congr_class).unwrap())
                                .is_some());

                            let Unioned { root, unioned } = self
                                .congr_uf
                                .union(other_congr_class, old_congr_class)
                                .unwrap();

                            if let Some(unioned) = unioned {
                                if let Some(class) = new_parents.remove(&unioned) {
                                    new_parents.insert(root, class);
                                }
                            }

                            root
                        } else {
                            old_congr_class
                        };

                    assert!(self.node_congr_classes.remove(&old_par.node).is_some());
                    assert!(self
                        .node_congr_classes
                        .insert(new_par.clone(), new_congr_class)
                        .is_none());

                    let Entry::Vacant(v) = self.node_data.entry(new_congr_class) else {
                        panic!("Entry exists in node_data for new congruence class");
                    };

                    new_par_data = v.insert(NodeData::new(new_par.clone(), old_eq_class));
                } else {
                    new_congr_class = old_congr_class;
                    new_par_data = old_par.get_mut();
                }

                let new_eq_class = self.eq_uf.find(old_eq_class).unwrap();

                match new_parents.entry(new_congr_class) {
                    Entry::Vacant(v) => {
                        v.insert(new_eq_class);
                        new_par_data.class = new_eq_class;
                    },
                    Entry::Occupied(mut o) => {
                        to_merge.push((o.insert(new_eq_class), new_eq_class));
                        assert!(self.node_data.contains_key(&new_congr_class));
                    },
                }
            }

            assert!(mem::replace(&mut root_data.parents, new_parents).is_empty());

            self.trace(t, || [root]);
            t.hl_class(root);
            t.hl_merges(to_merge.iter().copied());

            self.assert_invariants(false);

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
        assert_eq!(
            self.node_data.len(),
            self.node_congr_classes.len(),
            "Length mismatch between node_data vs. node_congr_classes"
        );

        for (node, &congr_class) in &self.node_congr_classes {
            assert!(*node == self.node_data.get(&congr_class).unwrap().node);
            assert!(node.classes_canonical(&self.eq_uf).unwrap());

            let root = self.congr_uf.find(congr_class).unwrap();
            for arg in node.args() {
                assert!(self
                    .class_data
                    .get(arg)
                    .unwrap()
                    .parents
                    .keys()
                    .any(|&c| self.congr_uf.find(c).unwrap() == root));
            }
        }

        for &congr_class in self.node_data.keys() {
            assert_eq!(
                congr_class,
                self.congr_uf.find(congr_class).unwrap(),
                "Congruence class was not canonical"
            );
        }

        for congr_root in self.congr_uf.roots() {
            assert!(self.node_data.contains_key(&congr_root));
        }

        for eq_root in self.eq_uf.roots() {
            assert!(self.class_data.contains_key(&eq_root));
        }

        for (&eq_class, EClassData { parents }) in &self.class_data {
            assert_eq!(
                eq_class,
                self.eq_uf.find(eq_class).unwrap(),
                "Equivalence class was not canonical"
            );

            if merged {
                let mut seen = BTreeMap::new();
                for (&congr_class, &eq_class) in parents {
                    let congr_class = self.congr_uf.find(congr_class).unwrap();
                    let eq_class = self.eq_uf.find(eq_class).unwrap();

                    assert_eq!(
                        eq_class,
                        self.eq_uf
                            .find(self.node_data.get(&congr_class).unwrap().class)
                            .unwrap(),
                        "Node parent class is different than registered node class"
                    );

                    if let Some(&other_eq_class) = seen.get(&congr_class) {
                        assert_eq!(
                            other_eq_class, eq_class,
                            "Class referents canonicalize to multiple duplicate classes"
                        );
                    } else {
                        seen.insert(congr_class, eq_class);
                    }
                }
            }
        }
    }
}
