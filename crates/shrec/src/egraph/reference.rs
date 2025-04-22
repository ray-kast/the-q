use std::{borrow::Cow, fmt, hash::Hash, mem};

use hashbrown::{HashMap, HashSet};

use super::{prelude::*, ENode};
use crate::{
    dot,
    union_find::{ClassId, NoNode, UnionFind, Unioned},
};

struct EClassData<F, C> {
    parents: HashMap<ENode<F, C>, ClassId<C>>,
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

impl<F: Eq + Hash, C> EClassData<F, C> {
    fn new() -> Self {
        Self {
            parents: HashMap::new(),
        }
    }
}

pub struct EGraph<F, C> {
    uf: UnionFind<C>,
    class_data: HashMap<ClassId<C>, EClassData<F, C>>,
    node_classes: HashMap<ENode<F, C>, ClassId<C>>,
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
            class_data: HashMap::new(),
            node_classes: HashMap::new(),
        }
    }
}

impl<F: Eq + Hash, C> EGraph<F, C> {
    #[cfg(test)]
    #[must_use]
    pub(super) fn into_parts(self) -> super::EGraphParts<F, C> {
        let Self {
            uf,
            class_data,
            node_classes,
        } = self;

        let class_refs = class_data
            .into_iter()
            .map(|(k, EClassData { parents })| {
                (
                    k,
                    parents
                        .into_keys()
                        .map(|mut n| {
                            n.canonicalize_classes(&uf).unwrap();
                            n
                        })
                        .collect(),
                )
            })
            .collect();

        super::EGraphParts {
            uf,
            class_refs,
            node_classes,
        }
    }
}

impl<F: Eq + Hash, C> EGraph<F, C> {
    #[must_use]
    pub fn class_nodes(&self) -> HashMap<ClassId<C>, HashSet<&ENode<F, C>>> {
        self.node_classes
            .iter()
            .fold(HashMap::new(), |mut m: HashMap<_, HashSet<_>>, (n, &c)| {
                assert!(n.is_canonial(self).unwrap());
                assert!(m.entry(self.uf.find(c).unwrap()).or_default().insert(n));
                m
            })
    }
}

impl<F: Eq + Hash, C> EGraphCore for EGraph<F, C> {
    type Class = C;
    type FuncSymbol = F;

    fn add(&mut self, mut node: ENode<F, C>) -> Result<ClassId<C>, NoNode> {
        node.canonicalize_classes(&self.uf)?;
        Ok(if let Some(&klass) = self.node_classes.get(&node) {
            assert_eq!(klass, self.uf.find(klass).unwrap());
            klass
        } else {
            let klass = self.uf.add();
            assert!(self.class_data.insert(klass, EClassData::new()).is_none());

            for &arg in node.args() {
                assert!(self
                    .class_data
                    .get_mut(&arg)
                    .unwrap()
                    .parents
                    .insert(node.clone(), klass)
                    .is_none_or(|c| klass == c));
            }

            self.node_classes.insert(node, klass);
            klass
        })
    }
}

impl<F: Eq + Hash, C> EGraphRead for EGraph<F, C> {
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
    fn dot<'a, O: Fn(&F, ClassId<C>) -> Cow<'a, str>, E: Fn(&F, usize) -> Option<Cow<'a, str>>>(
        &self,
        fmt_op: O,
        fmt_edge: E,
    ) -> dot::Graph<'a> {
        dot::Graph::egraph(self.class_nodes(), fmt_op, fmt_edge)
    }
}

impl<F: Eq + Hash, C> EGraphWrite for EGraph<F, C> {
    fn merge(&mut self, a: ClassId<C>, b: ClassId<C>) -> Result<Unioned<C>, NoNode> {
        let ret = self.merge_impl(a, b, &mut self.uf.clone());
        self.assert_invariants(true);
        ret
    }
}

impl<F: Eq + Hash, C> EGraph<F, C> {
    fn merge_impl(
        &mut self,
        a: ClassId<C>,
        b: ClassId<C>,
        old_uf: &mut UnionFind<C>,
        // i: &str,
    ) -> Result<Unioned<C>, NoNode> {
        // println!("{i}[merge a = {a:?}, b = {b:?}]");
        // println!("{i}    uf = {:?}", self.uf);

        old_uf.clone_from(&self.uf);
        let union = self.uf.union(a, b)?;
        // println!("{i}    union = {union:?}");
        let Unioned { root, unioned } = union;

        if let Some(unioned) = unioned {
            // println!("{i}    class_data = {:?}", self.class_data);
            // println!("{i}    node_classes = {:?}", self.node_classes);

            let EClassData { parents } = self.class_data.remove(&unioned).unwrap();
            // println!("{i}    parents = {parents:?}");
            let root_data = self.class_data.get_mut(&root).unwrap();
            // println!("{i}    root_data = {root_data:?}");

            let mut to_merge = vec![];
            let mut new_parents = HashMap::with_capacity(root_data.parents.len());
            let mut new_nodes = HashMap::with_capacity(root_data.parents.len() + parents.len());
            for (mut par, par_class) in parents.into_iter().chain(root_data.parents.drain()) {
                // println!("{i}    [parent node = {par:?}, class = {par_class:?}]");
                assert!(par.args().iter().any(|&c| c == unioned || c == root));

                // print!("{i}        canonicalize_old({par:?}) ");
                par.canonicalize_classes(old_uf).unwrap();
                // println!("-> {par:?}");

                let par_class = self.uf.find(par_class).unwrap();
                let old = self.node_classes.remove(&par);

                // print!("{i}        canonicalize({par:?}) ");
                par.canonicalize_classes(&self.uf).unwrap();
                // println!("-> {par:?}");

                if let Some(old) = old {
                    assert_eq!(self.uf.find(old).unwrap(), par_class);
                } else {
                    assert!(new_parents.contains_key(&par));
                }

                if let Some(other_par_class) = new_parents.insert(par.clone(), par_class) {
                    // println!("{i}        zip {par_class:?}, {other_par_class:?}");
                    to_merge.push((par_class, other_par_class));
                } else if let Some(other_node_class) = new_nodes.insert(par, par_class) {
                    assert_eq!(self.uf.find(other_node_class).unwrap(), par_class);
                }
            }

            assert!(mem::replace(&mut root_data.parents, new_parents).is_empty());
            let expected_len = self.node_classes.len() + new_nodes.len();
            self.node_classes.extend(new_nodes);
            assert_eq!(self.node_classes.len(), expected_len);

            // println!("{i}    node_classes = {:?}", self.node_classes);
            // println!("{i}--  to_merge = {to_merge:?}");

            self.assert_invariants(false);

            for (a, b) in to_merge {
                self.merge_impl(a, b, old_uf).unwrap();
            }
        } else {
            // println!("{i}--  done");

            self.assert_invariants(false);
        }

        Ok(union)
    }

    #[cfg(not(test))]
    #[inline]
    fn assert_invariants(&self, _: bool) { let _ = self; }

    #[cfg(test)]
    fn assert_invariants(&self, merged: bool) {
        for node in self.node_classes.keys() {
            assert!(node.classes_canonical(&self.uf).unwrap());
        }

        for (&klass, EClassData { parents }) in &self.class_data {
            assert_eq!(klass, self.uf.find(klass).unwrap());

            if merged {
                let mut seen = HashMap::new();
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
