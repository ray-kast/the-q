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

    pub fn canonicalize(&mut self, eg: &EGraph<F, C>) -> Result<(), NoNode> {
        for arg in Rc::make_mut(&mut self.1) {
            *arg = eg.uf.find(*arg)?;
        }

        Ok(())
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
    // TODO: gather an intuition of why the ClassId is necessary here
    parents: HashMap<ENode<F, C>, ClassId<C>>,
}

impl<F: Eq + Hash, C> EClassData<F, C> {
    fn merge(&mut self, rhs: EClassData<F, C>) {
        let EClassData { parents } = rhs;

        for (node, klass) in parents {
            assert_eq!(klass, *self.parents.entry(node).or_insert(klass));
        }
    }
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

impl<F, C> Default for EClassData<F, C> {
    fn default() -> Self {
        Self {
            parents: HashMap::new(),
        }
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
        self.node_classes
            .iter()
            .fold(HashMap::new(), |mut m: HashMap<_, HashSet<_>>, (n, &c)| {
                assert!(m
                    .entry(self.uf.find(c).unwrap())
                    .or_default()
                    .insert(n.clone()));
                m
            })
    }

    pub fn get(&self, node: &mut ENode<F, C>) -> Result<Option<ClassId<C>>, NoNode> {
        node.canonicalize(self)
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
        node.canonicalize(self)?;
        Ok(if let Some(&klass) = self.node_classes.get(&node) {
            klass
        } else {
            let klass = self.uf.add();
            assert!(self
                .class_data
                .insert(klass, EClassData::default())
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
        fmt_op: impl Fn(&F) -> Cow<'a, str>,
        fmt_edge: impl Fn(&F, usize) -> Option<Cow<'a, str>>,
    ) -> dot::Graph<'a> {
        self.poison_check();

        let mut graph = dot::Graph::new(dot::GraphType::Directed, None);
        let class_nodes = self.class_nodes();

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

            if let Some(nodes) = class_nodes.get(&root) {
                for node in nodes {
                    let mut label = format!("{}(", fmt_op(&node.0));
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
            if let Some(nodes) = class_nodes.get(&root) {
                for node in nodes {
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
        klass: ClassId<C>,
        others: HashSet<ClassId<C>>,
        rewrites: &mut HashMap<ENode<F, C>, ENode<F, C>>,
    ) {
        let merged = others
            .into_iter()
            .map(|c| self.eg.class_data.remove(&c).unwrap())
            .reduce(|mut l, r| {
                l.merge(r);
                l
            });

        let mut data = safe_nodes_opt(self.eg.class_data.get_mut(&klass)).clone();
        if let Some(merged) = merged {
            data.merge(merged);
        }

        let mut new_parents = HashMap::new();
        for (mut node, klass) in data.parents {
            use hashbrown::hash_map::Entry;

            self.eg.node_classes.remove(&node).unwrap_or_else(|| {
                self.eg
                    .node_classes
                    .remove(rewrites.get(&node).unwrap())
                    .unwrap()
            });
            let old_node = node.clone();
            safe_nodes(node.canonicalize(self.eg));
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

            safe_nodes(node.canonicalize(self.eg));
            rewrites.insert(old_node, node.clone());
            self.eg.node_classes.insert(node.clone(), root);
        }
    }
}
