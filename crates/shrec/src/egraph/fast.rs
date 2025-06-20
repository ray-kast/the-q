use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, mem,
};

use super::{prelude::*, test_tools::EGraphParts, trace, ClassNodes, EGraphTrace, ENode};
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
    parents: BTreeMap<ENode<F, C>, ClassId<C>>,
    // TODO: was it actually necessary to add this
    nodes: BTreeSet<ENode<F, C>>,
}

impl<F: fmt::Debug, C> fmt::Debug for EClassData<F, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { parents, nodes } = self;
        f.debug_struct("EClassData")
            .field("parents", parents)
            .field("nodes", nodes)
            .finish()
    }
}

impl<F, C> Clone for EClassData<F, C> {
    fn clone(&self) -> Self {
        Self {
            parents: self.parents.clone(),
            nodes: self.nodes.clone(),
        }
    }
}

impl<F: Ord, C> EClassData<F, C> {
    fn new(node: ENode<F, C>) -> Self {
        Self {
            parents: BTreeMap::new(),
            nodes: [node].into_iter().collect(),
        }
    }

    fn merge(&mut self, rhs: EClassData<F, C>, uf: &UnionFind<C>) {
        let EClassData { parents, nodes } = rhs;

        for (node, klass) in parents {
            assert_eq!(
                uf.find(klass).unwrap(),
                uf.find(*self.parents.entry(node).or_insert(klass)).unwrap()
            );
        }

        self.nodes = mem::take(&mut self.nodes)
            .into_iter()
            .chain(nodes)
            .map(|mut n| {
                n.canonicalize_classes(uf).unwrap();
                n
            })
            .collect();
    }

    // TODO: is this the most efficient way to repair the class map?
    fn canonicalize_impl(&mut self, uf: &UnionFind<C>, buf: &mut Vec<ENode<F, C>>) {
        debug_assert!(buf.is_empty());
        buf.extend(mem::take(&mut self.nodes).into_iter().map(|mut n| {
            safe_nodes(n.canonicalize_classes(uf));
            n
        }));
        self.nodes.extend(buf.drain(..));
    }
}

pub struct EGraph<F, C> {
    uf: UnionFind<C>,
    class_data: BTreeMap<ClassId<C>, EClassData<F, C>>,
    node_classes: BTreeMap<ENode<F, C>, ClassId<C>>,
    poison: bool,
}

impl<F: fmt::Debug, C> fmt::Debug for EGraph<F, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            uf,
            class_data,
            node_classes,
            poison,
        } = self;
        f.debug_struct("EGraph")
            .field("uf", uf)
            .field("class_data", class_data)
            .field("node_classes", node_classes)
            .field("poison", poison)
            .finish()
    }
}

impl<F, C> Clone for EGraph<F, C> {
    fn clone(&self) -> Self {
        Self {
            uf: self.uf.clone(),
            class_data: self.class_data.clone(),
            node_classes: self.node_classes.clone(),
            poison: self.poison,
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
            uf,
            class_data: _,
            node_classes,
            poison: _,
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
        Ok(if let Some(&klass) = self.node_classes.get(&node) {
            klass
        } else {
            let klass = self.uf.add();
            assert!(self
                .class_data
                .insert(klass, EClassData::new(node.clone()))
                .is_none());

            for &arg in node.args() {
                // Rationale for not canonicalizing c: the inserted class is a
                // new singleton, thus any existing instances of it are already
                // canonical
                assert!(self
                    .class_data
                    .get_mut(&arg)
                    .unwrap()
                    .parents
                    .insert(node.clone(), klass)
                    .is_none_or(|c| c == klass));
            }

            self.node_classes.insert(node, klass);
            klass
        })
    }
}

impl<F: Ord, C> EGraphRead for EGraph<F, C> {
    #[inline]
    fn find(&self, klass: ClassId<C>) -> Result<ClassId<C>, NoNode> {
        self.poison_check();
        self.uf.find(klass)
    }

    #[inline]
    fn canonicalize(&self, node: &mut ENode<F, C>) -> Result<bool, NoNode> {
        self.poison_check();
        node.canonicalize_classes(&self.uf)
    }

    #[inline]
    fn is_canonical(&self, node: &ENode<F, C>) -> Result<bool, NoNode> {
        self.poison_check();
        node.classes_canonical(&self.uf)
    }

    #[must_use]
    fn class_nodes(&self) -> ClassNodes<Self> {
        self.poison_check();

        self.class_data
            .iter()
            .map(|(&k, v)| {
                let nodes: BTreeSet<_> = v.nodes.iter().collect();
                assert!(nodes.len() == v.nodes.len());
                (k, nodes)
            })
            .collect()
    }

    #[inline]
    #[must_use]
    fn dot<M: trace::dot::Formatter<Self::FuncSymbol>>(&self, f: M) -> dot::Graph<'static> {
        self.poison_check();

        trace::dot_graph(f, self.uf.roots().map(|r| (r, &self.class_data[&r].nodes)))
    }
}

impl<F: Ord, C> EGraphUpgrade for EGraph<F, C> {
    type WriteRef<'a>
        = EGraphMut<'a, F, C>
    where Self: 'a;

    fn write(&mut self) -> Self::WriteRef<'_> {
        self.poison_check();
        self.poison = true;

        EGraphMut {
            eg: self,
            dirty: BTreeMap::new(),
            old_uf: UnionFind::new(),
        }
    }
}

impl<F: Ord, C> EGraph<F, C> {
    pub fn get_nodes(&self, klass: ClassId<C>) -> Result<Option<&BTreeSet<ENode<F, C>>>, NoNode> {
        self.poison_check();
        self.uf
            .find(klass)
            .map(|c| self.class_data.get(&c).map(|d| &d.nodes))
    }

    pub fn get_class(&self, node: &mut ENode<F, C>) -> Result<Option<ClassId<C>>, NoNode> {
        self.poison_check();
        node.canonicalize_classes(&self.uf)
            .map(|_: bool| self.node_classes.get(node).copied())
    }
}

type DirtySet<C> = BTreeMap<ClassId<C>, BTreeSet<ClassId<C>>>;

pub struct EGraphMut<'a, F: Ord, C> {
    eg: &'a mut EGraph<F, C>,
    dirty: DirtySet<C>,
    // TODO: it's not tracking rewrites, but it still feels hacky
    old_uf: UnionFind<C>,
}

impl<F: fmt::Debug + Ord, C> fmt::Debug for EGraphMut<'_, F, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { eg, dirty, old_uf } = self;
        f.debug_struct("EGraphMut")
            .field("eg", eg)
            .field("dirty", dirty)
            .field("old_uf", old_uf)
            .finish()
    }
}

impl<F: Ord, C> Drop for EGraphMut<'_, F, C> {
    fn drop(&mut self) {
        self.rebuild();
        self.eg.poison = false;
    }
}

#[inline]
fn safe_nodes<T>(res: Result<T, NoNode>) -> T { res.unwrap_or_else(|_| unreachable!()) }

#[inline]
fn safe_nodes_opt<T>(opt: Option<T>) -> T { opt.unwrap_or_else(|| unreachable!()) }

impl<F: Ord, C> EGraphCore for EGraphMut<'_, F, C> {
    type Class = C;
    type FuncSymbol = F;

    fn add(&mut self, node: ENode<F, C>) -> Result<ClassId<C>, NoNode> { self.eg.add(node) }
}

impl<F: Ord, C> EGraphWrite for EGraphMut<'_, F, C> {
    fn merge_trace<T: EGraphTrace<F, C>>(
        &mut self,
        a: ClassId<C>,
        b: ClassId<C>,
        _: &mut T,
    ) -> Result<Unioned<C>, NoNode> {
        // TODO: can we trace anything, merges are deferred
        if self.old_uf.is_empty() {
            self.old_uf.clone_from(&self.eg.uf);
        }

        let union = self.eg.uf.union(a, b)?;

        if let Some(other) = union.unioned {
            self.dirty.entry(union.root).or_default().insert(other);
        }

        Ok(union)
    }
}

impl<F: Ord, C> EGraphMut<'_, F, C> {
    fn rebuild(&mut self) {
        let mut q = DirtySet::new();
        loop {
            debug_assert!(q.is_empty());
            for (root, others) in mem::take(&mut self.dirty) {
                let root = safe_nodes(self.eg.uf.find(root));
                q.entry(root).or_default().extend(others);
            }

            mem::take(&mut q)
                .into_iter()
                .for_each(|(c, o)| self.repair(c, o));

            if self.dirty.is_empty() {
                break;
            }

            self.old_uf.clone_from(&self.eg.uf);
        }
    }

    fn repair(&mut self, repair_class: ClassId<C>, equiv_classes: BTreeSet<ClassId<C>>) {
        let merged = equiv_classes
            .into_iter()
            .map(|c| self.eg.class_data.remove(&c).unwrap())
            .reduce(|mut l, r| {
                l.merge(r, &self.eg.uf);
                l
            });

        let mut data = safe_nodes_opt(self.eg.class_data.remove(&repair_class));
        if let Some(merged) = merged {
            data.merge(merged, &self.eg.uf);
        }

        let mut new_parents = BTreeMap::new();
        let mut canon_buf = vec![];
        for (mut node, klass) in data.parents {
            use std::collections::btree_map::Entry;

            safe_nodes(node.canonicalize_classes(&self.old_uf));
            safe_nodes_opt(self.eg.node_classes.remove(&node));
            safe_nodes(node.canonicalize_classes(&self.eg.uf));
            let root = safe_nodes(self.eg.uf.find(klass));

            let root = match new_parents.entry(node.clone()) {
                Entry::Occupied(mut o) => {
                    let prev = o.insert(root);
                    let union = safe_nodes(self.merge(root, prev));

                    union.root
                },
                Entry::Vacant(v) => {
                    v.insert(root);
                    root
                },
            };

            debug_assert_eq!(root, safe_nodes(self.eg.uf.find(root)));

            safe_nodes(node.canonicalize_classes(&self.eg.uf));
            if root != repair_class {
                safe_nodes_opt(self.eg.class_data.get_mut(&root))
                    .canonicalize_impl(&self.eg.uf, &mut canon_buf);
            }
            self.eg.node_classes.insert(node.clone(), root);
        }

        data.parents = new_parents;
        data.canonicalize_impl(&self.eg.uf, &mut canon_buf);
        self.eg.class_data.insert(repair_class, data);
    }
}
