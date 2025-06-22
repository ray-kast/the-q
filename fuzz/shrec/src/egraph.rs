use std::{collections::BTreeMap, num::NonZeroU8};

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
pub struct Tree(Symbol, u64, u64, Vec<Tree>);

#[derive(Debug)]
pub struct Expr;

impl Tree {
    fn fold_impl<T>(self, f: &mut impl FnMut(Symbol, (u64, u64), Vec<T>) -> T) -> T {
        let Self(sym, l_order, r_order, children) = self;
        let children = children.into_iter().map(|c| c.fold_impl(f)).collect();
        f(sym, (l_order, r_order), children)
    }

    #[inline]
    fn fold<T>(self, mut f: impl FnMut(Symbol, (u64, u64), Vec<T>) -> T) -> T {
        self.fold_impl(&mut f)
    }

    pub fn count(&self) -> usize {
        self.3
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

#[derive(Debug, Arbitrary)]
pub struct Input {
    batch_size: NonZeroU8,
    hash_seed: u64,
    tree: Tree,
}

impl Input {
    // TODO: test adding after merging
    pub fn run_reference<
        G: Default + Clone + EGraphUpgrade<FuncSymbol = Symbol, Class = Expr> + Into<Parts>,
    >(
        self,
    ) {
        let Self {
            batch_size: _,
            hash_seed: _,
            tree,
        } = self;
        let len = tree.count();

        if len == 0 {
            return;
        }

        let mut graph = G::default();
        let mut t = Tracer::new();
        let mut classes = BTreeMap::new();
        let mut l_classes = vec![];
        let mut r_classes = vec![];

        let root = tree.clone().fold(|sym, (l, r), args| {
            let node = Node::new(sym.into(), args.into());
            let class = graph.add(node.clone()).unwrap();

            l_classes.push((l, class));
            r_classes.push((r, class));

            assert_eq!(*classes.entry(node).or_insert(class), class);

            class
        });

        graph.find(root).unwrap();

        l_classes.sort_unstable();
        r_classes.sort_unstable();

        {
            let mut wr = graph.write_trace(&mut t.0);

            for ((_, a), (_, b)) in l_classes.into_iter().zip(r_classes) {
                wr.merge(a, b).unwrap();
            }
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
        let Self {
            batch_size,
            hash_seed,
            tree,
        } = self;
        let len = tree.count();

        if len == 0 {
            return;
        }

        #[cfg(feature = "trace")]
        {
            eprintln!("Using batch size {batch_size}");
        }

        let mut a = A::default();
        let mut b = f(FixedState::with_seed(hash_seed));
        let mut t = Tracer::new();
        let mut classes = BTreeMap::new();
        let mut l_classes = vec![];
        let mut r_classes = vec![];

        let root = tree.clone().fold(|sym, (l, r), args| {
            let node = Node::new(sym.into(), args.into());

            let class = a.add(node.clone()).unwrap();
            assert_eq!(b.add(node.clone()).unwrap(), class);

            l_classes.push((l, class));
            r_classes.push((r, class));

            assert_eq!(*classes.entry(node).or_insert(class), class);

            class
        });

        // Sanity assertion that the root ended up in the graphs
        a.find(root).unwrap();
        b.find(root).unwrap();

        l_classes.sort_unstable();
        r_classes.sort_unstable();

        let mut it = l_classes
            .into_iter()
            .zip(r_classes)
            .map(|((_, l), (_, r))| (l, r));

        'iter: loop {
            {
                let mut a = a.write();
                let mut b = b.write_trace(&mut t.0);

                for _ in 0..batch_size.get() {
                    let Some((l, r)) = it.next() else { break 'iter };

                    a.merge(l, r).unwrap();
                    b.merge(l, r).unwrap();
                }
            }

            t.flush();

            assert_equiv(a.clone(), b.clone());
        }
    }
}
