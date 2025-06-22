use std::collections::BTreeMap;

use arbitrary::Arbitrary;
use foldhash::fast::FixedState;
#[cfg(feature = "trace")]
use shrec::egraph::trace::{dot, DotTracer};
use shrec::egraph::{self, prelude::*, test_tools};

struct Tracer(
    #[cfg(not(feature = "trace"))] (),
    #[cfg(feature = "trace")] DotTracer<dot::DebugFormatter, fn(dot::Snapshot)>,
);

impl Tracer {
    #[cfg(feature = "trace")]
    fn print(dot::Snapshot { graph }: dot::Snapshot) { println!("{graph}") }

    #[cfg(not(feature = "trace"))]
    fn new() -> Self { Self(()) }

    #[cfg(feature = "trace")]
    fn new() -> Self { Self(DotTracer::debug(Self::print)) }

    #[cfg(not(feature = "trace"))]
    fn flush(&mut self) {}

    #[cfg(feature = "trace")]
    fn flush(&mut self) { self.0.flush() }
}

impl Drop for Tracer {
    fn drop(&mut self) { self.flush() }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Arbitrary)]
pub struct Symbol(char);
#[derive(Debug, Clone, Arbitrary)]
pub struct Tree(Symbol, Vec<Tree>);

#[derive(Debug)]
pub struct Expr;

impl Tree {
    fn fold_impl<T>(self, f: &mut impl FnMut(Symbol, Vec<T>) -> T) -> T {
        let Self(sym, children) = self;
        let children = children.into_iter().map(|c| c.fold_impl(f)).collect();
        f(sym, children)
    }

    #[inline]
    fn fold<T>(self, mut f: impl FnMut(Symbol, Vec<T>) -> T) -> T { self.fold_impl(&mut f) }

    pub fn count(&self) -> usize {
        self.1
            .iter()
            .map(Tree::count)
            .reduce(|l, r| l.checked_add(r).unwrap())
            .unwrap_or(0)
            .checked_add(1)
            .unwrap()
    }
}

type Node = egraph::ENode<Symbol, Expr>;
pub type SlowGraph = egraph::reference::EGraph<Symbol, Expr>;
pub type CongrGraph = egraph::congr::EGraph<Symbol, Expr>;
pub type FastGraph<S> = egraph::fast::EGraph<Symbol, Expr, S>;

type Parts = egraph::test_tools::EGraphParts<Symbol, Expr>;

// TODO: track that only merged and originally-equivalent nodes are still equivalent
// fn assert_merges<G: EGraphRead>(
//     merges: &[(usize, usize)],
//     class: impl Fn(usize) -> ClassId<G::Class>,
// ) {
// }

fn assert_equiv<A: Into<Parts>, B: Into<Parts>>(a: A, b: B) {
    let mapping = test_tools::assert_equiv(a, b);

    #[cfg(not(feature = "trace"))]
    {
        let _ = mapping;
    }

    #[cfg(feature = "trace")]
    {
        eprintln!("{mapping:?}");
    }
}

#[derive(Arbitrary)]
pub struct Input(Tree, Vec<(usize, usize)>, bool, u64);

impl Input {
    // TODO: test adding after merging
    pub fn run_reference<
        G: Default + Clone + EGraphUpgrade<FuncSymbol = Symbol, Class = Expr> + Into<Parts>,
    >(
        self,
    ) {
        let Self(tree, merges, ..) = self;
        let len = tree.count();

        if len == 0 {
            return;
        }

        let merges: Vec<_> = merges
            .into_iter()
            .map(|(a, b)| (a % len, b % len))
            .collect();

        let mut graph = G::default();
        let mut t = Tracer::new();
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

        for (a, b) in merges {
            let a = class_list[a];
            let b = class_list[b];
            graph.write_trace(&mut t.0).merge(a, b).unwrap();
        }

        t.flush();
    }

    // TODO: test adding after merging
    pub fn run_differential<
        A: Default + Clone + EGraphUpgrade<FuncSymbol = Symbol, Class = Expr> + Into<Parts>,
        B: Clone + EGraphUpgrade<FuncSymbol = Symbol, Class = Expr> + Into<Parts>,
        F: FnOnce(FixedState) -> B,
    >(
        self,
        f: F,
    ) {
        let Self(tree, merges, stepwise, seed) = self;
        let len = tree.count();

        if len == 0 {
            return;
        }

        let merges: Vec<_> = merges
            .into_iter()
            .map(|(a, b)| (a % len, b % len))
            .collect();

        #[cfg(feature = "trace")]
        {
            eprintln!(
                "Using {} execution",
                if stepwise { "stepwise" } else { "batched" }
            );
        }

        let mut a = A::default();
        let mut b = f(FixedState::with_seed(seed));
        let mut t = Tracer::new();
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
            for (l, r) in merges {
                let l = class_list[l];
                let r = class_list[r];
                a.write().merge(l, r).unwrap();
                b.write_trace(&mut t.0).merge(l, r).unwrap();

                t.flush();

                assert_equiv(a.clone(), b.clone());
            }
        } else {
            {
                let mut a = a.write();
                let mut b = b.write_trace(&mut t.0);

                for (l, r) in merges {
                    let l = class_list[l];
                    let r = class_list[r];
                    a.merge(l, r).unwrap();
                    b.merge(l, r).unwrap();
                }
            }

            t.flush();

            assert_equiv(a.clone(), b.clone());
        }
    }
}
