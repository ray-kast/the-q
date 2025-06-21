use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, mem,
};

use hashbrown::HashSet;

use super::{
    prelude::*,
    test_tools::EGraphParts,
    trace::{self, SnapshotEGraph, SnapshotEquivClass},
    ClassNodes, EGraphTrace, ENode,
};
use crate::{
    dot,
    union_find::{ClassId, NoNode, UnionFind, Unioned},
};

// TODO: tests to add:
//       - congruence invariant
//       - hashcons invariant
//       - assert class_data.nodes is correct
//       - assert node_classes isn't leaking
//       - assert only roots have EClassData
//       - assert all parents are stored correctly
//       - assert no empty e-classes

// TODO: fixup usages of unwrap()

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
    poison: bool,
}

impl<F: fmt::Debug, C> fmt::Debug for EGraph<F, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            eq_uf,
            congr_uf,
            class_data,
            node_data,
            node_congr_classes,
            poison,
        } = self;
        f.debug_struct("EGraph")
            .field("eq_uf", eq_uf)
            .field("congr_uf", congr_uf)
            .field("class_data", class_data)
            .field("node_data", node_data)
            .field("node_congr_classes", node_congr_classes)
            .field("poison", poison)
            .finish()
    }
}

impl<F, C> Clone for EGraph<F, C> {
    fn clone(&self) -> Self {
        Self {
            eq_uf: self.eq_uf.clone(),
            congr_uf: self.congr_uf.clone(),
            class_data: self.class_data.clone(),
            node_data: self.node_data.clone(),
            node_congr_classes: self.node_congr_classes.clone(),
            poison: self.poison,
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
            poison: false,
        }
    }

    #[inline]
    fn poison_check(&self) {
        assert!(!self.poison, "e-graph was poisoned!");
    }
}

impl<F: Ord, C> From<EGraph<F, C>> for EGraphParts<F, C> {
    fn from(eg: EGraph<F, C>) -> Self {
        eg.poison_check();

        let EGraph {
            eq_uf,
            congr_uf: _,
            class_data: _,
            mut node_data,
            node_congr_classes,
            poison: _,
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
    fn find(&self, class: ClassId<C>) -> Result<ClassId<C>, NoNode> {
        self.poison_check();
        self.eq_uf.find(class)
    }

    #[inline]
    fn canonicalize(&self, node: &mut ENode<F, C>) -> Result<bool, NoNode> {
        self.poison_check();
        node.canonicalize_classes(&self.eq_uf)
    }

    #[inline]
    fn is_canonical(&self, node: &ENode<F, C>) -> Result<bool, NoNode> {
        self.poison_check();
        node.classes_canonical(&self.eq_uf)
    }

    fn class_nodes(&self) -> ClassNodes<Self> {
        self.poison_check();

        self.node_data.values().fold(BTreeMap::new(), |mut m, d| {
            assert!(m
                .entry(self.eq_uf.find(d.class).unwrap())
                .or_default()
                .insert(&d.node));
            m
        })
    }

    #[inline]
    fn dot<M: trace::dot::Formatter<F>>(&self, f: M) -> dot::Graph<'static> {
        self.poison_check();
        trace::dot_graph(f, self.class_nodes())
    }
}

impl<F: Ord, C> EGraphUpgradeTrace for EGraph<F, C> {
    type WriteRef<'a, T: EGraphTrace<F, C>>
        = EGraphMut<'a, F, C, T>
    where Self: 'a;

    fn write_trace<T: EGraphTrace<F, C>>(&mut self, tracer: T) -> Self::WriteRef<'_, T> {
        self.poison_check();
        self.poison = true;

        EGraphMut {
            eg: self,
            dirty: BTreeMap::new(),
            tracer,
        }
    }
}

type DirtySet<C> = BTreeMap<ClassId<C>, BTreeSet<ClassId<C>>>;

pub struct EGraphMut<'a, F: Ord, C, T: EGraphTrace<F, C>> {
    eg: &'a mut EGraph<F, C>,
    dirty: DirtySet<C>,
    tracer: T,
}

impl<F: fmt::Debug + Ord, C, T: fmt::Debug + EGraphTrace<F, C>> fmt::Debug
    for EGraphMut<'_, F, C, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { eg, dirty, tracer } = self;
        f.debug_struct("EGraphMut")
            .field("eg", eg)
            .field("dirty", dirty)
            .field("tracer", tracer)
            .finish()
    }
}

impl<F: Ord, C, T: EGraphTrace<F, C>> Drop for EGraphMut<'_, F, C, T> {
    fn drop(&mut self) {
        self.rebuild();
        self.eg.poison = false;
    }
}

trait ExpectInvariant<T> {
    fn expect_invariant(self, msg: &str) -> T;

    fn expect_none_invariant(self, msg: &str);
}

#[cfg(any(test, feature = "test"))]
impl<T> ExpectInvariant<T> for Option<T> {
    #[inline]
    fn expect_invariant(self, msg: &str) -> T { self.expect(msg) }

    #[inline]
    fn expect_none_invariant(self, msg: &str) {
        assert!(self.is_none(), "{msg}");
    }
}

#[cfg(any(test, feature = "test"))]
impl<T, E> ExpectInvariant<T> for Result<T, E> {
    #[inline]
    fn expect_invariant(self, msg: &str) -> T { self.unwrap_or_else(|_| panic!("{msg}")) }

    #[inline]
    fn expect_none_invariant(self, msg: &str) {
        assert!(self.is_err(), "{msg}");
    }
}

#[cfg(not(any(test, feature = "test")))]
impl<T> ExpectInvariant<T> for Option<T> {
    #[inline]
    fn expect_invariant(self, _: &str) -> T { self.unwrap_or_else(|| unreachable!()) }

    #[inline]
    fn expect_none_invariant(self, _: &str) {
        if self.is_some() {
            unreachable!();
        }
    }
}

#[cfg(not(any(test, feature = "test")))]
impl<T, E> ExpectInvariant<T> for Result<T, E> {
    #[inline]
    fn expect_invariant(self, _: &str) -> T { self.unwrap_or_else(|_| unreachable!()) }

    #[inline]
    fn expect_none_invariant(self, _: &str) {
        if self.is_ok() {
            unreachable!();
        }
    }
}

macro_rules! invariant {
    ($($tt:tt)*) => {
        #[cfg(any(test, feature = "test"))]
        { panic!($($tt)*) }

        #[cfg(not(any(test, feature = "test")))]
        { unreachable!() }
    };
}

impl<F: Ord, C, T: EGraphTrace<F, C>> EGraphCore for EGraphMut<'_, F, C, T> {
    type Class = C;
    type FuncSymbol = F;

    fn add(&mut self, node: ENode<F, C>) -> Result<ClassId<C>, NoNode> { self.eg.add(node) }
}

impl<F: Ord, C, T: EGraphTrace<F, C>> EGraphWrite for EGraphMut<'_, F, C, T> {
    fn merge(&mut self, a: ClassId<C>, b: ClassId<C>) -> Result<Unioned<C>, NoNode> {
        let union = self.eg.eq_uf.union(a, b)?;

        if let Some(other) = union.unioned {
            self.dirty.entry(union.root).or_default().insert(other);
        }

        Ok(union)
    }
}

impl<F: Ord, C, T: EGraphTrace<F, C>> EGraphMut<'_, F, C, T> {
    fn trace<G: FnOnce() -> I, I: IntoIterator<Item = ClassId<C>>>(&mut self, current: G) {
        self.tracer.graph(|g| {
            let mut nodes = trace::snapshot_graph(
                g,
                self.eg.node_data.values().fold(
                    BTreeMap::new(),
                    |mut m: BTreeMap<ClassId<C>, BTreeSet<_>>, d| {
                        assert!(m
                            .entry(self.eg.eq_uf.find(d.class).unwrap())
                            .or_default()
                            .insert(&d.node));
                        m
                    },
                ),
            );

            let mut seen = HashSet::new();
            for class in current() {
                let data = self.eg.class_data.get(&class).unwrap();
                for (&parent, &par_id) in &data.parents {
                    let parent = self.eg.congr_uf.find(parent).unwrap();
                    let par_id = self.eg.eq_uf.find(par_id).unwrap();

                    if !seen.insert((class, parent, par_id)) {
                        continue;
                    }

                    for class in [class, par_id] {
                        nodes
                            .class_reps
                            .entry(class)
                            .or_insert_with(|| g.equiv_class(class).id().clone());
                    }

                    g.parent_edge(
                        nodes.class_reps.get(&class).unwrap(),
                        nodes
                            .node_ids
                            .get(&self.eg.node_data.get(&parent).unwrap().node)
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

    fn rebuild(&mut self) {
        let mut q = DirtySet::new();
        while !self.dirty.is_empty() {
            #[cfg(any(test, feature = "test"))]
            {
                assert!(q.is_empty());
            }

            for (root, unioned) in mem::take(&mut self.dirty) {
                let root = self
                    .eg
                    .eq_uf
                    .find(root)
                    .expect_invariant("Invalid root repair class");
                q.entry(root).or_default().extend(unioned);
            }

            for (c, o) in mem::take(&mut q) {
                self.repair(c, o);
                self.tracer.hl_merges(
                    q.iter()
                        .chain(&self.dirty)
                        .flat_map(|(&l, r)| r.iter().map(move |&r| (l, r))),
                );
            }
        }

        self.assert_invariants();
    }

    #[allow(clippy::too_many_lines, reason = "its just that complicated")]
    fn repair(&mut self, root: ClassId<C>, unioned: BTreeSet<ClassId<C>>) {
        use std::collections::btree_map::Entry;

        self.trace(|| [root].into_iter().chain(unioned.iter().copied()));
        self.tracer.hl_class(root);
        self.tracer.hl_classes(unioned.iter().copied());

        let merged_data: Vec<_> = unioned
            .into_iter()
            .map(|c| {
                self.eg
                    .class_data
                    .remove(&c)
                    .expect_invariant("Missing data for merged repair class")
            })
            .collect();

        let root_data = self
            .eg
            .class_data
            .get_mut(&root)
            .expect_invariant("Missing data for root repair class");

        let mut new_parents = BTreeMap::new();
        let mut to_merge = vec![];
        for (old_congr_class, old_eq_class) in merged_data
            .into_iter()
            .flat_map(|EClassData { parents }| parents)
            .chain(mem::take(&mut root_data.parents))
        {
            let old_congr_class = self
                .eg
                .congr_uf
                .find(old_congr_class)
                .expect_invariant("Invalid parent ID in repair class");

            let Entry::Occupied(mut old_par) = self.eg.node_data.entry(old_congr_class) else {
                invariant!("Missing node data for repair class parent");
            };

            let mut new_par = old_par.get().node.clone();
            let was_not_canon = new_par
                .canonicalize_classes(&self.eg.eq_uf)
                .expect_invariant("Invalid resolved parent in repair class");

            let new_congr_class;
            let new_par_data;
            if was_not_canon {
                let old_par = old_par.remove();

                new_congr_class =
                    if let Some(other_congr_class) = self.eg.node_congr_classes.remove(&new_par) {
                        self.eg
                            .node_data
                            .remove(
                                &self
                                    .eg
                                    .congr_uf
                                    .find(other_congr_class)
                                    .expect_invariant("Invalid existing ID for updated parent"),
                            )
                            .expect_invariant("Missing data for updated parent");

                        let Unioned { root, unioned } = self
                            .eg
                            .congr_uf
                            .union(other_congr_class, old_congr_class)
                            .expect_invariant("Unable to union updated parent IDs");

                        if let Some(unioned) = unioned {
                            if let Some(class) = new_parents.remove(&unioned) {
                                new_parents.insert(root, class);
                            }
                        }

                        root
                    } else {
                        old_congr_class
                    };

                self.eg
                    .node_congr_classes
                    .remove(&old_par.node)
                    .expect_invariant("Invalid previous parent for update");
                self.eg
                    .node_congr_classes
                    .insert(new_par.clone(), new_congr_class)
                    .expect_none_invariant("Updated parent collided when storing ID");

                let Entry::Vacant(v) = self.eg.node_data.entry(new_congr_class) else {
                    invariant!("Missing node data for updated parent");
                };

                new_par_data = v.insert(NodeData::new(new_par.clone(), old_eq_class));
            } else {
                new_congr_class = old_congr_class;
                new_par_data = old_par.get_mut();
            }

            let new_eq_class = self
                .eg
                .eq_uf
                .find(old_eq_class)
                .expect_invariant("Invalid equivalence class for repair");

            match new_parents.entry(new_congr_class) {
                Entry::Vacant(v) => {
                    v.insert(new_eq_class);
                    new_par_data.class = new_eq_class;
                },
                Entry::Occupied(mut o) => {
                    to_merge.push((o.insert(new_eq_class), new_eq_class));
                },
            }
        }

        if !mem::replace(&mut root_data.parents, new_parents).is_empty() {
            invariant!("Root parent list was not fully drained");
        }

        self.trace(|| [root]);
        self.tracer.hl_class(root);

        for (a, b) in to_merge {
            self.merge(a, b)
                .expect_invariant("Unable to perform upward merge");
        }
    }

    #[cfg(not(any(test, feature = "test")))]
    #[inline]
    fn assert_invariants(&self) { let _ = self; }

    #[cfg(any(test, feature = "test"))]
    fn assert_invariants(&self) {
        assert_eq!(
            self.eg.node_data.len(),
            self.eg.node_congr_classes.len(),
            "Length mismatch between node_data vs. node_congr_classes"
        );

        for (node, &congr_class) in &self.eg.node_congr_classes {
            assert!(*node == self.eg.node_data.get(&congr_class).unwrap().node);
            assert!(node.classes_canonical(&self.eg.eq_uf).unwrap());

            let root = self.eg.congr_uf.find(congr_class).unwrap();
            for arg in node.args() {
                assert!(self
                    .eg
                    .class_data
                    .get(arg)
                    .unwrap()
                    .parents
                    .keys()
                    .any(|&c| self.eg.congr_uf.find(c).unwrap() == root));
            }
        }

        for &congr_class in self.eg.node_data.keys() {
            assert_eq!(
                congr_class,
                self.eg.congr_uf.find(congr_class).unwrap(),
                "Congruence class was not canonical"
            );
        }

        for congr_root in self.eg.congr_uf.roots() {
            assert!(self.eg.node_data.contains_key(&congr_root));
        }

        for eq_root in self.eg.eq_uf.roots() {
            assert!(self.eg.class_data.contains_key(&eq_root));
        }

        for (&eq_class, EClassData { parents }) in &self.eg.class_data {
            assert_eq!(
                eq_class,
                self.eg.eq_uf.find(eq_class).unwrap(),
                "Equivalence class was not canonical"
            );

            let mut seen = BTreeMap::new();
            for (&congr_class, &eq_class) in parents {
                let congr_class = self.eg.congr_uf.find(congr_class).unwrap();
                let eq_class = self.eg.eq_uf.find(eq_class).unwrap();

                assert_eq!(
                    eq_class,
                    self.eg
                        .eq_uf
                        .find(self.eg.node_data.get(&congr_class).unwrap().class)
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
