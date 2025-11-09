use hashbrown::{HashMap, HashSet};
pub use node::*;
pub use trace::EGraphTrace;

use crate::{
    dot,
    union_find::{ClassId, NoNode, Unioned},
};

pub mod congr;
pub mod fast;
mod node;
pub mod reference;
pub mod trace;

// TODO: a lot of this module could be cleaned up if they introduced a solution
//       for better derive bounds

pub mod prelude {
    pub use super::{EGraphCore, EGraphRead, EGraphUpgrade, EGraphUpgradeTrace, EGraphWrite};
}

pub trait EGraphCore {
    type FuncSymbol;
    type Class;

    fn add(
        &mut self,
        node: ENode<Self::FuncSymbol, Self::Class>,
    ) -> Result<ClassId<Self::Class>, NoNode>;
}

impl<T: ?Sized + EGraphCore> EGraphCore for &mut T {
    type Class = T::Class;
    type FuncSymbol = T::FuncSymbol;

    fn add(
        &mut self,
        node: ENode<Self::FuncSymbol, Self::Class>,
    ) -> Result<ClassId<Self::Class>, NoNode> {
        T::add(self, node)
    }
}

pub type ClassNodes<'a, G, S = hashbrown::DefaultHashBuilder> = HashMap<
    ClassId<<G as EGraphCore>::Class>,
    HashSet<&'a ENode<<G as EGraphCore>::FuncSymbol, <G as EGraphCore>::Class>, S>,
    S,
>;

pub trait EGraphRead: EGraphCore {
    type Hasher;

    fn find(&self, class: ClassId<Self::Class>) -> Result<ClassId<Self::Class>, NoNode>;

    /// Returns true if the node was modified
    fn canonicalize(&self, node: &mut ENode<Self::FuncSymbol, Self::Class>)
        -> Result<bool, NoNode>;

    fn is_canonical(&self, node: &ENode<Self::FuncSymbol, Self::Class>) -> Result<bool, NoNode>;

    fn class_nodes(&self) -> ClassNodes<'_, Self, Self::Hasher>;

    fn dot<M: trace::dot::Formatter<Self::FuncSymbol>>(&self, f: M) -> dot::Graph<'static>;
}

pub trait EGraphWrite: EGraphCore {
    fn merge(
        &mut self,
        a: ClassId<Self::Class>,
        b: ClassId<Self::Class>,
    ) -> Result<Unioned<Self::Class>, NoNode>;
}

pub trait EGraphWriteTrace: EGraphCore {
    fn merge_trace<T: EGraphTrace<Self::FuncSymbol, Self::Class>>(
        &mut self,
        a: ClassId<Self::Class>,
        b: ClassId<Self::Class>,
        t: &mut T,
    ) -> Result<Unioned<Self::Class>, NoNode>;
}

impl<T: EGraphWrite> EGraphWrite for &mut T {
    fn merge(
        &mut self,
        a: ClassId<Self::Class>,
        b: ClassId<Self::Class>,
    ) -> Result<Unioned<Self::Class>, NoNode> {
        T::merge(self, a, b)
    }
}

impl<G: EGraphWriteTrace> EGraphWriteTrace for &mut G {
    fn merge_trace<T: EGraphTrace<Self::FuncSymbol, Self::Class>>(
        &mut self,
        a: ClassId<Self::Class>,
        b: ClassId<Self::Class>,
        t: &mut T,
    ) -> Result<Unioned<Self::Class>, NoNode> {
        G::merge_trace(self, a, b, t)
    }
}

pub trait EGraphUpgradeTrace: EGraphRead {
    type WriteRef<'a, T: EGraphTrace<Self::FuncSymbol, Self::Class>>: EGraphWrite<
        FuncSymbol = Self::FuncSymbol,
        Class = Self::Class,
    >
    where Self: 'a;

    fn write_trace<T: EGraphTrace<Self::FuncSymbol, Self::Class>>(
        &mut self,
        tracer: T,
    ) -> Self::WriteRef<'_, T>;
}

pub trait EGraphUpgrade: EGraphUpgradeTrace {
    #[inline]
    fn write(&mut self) -> Self::WriteRef<'_, ()> { self.write_trace(()) }
}

impl<G: EGraphUpgradeTrace> EGraphUpgrade for G {}

#[derive(Debug)]
pub struct SimpleWriteRef<'a, G, T>(&'a mut G, T);

impl<G: EGraphRead + EGraphWriteTrace> EGraphUpgradeTrace for G {
    type WriteRef<'a, T: EGraphTrace<Self::FuncSymbol, Self::Class>>
        = SimpleWriteRef<'a, G, T>
    where Self: 'a;

    #[inline]
    fn write_trace<T: EGraphTrace<Self::FuncSymbol, Self::Class>>(
        &mut self,
        tracer: T,
    ) -> Self::WriteRef<'_, T> {
        SimpleWriteRef(self, tracer)
    }
}

impl<G: EGraphCore, T> EGraphCore for SimpleWriteRef<'_, G, T> {
    type Class = G::Class;
    type FuncSymbol = G::FuncSymbol;

    #[inline]
    fn add(
        &mut self,
        node: ENode<Self::FuncSymbol, Self::Class>,
    ) -> Result<ClassId<Self::Class>, NoNode> {
        self.0.add(node)
    }
}

impl<G: EGraphWriteTrace, T: EGraphTrace<G::FuncSymbol, G::Class>> EGraphWrite
    for SimpleWriteRef<'_, G, T>
{
    #[inline]
    fn merge(
        &mut self,
        a: ClassId<Self::Class>,
        b: ClassId<Self::Class>,
    ) -> Result<Unioned<Self::Class>, NoNode> {
        self.0.merge_trace(a, b, &mut self.1)
    }
}

pub mod test_tools {
    use std::{collections::BTreeMap, fmt};

    use hashbrown::HashMap;

    use super::ENode;
    use crate::{bijection::Bijection, union_find::ClassId};

    #[derive(Debug)]
    pub struct EGraphParts<F, C> {
        pub uf: crate::union_find::UnionFind<C>,
        pub node_classes: HashMap<ENode<F, C>, ClassId<C>>,
    }

    pub fn assert_equiv<
        F: fmt::Debug + Ord,
        C,
        A: Into<EGraphParts<F, C>>,
        B: Into<EGraphParts<F, C>>,
    >(
        a: A,
        b: B,
    ) -> Bijection<ClassId<C>, ClassId<C>> {
        let EGraphParts {
            uf: a_uf,
            node_classes: a_node_classes,
        } = a.into();
        let EGraphParts {
            uf: b_uf,
            node_classes: b_node_classes,
        } = b.into();

        let mut mapping = Bijection::new();

        assert_eq!(a_uf.len(), b_uf.len());
        for (a, b) in a_uf.classes().zip(b_uf.classes()) {
            mapping
                .insert(a_uf.find(a).unwrap(), b_uf.find(b).unwrap())
                .unwrap();
        }

        let a_node_classes_mapped: BTreeMap<_, _> = a_node_classes
            .iter()
            .map(|(n, c)| {
                let mut n = n.clone();
                n.map_args(|c| *mapping.image(&c).unwrap());
                (n, *mapping.image(c).unwrap())
            })
            .collect();

        let b_node_classes_mapped: BTreeMap<_, _> = b_node_classes
            .iter()
            .map(|(n, c)| {
                let mut n = n.clone();
                n.map_args(|c| *mapping.preimage(&c).unwrap());
                (n, *mapping.preimage(c).unwrap())
            })
            .collect();

        assert_eq!(
            a_node_classes.into_iter().collect::<BTreeMap<_, _>>(),
            b_node_classes_mapped
        );
        assert_eq!(
            a_node_classes_mapped,
            b_node_classes.into_iter().collect::<BTreeMap<_, _>>()
        );

        mapping
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

    use foldhash::fast::FixedState;
    use prop::sample::SizeRange;
    use proptest::prelude::*;

    use super::{prelude::*, test_tools::assert_equiv};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct Symbol(char);
    #[derive(Debug, Clone)]
    struct Tree(Symbol, Vec<Tree>);

    #[derive(Debug)]
    struct Expr;

    impl Tree {
        fn fold_impl<T>(self, f: &mut impl FnMut(Symbol, Vec<T>) -> T) -> T {
            let Self(sym, children) = self;
            let children = children.into_iter().map(|c| c.fold_impl(f)).collect();
            f(sym, children)
        }

        #[inline]
        fn fold<T>(self, mut f: impl FnMut(Symbol, Vec<T>) -> T) -> T { self.fold_impl(&mut f) }

        fn count(&self) -> usize {
            self.1
                .iter()
                .map(Tree::count)
                .reduce(|l, r| l.checked_add(r).unwrap())
                .unwrap_or(0)
                .checked_add(1)
                .unwrap()
        }
    }

    type Node = super::ENode<Symbol, Expr>;
    type SlowGraph = super::reference::EGraph<Symbol, Expr>;
    type CongrGraph = super::congr::EGraph<Symbol, Expr>;
    type FastGraph<S> = super::fast::EGraph<Symbol, Expr, S>;

    type Parts = super::test_tools::EGraphParts<Symbol, Expr>;

    // TODO: track that only merged and originally-equivalent nodes are still equivalent
    // fn assert_merges<G: EGraphRead>(
    //     merges: &[(usize, usize)],
    //     class: impl Fn(usize) -> ClassId<G::Class>,
    // ) {
    // }

    // TODO: test adding after merging
    fn run_reference(tree: &Tree, merges: &Vec<(usize, usize)>) {
        let mut graph = SlowGraph::new();
        let mut classes = BTreeMap::new();
        let mut class_list = vec![];

        let root = tree.clone().fold(|sym, args| {
            let node = Node::new(sym.into(), args.into());
            let class = graph.add(node.clone()).unwrap();

            class_list.push(class);

            assert_eq!(*classes.entry(node).or_insert(class), class);

            class
        });

        graph.find(root).unwrap();

        for &(a, b) in merges {
            let a = class_list[a];
            let b = class_list[b];
            graph.write().merge(a, b).unwrap();
        }
    }

    // TODO: test adding after merging
    fn run<
        A: Default + Clone + EGraphUpgrade<FuncSymbol = Symbol, Class = Expr> + Into<Parts>,
        B: Clone + EGraphUpgrade<FuncSymbol = Symbol, Class = Expr> + Into<Parts>,
    >(
        mut b: B,
        tree: &Tree,
        merges: &Vec<(usize, usize)>,
        stepwise: bool,
    ) {
        let mut a = A::default();
        let mut classes = BTreeMap::new();
        let mut class_list = vec![];

        let root = tree.clone().fold(|sym, args| {
            let node = Node::new(sym.into(), args.into());

            let class = a.add(node.clone()).unwrap();
            assert_eq!(b.add(node.clone()).unwrap(), class);

            class_list.push(class);

            assert_eq!(*classes.entry(node).or_insert(class), class);

            class
        });

        // Sanity assertion that the root ended up in the graphs
        a.find(root).unwrap();
        b.find(root).unwrap();

        if stepwise {
            for &(l, r) in merges {
                let l = class_list[l];
                let r = class_list[r];
                a.write().merge(l, r).unwrap();
                b.write().merge(l, r).unwrap();

                assert_equiv(a.clone(), b.clone());
            }
        } else {
            {
                let mut a = a.write();
                let mut b = b.write();

                for &(l, r) in merges {
                    let l = class_list[l];
                    let r = class_list[r];
                    a.merge(l, r).unwrap();
                    b.merge(l, r).unwrap();
                }
            }

            assert_equiv(a, b);
        }
    }

    fn nodes_and_merges(
        symbol: impl Strategy<Value = char> + Clone + 'static,
        depth: u32,
        tree_size: u32,
        branch_size: u32,
        merge_size: impl Into<SizeRange>,
    ) -> impl Strategy<Value = (Tree, Vec<(usize, usize)>)> {
        let sym = symbol.prop_map(Symbol);
        let merge_size = merge_size.into();

        sym.clone()
            .prop_map(|s| Tree(s, vec![]))
            .prop_recursive(depth, tree_size, branch_size, move |t| {
                (
                    sym.clone(),
                    prop::collection::vec(t, 0..=(branch_size.try_into().unwrap())),
                )
                    .prop_map(|(s, c)| Tree(s, c))
            })
            .prop_flat_map(move |t| {
                let id = 0..t.count();
                prop::collection::vec((id.clone(), id), merge_size.clone())
                    .prop_map(move |m| (t.clone(), m))
            })
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            // cases: 1 << 17,
            max_shrink_time: 0,
            max_shrink_iters: 16384,
            // max_shrink_iters: 1 << 18,
            ..ProptestConfig::default()
        })]

        #[test]
        fn reference(
            (nodes, merges) in nodes_and_merges(
                crate::prop::symbol(),
                32,
                512,
                6,
                1..=128,
            ),
        ) {
            run_reference(&nodes, &merges);
        }

        #[test]
        fn batched(
            (nodes, merges) in nodes_and_merges(
                crate::prop::symbol(),
                32,
                512,
                6,
                1..=128,
            ),
            seed in any::<u64>(),
        ) {
            run::<SlowGraph, FastGraph<_>>(
                FastGraph::with_hasher(FixedState::with_seed(seed)),
                &nodes,
                &merges,
                false,
            );
        }

        #[test]
        fn stepwise(
            (nodes, merges) in nodes_and_merges(
                crate::prop::symbol(),
                32,
                512,
                6,
                1..=128,
            ),
            seed in any::<u64>(),
        ) {
            run::<SlowGraph, FastGraph<_>>(
                FastGraph::with_hasher(FixedState::with_seed(seed)),
                &nodes,
                &merges,
                true,
            );
        }

        #[test]
        fn congr(
            (nodes, merges) in nodes_and_merges(
                crate::prop::symbol(),
                32,
                512,
                6,
                1..=128,
            ),
        ) {
            run::<SlowGraph, CongrGraph>(CongrGraph::default(), &nodes, &merges, true);
        }
    }
}
