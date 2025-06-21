use std::collections::BTreeMap;

use arbitrary::Arbitrary;
#[cfg(feature = "trace")]
use shrec::egraph::trace::{dot, DotTracer};
use shrec::{
    bijection::Bijection,
    egraph::{self, prelude::*},
    union_find::ClassId,
};

struct Tracer(
    #[cfg(not(feature = "trace"))] (),
    #[cfg(feature = "trace")] DotTracer<dot::DebugFormatter>,
);

impl Tracer {
    #[cfg(not(feature = "trace"))]
    fn new() -> Self { Self(()) }

    #[cfg(feature = "trace")]
    fn new() -> Self { Self(DotTracer::debug()) }

    #[cfg(not(feature = "trace"))]
    fn flush(&mut self) {}

    #[cfg(feature = "trace")]
    fn flush(&mut self) { self.0.flush(|dot::Snapshot { graph }| println!("{graph}")); }
}

impl Drop for Tracer {
    fn drop(&mut self) { self.flush(); }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Arbitrary)]
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
pub type IntrusiveGraph = egraph::intrusive::EGraph<Symbol, Expr>;
pub type FastGraph = egraph::fast::EGraph<Symbol, Expr>;

type Parts = egraph::test_tools::EGraphParts<Symbol, Expr>;

// TODO: track that only merged and originally-equivalent nodes are still equivalent
fn assert_merges<G: EGraphRead>(
    merges: &[(usize, usize)],
    klass: impl Fn(usize) -> ClassId<G::Class>,
) {
}

fn assert_equiv<A: Clone + Into<Parts>, B: Clone + Into<Parts>>(a: &A, b: &B) {
    let Parts {
        uf: a_uf,
        node_classes: a_node_classes,
    } = a.clone().into();
    let Parts {
        uf: b_uf,
        node_classes: b_node_classes,
    } = b.clone().into();

    let mut mapping = Bijection::new();

    assert_eq!(a_uf.len(), b_uf.len());
    for (a, b) in a_uf.classes().zip(b_uf.classes()) {
        mapping
            .insert(a_uf.find(a).unwrap(), b_uf.find(b).unwrap())
            .unwrap();
    }

    #[cfg(feature = "trace")]
    {
        eprintln!("{mapping:?}");
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

    assert_eq!(a_node_classes, b_node_classes_mapped);
    assert_eq!(a_node_classes_mapped, b_node_classes);
}

// TODO: test adding after merging
pub fn run_reference<
    G: Default + Clone + EGraphUpgrade<FuncSymbol = Symbol, Class = Expr> + Into<Parts>,
>(tree: &Tree, merges: &Vec<(usize, usize)>) {
    let mut graph = G::default();
    let mut t = Tracer::new();
    let mut classes = BTreeMap::new();
    let mut class_list = vec![];

    let root = tree.clone().fold(|sym, args| {
        let node = Node::new(sym.into(), args.into());
        let klass = graph.add(node.clone()).unwrap();

        class_list.push(klass);

        assert_eq!(*classes.entry(node).or_insert(klass), klass);

        klass
    });

    graph.find(root).unwrap();

    for &(a, b) in merges {
        let a = class_list[a];
        let b = class_list[b];
        graph.write().merge_trace(a, b, &mut t.0).unwrap();
    }

    t.flush();
}

// TODO: test adding after merging
pub fn run_differential<
    A: Default + Clone + EGraphUpgrade<FuncSymbol = Symbol, Class = Expr> + Into<Parts>,
    B: Default + Clone + EGraphUpgrade<FuncSymbol = Symbol, Class = Expr> + Into<Parts>,
>(
    tree: &Tree,
    merges: &Vec<(usize, usize)>,
    stepwise: bool,
) {
    let mut a = A::default();
    let mut b = B::default();
    let mut t = Tracer::new();
    let mut classes = BTreeMap::new();
    let mut class_list = vec![];

    let root = tree.clone().fold(|sym, args| {
        let node = Node::new(sym.into(), args.into());

        let klass = a.add(node.clone()).unwrap();
        assert_eq!(b.add(node.clone()).unwrap(), klass);

        class_list.push(klass);

        assert_eq!(*classes.entry(node).or_insert(klass), klass);

        klass
    });

    // Sanity assertion that the root ended up in the graphs
    a.find(root).unwrap();
    b.find(root).unwrap();

    if stepwise {
        for &(l, r) in merges {
            let l = class_list[l];
            let r = class_list[r];
            a.write().merge(l, r).unwrap();
            b.write().merge_trace(l, r, &mut t.0).unwrap();

            t.flush();

            assert_equiv(&a, &b);
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

        t.flush();

        assert_equiv(&a, &b);
    }
}
