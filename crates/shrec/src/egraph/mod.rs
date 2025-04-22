use std::borrow::Cow;

pub use fast::*;
pub use node::*;

use crate::{
    dot,
    union_find::{ClassId, NoNode, Unioned},
};

mod fast;
pub mod node;
pub mod reference;

// TODO: a lot of this module could be cleaned up if they introduced a solution
//       for better derive bounds

pub mod prelude {
    pub use super::{EGraphCore, EGraphRead, EGraphUpgrade, EGraphWrite};
}

pub trait EGraphCore {
    type FuncSymbol;
    type Class;

    fn add(
        &mut self,
        node: ENode<Self::FuncSymbol, Self::Class>,
    ) -> Result<ClassId<Self::Class>, NoNode>;
}

pub trait EGraphRead: EGraphCore {
    fn find(&self, class: ClassId<Self::Class>) -> Result<ClassId<Self::Class>, NoNode>;

    /// Returns true if the node was modified
    fn canonicalize(&self, node: &mut ENode<Self::FuncSymbol, Self::Class>)
        -> Result<bool, NoNode>;

    fn is_canonical(&self, node: &ENode<Self::FuncSymbol, Self::Class>) -> Result<bool, NoNode>;

    fn dot<
        'a,
        O: Fn(&Self::FuncSymbol, ClassId<Self::Class>) -> Cow<'a, str>,
        E: Fn(&Self::FuncSymbol, usize) -> Option<Cow<'a, str>>,
    >(
        &self,
        fmt_op: O,
        fmt_edge: E,
    ) -> dot::Graph<'a>;
}

pub trait EGraphWrite: EGraphCore {
    fn merge(
        &mut self,
        a: ClassId<Self::Class>,
        b: ClassId<Self::Class>,
    ) -> Result<Unioned<Self::Class>, NoNode>;
}

pub trait EGraphUpgrade: EGraphRead {
    type WriteRef<'a>
    where Self: 'a;

    fn write(&mut self) -> Self::WriteRef<'_>;
}

impl<T: EGraphRead + EGraphWrite> EGraphUpgrade for T {
    type WriteRef<'a>
        = &'a mut Self
    where Self: 'a;

    fn write(&mut self) -> Self::WriteRef<'_> { self }
}

#[cfg(test)]
#[derive(Debug)]
struct EGraphParts<F, C> {
    uf: crate::union_find::UnionFind<C>,
    class_refs: hashbrown::HashMap<ClassId<C>, hashbrown::HashSet<ENode<F, C>>>,
    node_classes: hashbrown::HashMap<ENode<F, C>, ClassId<C>>,
}

#[cfg(test)]
mod test {
    use hashbrown::HashMap;
    use prop::sample::SizeRange;
    use proptest::prelude::*;

    use super::prelude::*;
    use crate::union_find::ClassId;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    type FastGraph = super::fast::EGraph<Symbol, Expr>;

    // TODO: track that only merged and originally-equivalent nodes are still equivalent
    fn assert_merges<G: EGraphRead>(
        merges: &[(usize, usize)],
        klass: impl Fn(usize) -> ClassId<G::Class>,
    ) {
    }

    fn assert_equiv(slow: &SlowGraph, fast: &FastGraph) {
        let super::EGraphParts {
            uf: slow_uf,
            class_refs: slow_class_refs,
            node_classes: slow_node_classes,
        } = slow.clone().into_parts();
        let super::EGraphParts {
            uf: fast_uf,
            class_refs: fast_class_refs,
            node_classes: fast_node_classes,
        } = fast.clone().into_parts();

        assert_eq!(slow_uf.len(), fast_uf.len());

        for (slow, fast) in slow_uf.classes().zip(fast_uf.classes()) {
            assert_eq!(
                slow_uf.find(slow).unwrap().id(),
                fast_uf.find(fast).unwrap().id()
            );
        }

        assert_eq!(slow_class_refs, fast_class_refs);
        assert_eq!(slow_node_classes, fast_node_classes);
    }

    // TODO: test adding after merging
    fn run_reference(tree: Tree, merges: Vec<(usize, usize)>) {
        for _ in 0..64 {
            let mut graph = SlowGraph::new();
            let mut classes = HashMap::new();
            let mut class_list = vec![];

            let root = tree.clone().fold(|sym, args| {
                let node = Node::new(sym.into(), args.into());
                let klass = graph.add(node.clone()).unwrap();

                class_list.push(klass);

                assert_eq!(*classes.entry(node).or_insert(klass), klass);

                klass
            });

            graph.find(root).unwrap();

            for &(a, b) in &merges {
                let a = class_list[a];
                let b = class_list[b];
                graph.merge(a, b).unwrap();
            }
        }
    }

    // TODO: test adding after merging
    fn run(tree: Tree, merges: Vec<(usize, usize)>, stepwise: bool) {
        for _ in 0..32 {
            let mut slow = SlowGraph::new();
            let mut fast = FastGraph::new();
            let mut classes = HashMap::new();
            let mut class_list = vec![];

            let root = tree.clone().fold(|sym, args| {
                let node = Node::new(sym.into(), args.into());

                let klass = slow.add(node.clone()).unwrap();
                assert_eq!(fast.add(node.clone()).unwrap(), klass);

                class_list.push(klass);

                assert_eq!(*classes.entry(node).or_insert(klass), klass);

                klass
            });

            // Sanity assertion that the root ended up in the graphs
            slow.find(root).unwrap();
            fast.find(root).unwrap();

            if stepwise {
                for &(a, b) in &merges {
                    let a = class_list[a];
                    let b = class_list[b];
                    slow.merge(a, b).unwrap();
                    fast.write().merge(a, b).unwrap();

                    assert_equiv(&slow, &fast);
                }
            } else {
                {
                    let mut fast = fast.write();

                    for &(a, b) in &merges {
                        let a = class_list[a];
                        let b = class_list[b];
                        slow.merge(a, b).unwrap();
                        fast.merge(a, b).unwrap();
                    }
                }

                assert_equiv(&slow, &fast);
            }
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

    #[test]
    fn test_abc() {
        run_reference(
            Tree(Symbol('a'), vec![
                Tree(Symbol('b'), vec![
                    Tree(Symbol('c'), vec![]),
                    Tree(Symbol('d'), vec![]),
                ]),
                Tree(Symbol('b'), vec![
                    Tree(Symbol('e'), vec![]),
                    Tree(Symbol('f'), vec![]),
                ]),
            ]),
            vec![(1, 3), (1, 4), (1, 0)],
        );
    }

    #[test]
    fn test_non_dedup() {
        run_reference(
            Tree(Symbol('!'), vec![
                Tree(Symbol('¹'), vec![Tree(Symbol('Ό'), vec![
                    Tree(Symbol('A'), vec![]),
                    Tree(Symbol('a'), vec![]),
                    Tree(Symbol('0'), vec![]),
                    Tree(Symbol('A'), vec![]),
                ])]),
                Tree(Symbol('Ό'), vec![
                    Tree(Symbol('!'), vec![]),
                    Tree(Symbol('!'), vec![]),
                    Tree(Symbol('A'), vec![]),
                    Tree(Symbol('͵'), vec![]),
                ]),
            ]),
            vec![(6, 9), (8, 1), (0, 0), (0, 2), (6, 0)],
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            // cases: 2 << 16,
            max_shrink_time: 0,
            max_shrink_iters: 16384,
            ..ProptestConfig::default()
        })]

        #[test]
        fn test_reference(
            (nodes, merges) in nodes_and_merges(
                crate::prop::symbol(),
                32,
                512,
                6,
                1..=128,
            ),
        ) {
            run_reference(nodes, merges);
        }

        #[test]
        fn test_batched(
            (nodes, merges) in nodes_and_merges(
                crate::prop::symbol(),
                32,
                512,
                6,
                1..=128,
            ),
        ) {
            run(nodes, merges, false);
        }

        #[test]
        fn test_stepwise(
            (nodes, merges) in nodes_and_merges(
                crate::prop::symbol(),
                32,
                512,
                6,
                1..=128,
            ),
        ) {
            run(nodes, merges, true);
        }
    }
}
