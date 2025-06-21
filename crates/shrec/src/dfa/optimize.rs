use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    hash::Hash,
    mem,
};

use super::Dfa;
// TODO: switch to the optimized implementation once it...well, works
use crate::{
    dfa::Node,
    egraph::{self, prelude::*, trace::dot, EGraphTrace, ENode},
    union_find::ClassId,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Op<I, N, T> {
    Node {
        accept: Option<T>,
        edges: BTreeSet<I>,
    },
    Impostor(N),
}

impl<I: fmt::Debug, N: fmt::Debug, T: fmt::Debug> dot::Format for Op<I, N, T> {
    fn fmt_node(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Node { accept, .. } => f.write_fmt(format_args!("accept {accept:?}")),
            Self::Impostor(n) => f.write_fmt(format_args!("(impostor {n:?})")),
        }
    }

    fn fmt_edge(&self, idx: usize, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Node { edges, .. } => {
                f.write_fmt(format_args!("{:?}", edges.iter().nth(idx).unwrap()))
            },
            Self::Impostor(_) => unreachable!(),
        }
    }
}

pub type Graph<I, N, T> = egraph::reference::EGraph<Op<I, N, T>, N>;
pub type ClassMap<N> = BTreeMap<N, ClassId<N>>;
pub type Output<I, N, T, G = Graph<I, N, T>> = (Dfa<I, usize, T>, G, ClassMap<N>);

#[inline]
pub(super) fn run_default<I: Copy + Ord, N: Copy + Ord, T: Clone + Ord>(
    dfa: &Dfa<I, N, T>,
) -> Output<I, N, T> {
    run::<I, N, T, Graph<I, N, T>, ()>(dfa, &mut ())
}

pub fn run<
    I: Copy + Ord,
    N: Copy + Ord,
    T: Clone + Ord,
    G: Default + for<'a> EGraphUpgradeTrace<FuncSymbol = Op<I, N, T>, Class = N>,
    R: EGraphTrace<Op<I, N, T>, N>,
>(
    dfa: &Dfa<I, N, T>,
    t: &mut R,
) -> Output<I, N, T, G> {
    enum Command<N> {
        Explore(N),
        Add(N),
    }

    let mut eg = G::default();
    let mut stk = Vec::new();
    let mut classes = BTreeMap::new();
    let mut impostors = BTreeMap::new();

    stk.push(Command::Explore(dfa.start));

    while let Some(node) = stk.pop() {
        match node {
            Command::Explore(n) => {
                use std::collections::btree_map::Entry;

                match classes.entry(n) {
                    Entry::Occupied(_) => continue,
                    Entry::Vacant(v) => drop(v.insert(None)),
                }

                stk.push(Command::Add(n));
                for &n in dfa.states[&n].0.values().rev() {
                    if !classes.contains_key(&n) {
                        stk.push(Command::Explore(n));
                    }
                }
            },
            Command::Add(n) => {
                let Node(ref edges, ref accept) = dfa.states[&n];
                let enode = ENode::new(
                    Op::Node {
                        accept: accept.clone(),
                        edges: edges.keys().copied().collect(),
                    }
                    .into(),
                    edges
                        .values()
                        .map(|&n| {
                            classes[&n].unwrap_or_else(|| {
                                *impostors.entry(n).or_insert_with(|| {
                                    eg.add(ENode::new(Op::Impostor(n).into(), [].into()))
                                        .unwrap()
                                })
                            })
                        })
                        .collect(),
                );

                let klass = eg.add(enode.clone()).unwrap();
                assert!(mem::replace(classes.get_mut(&n).unwrap(), Some(klass)).is_none());
            },
        }
    }

    let classes: BTreeMap<_, _> = classes.into_iter().map(|(k, v)| (k, v.unwrap())).collect();

    let mut wr = eg.write_trace(t);
    for (node, klass) in impostors {
        wr.merge(classes[&node], klass).unwrap();
    }
    drop(wr);

    let states = eg
        .class_nodes()
        .into_iter()
        .map(|(k, v)| (k, v.into_iter().collect()))
        .collect::<BTreeMap<_, BTreeSet<_>>>()
        .into_iter()
        .map(|(c, mut n)| {
            n.retain(|n| !matches!(n.op(), Op::Impostor(_)));
            assert!(n.len() == 1);

            let node = &n.into_iter().next().unwrap();
            let Op::Node { accept, edges } = node.op() else {
                unreachable!();
            };
            let args = node.args();

            (
                c.id(),
                edges
                    .iter()
                    .enumerate()
                    .map(|(i, &e)| (e, args[i].id()))
                    .collect(),
                accept.clone(),
            )
        });

    (
        Dfa::new(states, eg.find(classes[&dfa.start]).unwrap().id()),
        eg,
        classes,
    )
}

#[cfg(test)]
mod test {
    use std::fmt;

    use proptest::prelude::*;

    use super::EGraphUpgrade;
    use crate::{
        egraph::{congr, fast, reference},
        re::kleene,
    };

    // fn print_snap(dot::Snapshot { graph }: dot::Snapshot) { println!("{graph}") }

    // struct FlushOnDrop(DotTracer<dot::RichFormatter, fn(dot::Snapshot)>);

    // impl FlushOnDrop {
    //     fn new() -> Self { Self(DotTracer::rich(print_snap)) }
    // }

    // impl Drop for FlushOnDrop {
    //     fn drop(&mut self) {
    //         if thread::panicking() {
    //             println!("================");
    //             self.0.flush();
    //         }
    //     }
    // }

    fn run<
        G: Default + EGraphUpgrade<FuncSymbol = super::Op<I, N, T>, Class = N>,
        I: Copy + Ord + fmt::Debug,
        N: Copy + Ord + fmt::Debug,
        T: Clone + Ord + fmt::Debug,
    >(
        dfa: &super::Dfa<I, N, T>,
    ) -> super::Output<I, N, T, G> {
        super::run::<_, _, _, G, _>(dfa, &mut ())
    }

    #[expect(clippy::type_complexity, reason = "chill out man, it's a test helper")]
    fn run_ref<I: Copy + Ord, N: Copy + Ord, T: Clone + Ord>(
        dfa: &super::Dfa<I, N, T>,
    ) -> super::Output<I, N, T, reference::EGraph<super::Op<I, N, T>, N>> {
        super::run(dfa, &mut ())
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            // cases: 2 << 16,
            max_shrink_time: 0,
            max_shrink_iters: 16384,
            ..ProptestConfig::default()
        })]

        #[test]
        fn reference(r in kleene::re(
            8,
            64,
            8,
            0..16,
            crate::prop::symbol(),
        )) {
            let nfa = r.compile_atomic();
            let (dfa, _) = nfa.compile().atomize_nodes::<u64>();
            // let mut t = FlushOnDrop::new();

            run::<reference::EGraph<_, _>, _, _, _>(&dfa);
        }

        #[test]
        fn congr(r in kleene::re(
            8,
            64,
            8,
            0..16,
            crate::prop::symbol(),
        )) {
            let nfa = r.compile_atomic();
            let (dfa, _) = nfa.compile().atomize_nodes::<u64>();
            // let mut t = FlushOnDrop::new();

            let (opt, ..) = run::<congr::EGraph<_, _>, _, _, _>(&dfa);
            assert_eq!(opt, run_ref(&dfa).0);
        }

        #[test]
        fn fast(r in kleene::re(
            8,
            64,
            8,
            0..16,
            crate::prop::symbol(),
        )) {
            let nfa = r.compile_atomic();
            let (dfa, _) = nfa.compile().atomize_nodes::<u64>();
            // let mut t = FlushOnDrop::new();

            let (opt, ..) = run::<fast::EGraph<_, _>, _, _, _>(&dfa);
            assert_eq!(opt, run_ref(&dfa).0);
        }
    }
}
