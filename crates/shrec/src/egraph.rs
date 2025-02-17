use std::{
    borrow::Cow,
    fmt,
    hash::{Hash, Hasher},
    rc::Rc,
};

use hashbrown::{HashMap, HashSet};

use crate::{
    dot,
    union_find::{ClassId, NoNode, Union, UnionFind},
};

// TODO: a lot of this could be cleaned up if they introduced a solution for
//       better derive bounds

// TODO: tests to add:
//       - congruence invariant
//       - hashcons invariant
//       - assert class_data.nodes is correct
//       - assert node_classes isn't leaking
//       - assert only roots have EClassData
//       - assert all parents are stored correctly
//       - assert no empty e-classes

// TODO: this uses HashMap and HashSet.  verify that behavior is deterministic
// TODO: fixup usages of unwrap()

// TODO: probably memoize this rather than use Rc
pub struct ENode<F, C>(Rc<F>, Rc<[ClassId<C>]>);

impl<F, C> ENode<F, C> {
    #[must_use]
    pub fn op(&self) -> &F { &self.0 }

    #[must_use]
    pub fn args(&self) -> &[ClassId<C>] { &self.1 }

    fn canonicalize_impl(&mut self, uf: &UnionFind<C>) -> Result<(), NoNode> {
        for arg in Rc::make_mut(&mut self.1) {
            *arg = uf.find(*arg)?;
        }

        Ok(())
    }

    pub fn canonicalize(&mut self, eg: &EGraph<F, C>) -> Result<(), NoNode> {
        eg.poison_check();
        self.canonicalize_impl(&eg.uf)
    }
}

impl<F: fmt::Debug, C> fmt::Debug for ENode<F, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self(op, args) = self;
        f.debug_tuple("ENode").field(&op).field(&args).finish()
    }
}

impl<F, C> Clone for ENode<F, C> {
    fn clone(&self) -> Self { Self(Rc::clone(&self.0), Rc::clone(&self.1)) }
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
    pub const fn new(op: Rc<F>, args: Rc<[ClassId<C>]>) -> Self { Self(op, args) }
}

struct EClassData<F, C> {
    parents: HashMap<ENode<F, C>, ClassId<C>>,
    // TODO: was it actually necessary to add this
    nodes: HashSet<ENode<F, C>>,
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

impl<F: Eq + Hash, C> EClassData<F, C> {
    fn new(node: ENode<F, C>) -> Self {
        Self {
            parents: HashMap::new(),
            nodes: [node].into_iter().collect(),
        }
    }

    fn merge(&mut self, rhs: EClassData<F, C>) {
        let EClassData { parents, nodes } = rhs;

        for (node, klass) in parents {
            assert_eq!(klass, *self.parents.entry(node).or_insert(klass));
        }

        self.nodes.extend(nodes);
    }

    // TODO: is this the most efficient way to repair the class map?
    fn canonicalize_impl(&mut self, uf: &UnionFind<C>, buf: &mut Vec<ENode<F, C>>) {
        debug_assert!(buf.is_empty());
        buf.extend(self.nodes.drain().map(|mut n| {
            safe_nodes(n.canonicalize_impl(uf));
            n
        }));
        self.nodes.extend(buf.drain(..));
    }
}

type DataMap<F, C> = HashMap<ClassId<C>, EClassData<F, C>>;
type HashCons<F, C> = HashMap<ENode<F, C>, ClassId<C>>;

pub struct EGraph<F, C> {
    uf: UnionFind<C>,
    class_data: DataMap<F, C>,
    node_classes: HashCons<F, C>,
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
            class_data: HashMap::new(),
            node_classes: HashMap::new(),
            poison: false,
        }
    }

    #[inline]
    fn poison_check(&self) {
        assert!(!self.poison, "e-graph was poisoned!");
    }

    pub fn find(&self, klass: ClassId<C>) -> Result<ClassId<C>, NoNode> {
        self.poison_check();
        self.uf.find(klass)
    }
}

impl<F: Eq + Hash, C> EGraph<F, C> {
    #[must_use]
    pub fn class_nodes(&self) -> HashMap<ClassId<C>, HashSet<ENode<F, C>>> {
        self.poison_check();

        // TODO: should the value be cloned or borrowed?
        let ret: HashMap<_, _> = self
            .class_data
            .iter()
            .map(|(&k, v)| (k, v.nodes.clone()))
            .collect();

        #[cfg(debug_assertions)]
        {
            let constructed = self.node_classes.iter().fold(
                HashMap::new(),
                |mut m: HashMap<_, HashSet<_>>, (n, &c)| {
                    assert!(m
                        .entry(self.uf.find(c).unwrap())
                        .or_default()
                        .insert(n.clone()));
                    m
                },
            );
            assert!(constructed == ret);
        }

        ret
    }

    pub fn get_nodes(&self, klass: ClassId<C>) -> Result<Option<&HashSet<ENode<F, C>>>, NoNode> {
        self.poison_check();
        self.uf
            .find(klass)
            .map(|c| self.class_data.get(&c).map(|d| &d.nodes))
    }

    pub fn get_class(&self, node: &mut ENode<F, C>) -> Result<Option<ClassId<C>>, NoNode> {
        self.poison_check();
        node.canonicalize_impl(&self.uf)
            .map(|()| self.node_classes.get(node).copied())
    }

    pub fn write(&mut self) -> EGraphMut<'_, F, C> {
        self.poison_check();
        self.poison = true;

        EGraphMut {
            eg: self,
            dirty: HashMap::new(),
        }
    }

    pub fn add(&mut self, mut node: ENode<F, C>) -> Result<ClassId<C>, NoNode> {
        node.canonicalize_impl(&self.uf)?;
        Ok(if let Some(&klass) = self.node_classes.get(&node) {
            klass
        } else {
            let klass = self.uf.add();
            assert!(self
                .class_data
                .insert(klass, EClassData::new(node.clone()))
                .is_none());

            for &arg in &*node.1 {
                // Rationale: the inserted class is a new singleton, thus any
                //            existing instances of it are already canonical
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

    #[must_use]
    pub fn dot<'a>(
        &self,
        fmt_op: impl Fn(&F, ClassId<C>) -> Cow<'a, str>,
        fmt_edge: impl Fn(&F, usize) -> Option<Cow<'a, str>>,
    ) -> dot::Graph<'a> {
        self.poison_check();

        let mut graph = dot::Graph::new(dot::GraphType::Directed, None);
        let mut class_reps = HashMap::new();
        let mut node_ids = HashMap::new();

        for root in self.uf.roots() {
            let sg = graph.subgraph(format!("cluster_{}", root.id()).into());
            sg.style("filled".into());

            let rep_id = Cow::from(format!("class_{}", root.id()));
            let class_node = sg.node(rep_id.clone());
            class_reps.insert(root, rep_id.clone());
            class_node.style("invis".into());
            class_node.shape("point".into());
            class_node.label("".into());

            if let Some(data) = self.class_data.get(&root) {
                for node in &data.nodes {
                    let mut label = format!("{}(", fmt_op(&node.0, root));
                    for (i, arg) in node.1.iter().enumerate() {
                        if i > 0 {
                            label.push(',');
                        }

                        label.push_str(&arg.id().to_string());
                    }
                    label.push(')');
                    let id = Cow::from(format!("node_{label}"));
                    node_ids.entry(node).or_insert_with(|| id.clone());

                    let node = sg.node(id.clone());
                    node.label(label.into());
                    let edge = sg.edge(rep_id.clone(), id.clone());
                    edge.style("invis".into());
                }
            }
        }

        for root in self.uf.roots() {
            if let Some(data) = self.class_data.get(&root) {
                for node in &data.nodes {
                    for (i, edge) in node.1.iter().enumerate() {
                        let edge = graph.edge(
                            node_ids[node].clone(),
                            class_reps[&self.uf.find(*edge).unwrap()].clone(),
                        );

                        if let Some(label) = fmt_edge(&node.0, i) {
                            edge.label(label);
                        }
                    }
                }
            }
        }

        graph
    }
}

type DirtySet<C> = HashMap<ClassId<C>, HashSet<ClassId<C>>>;

pub struct EGraphMut<'a, F: Eq + Hash, C> {
    eg: &'a mut EGraph<F, C>,
    dirty: DirtySet<C>,
}

impl<F: fmt::Debug + Eq + Hash, C> fmt::Debug for EGraphMut<'_, F, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { eg, dirty } = self;
        f.debug_struct("EGraphMut")
            .field("eg", eg)
            .field("dirty", dirty)
            .finish()
    }
}

impl<F: Eq + Hash, C> Drop for EGraphMut<'_, F, C> {
    fn drop(&mut self) {
        self.rebuild();
        self.eg.poison = false;
    }
}

#[inline]
fn safe_nodes<T>(res: Result<T, NoNode>) -> T { res.unwrap_or_else(|_| unreachable!()) }

#[inline]
fn safe_nodes_opt<T>(opt: Option<T>) -> T { opt.unwrap_or_else(|| unreachable!()) }

impl<F: Eq + Hash, C> EGraphMut<'_, F, C> {
    pub fn add(&mut self, node: ENode<F, C>) -> Result<ClassId<C>, NoNode> { self.eg.add(node) }
}

impl<F: Eq + Hash, C> EGraphMut<'_, F, C> {
    pub fn merge(&mut self, a: ClassId<C>, b: ClassId<C>) -> Result<Union<C>, NoNode> {
        let union = self.eg.uf.union(a, b)?;

        if let Some(other) = union.unioned {
            self.dirty.entry(union.root).or_default().insert(other);
        }

        Ok(union)
    }

    fn rebuild(&mut self) {
        let mut q = DirtySet::new();
        // TODO: tracking rewrites seems extremely hacky
        let mut rewrites = HashMap::new();
        while !self.dirty.is_empty() {
            debug_assert!(q.is_empty());
            for (root, others) in self.dirty.drain() {
                let root = safe_nodes(self.eg.uf.find(root));
                q.entry(root).or_default().extend(others);
            }

            q.drain()
                .for_each(|(c, o)| self.repair(c, o, &mut rewrites));
        }
    }

    fn repair(
        &mut self,
        repair_class: ClassId<C>,
        equiv_classes: HashSet<ClassId<C>>,
        rewrites: &mut HashMap<ENode<F, C>, ENode<F, C>>,
    ) {
        let merged = equiv_classes
            .into_iter()
            .map(|c| self.eg.class_data.remove(&c).unwrap())
            .reduce(|mut l, r| {
                l.merge(r);
                l
            });

        let mut data = safe_nodes_opt(self.eg.class_data.remove(&repair_class));
        if let Some(merged) = merged {
            data.merge(merged);
        }

        let mut new_parents = HashMap::new();
        let mut canon_buf = vec![];
        for (mut node, klass) in data.parents {
            use hashbrown::hash_map::Entry;

            self.eg.node_classes.remove(&node).unwrap_or_else(|| {
                self.eg
                    .node_classes
                    .remove(rewrites.get(&node).unwrap())
                    .unwrap()
            });
            let old_node = node.clone();
            safe_nodes(node.canonicalize_impl(&self.eg.uf));
            let root = safe_nodes(self.eg.uf.find(klass));

            // TODO: does node need to be re-canonicalized in new_parents
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

            safe_nodes(node.canonicalize_impl(&self.eg.uf));
            rewrites.insert(old_node, node.clone());
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
