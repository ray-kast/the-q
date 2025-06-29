use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    hash::Hash,
};

use super::Dfa;
// TODO: switch to the optimized implementation once it...well, works
use crate::{
    dfa::Node,
    egraph::{self, prelude::*, trace::dot, EGraphTrace, ENode},
    free::Succ,
    partition_map::{Partition, PartitionMap},
    union_find::ClassId,
};

// NOTE: Ord is not exactly mathematically sound here, but in this case I need
// it to make this insertable into BTreeMaps
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Op<I, N, E, T> {
    Node {
        accept: Option<T>,
        edges: BTreeMap<Partition<I>, Option<E>>,
    },
    Impostor(N),
}

impl<I: fmt::Debug, N: fmt::Debug, E: fmt::Debug, T: fmt::Debug> dot::Format for Op<I, N, E, T> {
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

pub type Graph<I, N, E, T> = egraph::fast::EGraph<Op<I, N, E, T>, N>;
pub type ClassMap<N> = BTreeMap<N, ClassId<N>>;
pub type Output<I, N, E, T, G = Graph<I, N, E, T>> = (Dfa<I, ClassId<N>, E, T>, G, ClassMap<N>);

#[inline]
pub(super) fn run_default<
    I: Clone + Ord + Hash + Succ,
    N: Clone + Ord + Hash,
    E: Clone + Ord + Hash,
    T: Clone + Ord + Hash,
>(
    dfa: &Dfa<I, N, E, T>,
) -> Output<I, N, E, T> {
    run::<I, N, E, T, Graph<I, N, E, T>, ()>(dfa, Graph::default(), &mut ())
}

pub fn run<
    I: Clone + Ord + Hash + Succ,
    N: Clone + Ord + Hash,
    E: Clone + Ord,
    T: Clone + Ord + Hash,
    G: EGraphUpgradeTrace<FuncSymbol = Op<I, N, E, T>, Class = N>,
    R: EGraphTrace<Op<I, N, E, T>, N>,
>(
    dfa: &Dfa<I, N, E, T>,
    mut eg: G,
    t: &mut R,
) -> Output<I, N, E, T, G> {
    enum Command<N> {
        Explore(N),
        Add(N),
    }

    let mut stk = Vec::new();
    let mut classes = BTreeMap::new();
    let mut impostors = BTreeMap::new();

    stk.push(Command::Explore(dfa.start.clone()));

    while let Some(node) = stk.pop() {
        match node {
            Command::Explore(n) => {
                use std::collections::btree_map::Entry;

                match classes.entry(n.clone()) {
                    Entry::Occupied(_) => continue,
                    Entry::Vacant(v) => drop(v.insert(None)),
                }

                let states = &dfa.states[&n];
                stk.push(Command::Add(n));
                for (_, n) in states.0.values().cloned() {
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
                        edges: edges
                            .partitions()
                            .map(|(k, (e, _))| (k.to_owned(), e.clone()))
                            .collect(),
                    }
                    .into(),
                    edges
                        .values()
                        .cloned()
                        .map(|(_, n)| {
                            classes[&n].unwrap_or_else(|| {
                                *impostors.entry(n.clone()).or_insert_with(|| {
                                    eg.add(ENode::new(Op::Impostor(n).into(), [].into()))
                                        .unwrap()
                                })
                            })
                        })
                        .collect(),
                );

                let class = eg.add(enode.clone()).unwrap();
                assert!(classes.get_mut(&n).unwrap().replace(class).is_none());
            },
        }
    }

    let classes: BTreeMap<_, _> = classes.into_iter().map(|(k, v)| (k, v.unwrap())).collect();

    let mut wr = eg.write_trace(t);
    for (node, class) in impostors {
        wr.merge(classes[&node], class).unwrap();
    }
    drop(wr);

    let trap = eg.find(classes[&dfa.trap]).unwrap();
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
                c,
                PartitionMap::from_iter_with_default(
                    edges
                        .iter()
                        .enumerate()
                        .map(|(i, (e, t))| (e.clone(), (t.clone(), args[i]))),
                    (None, trap),
                ),
                accept.clone(),
            )
        });

    (
        Dfa::new(states, eg.find(classes[&dfa.start]).unwrap(), trap),
        eg,
        classes,
    )
}

#[cfg(test)]
mod test {
    use std::{fmt, hash::Hash};

    use foldhash::fast::FixedState;
    use proptest::prelude::*;

    use super::EGraphUpgrade;
    use crate::{
        egraph::{self, congr, fast, reference, test_tools::EGraphParts},
        free::Succ,
        partition_map::PartitionMap,
        re::kleene,
        union_find::ClassId,
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
        G: EGraphUpgrade<FuncSymbol = super::Op<I, N, E, T>, Class = N>,
        I: Clone + Ord + Hash + Succ + fmt::Debug,
        N: Clone + Ord + Hash + fmt::Debug,
        E: Clone + Ord,
        T: Clone + Ord + Hash + fmt::Debug,
    >(
        dfa: &super::Dfa<I, N, E, T>,
        egraph: G,
    ) -> super::Output<I, N, E, T, G> {
        super::run::<_, _, _, _, G, _>(dfa, egraph, &mut ())
    }

    #[expect(clippy::type_complexity, reason = "chill out man, it's a test helper")]
    fn run_ref<
        I: Clone + Ord + Hash + Succ,
        N: Clone + Ord + Hash,
        E: Clone + Ord + Hash,
        T: Clone + Ord + Hash,
    >(
        dfa: &super::Dfa<I, N, E, T>,
    ) -> super::Output<I, N, E, T, reference::EGraph<super::Op<I, N, E, T>, N>> {
        super::run(dfa, reference::EGraph::default(), &mut ())
    }

    fn assert_equiv<
        I: fmt::Debug + Clone + Ord,
        N: fmt::Debug + Ord,
        E: fmt::Debug + Clone + Ord,
        T: fmt::Debug + Clone + Ord,
        L: Into<EGraphParts<super::Op<I, N, E, T>, N>>,
        R: Into<EGraphParts<super::Op<I, N, E, T>, N>>,
    >(
        lhs: &super::Dfa<I, ClassId<N>, E, T>,
        lhs_graph: L,
        rhs: &super::Dfa<I, ClassId<N>, E, T>,
        rhs_graph: R,
    ) {
        let mapping = egraph::test_tools::assert_equiv(lhs_graph, rhs_graph);

        let trap = *mapping.image(&lhs.trap).unwrap();
        let lhs_mapped = super::Dfa {
            states: lhs
                .states
                .iter()
                .map(|(s, n)| {
                    (
                        *mapping.image(s).unwrap(),
                        super::Node(
                            PartitionMap::from_iter_with_default(
                                n.0.partitions().map(|(k, (e, v))| {
                                    (k.to_owned(), (e.clone(), *mapping.image(v).unwrap()))
                                }),
                                (None, trap),
                            ),
                            n.1.clone(),
                        ),
                    )
                })
                .collect(),
            start: *mapping.image(&lhs.start).unwrap(),
            trap,
        };

        let trap = *mapping.preimage(&rhs.trap).unwrap();
        let rhs_mapped = super::Dfa {
            states: rhs
                .states
                .iter()
                .map(|(s, n)| {
                    (
                        *mapping.preimage(s).unwrap(),
                        super::Node(
                            PartitionMap::from_iter_with_default(
                                n.0.partitions().map(|(k, (e, v))| {
                                    (k.to_owned(), (e.clone(), *mapping.preimage(v).unwrap()))
                                }),
                                (None, trap),
                            ),
                            n.1.clone(),
                        ),
                    )
                })
                .collect(),
            start: *mapping.preimage(&rhs.start).unwrap(),
            trap,
        };

        assert_eq!(*lhs, rhs_mapped);
        assert_eq!(lhs_mapped, *rhs);
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            // cases: 1 << 17,
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

            run::<reference::EGraph<_, _>, _, _, _, _>(&dfa, reference::EGraph::default());
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

            let (opt, graph, _) = run::<congr::EGraph<_, _>, _, _, _, _>(
                &dfa,
                congr::EGraph::default(),
            );
            let (ref_opt, ref_graph, _) = run_ref(&dfa);
            assert_equiv(&opt, graph, &ref_opt, ref_graph);
        }

        #[test]
        fn fast(
            r in kleene::re(
                8,
                64,
                8,
                0..16,
                crate::prop::symbol(),
            ),
            seed in any::<u64>(),
        ) {
            let nfa = r.compile_atomic();
            let (dfa, _) = nfa.compile().atomize_nodes::<u64>();
            // let mut t = FlushOnDrop::new();

            let (opt, graph, _) = run::<fast::EGraph<_, _, _>, _, _, _, _>(
                &dfa,
                fast::EGraph::with_hasher(FixedState::with_seed(seed)),
            );
            let (ref_opt, ref_graph, _) = run_ref(&dfa);
            assert_equiv(&opt, graph, &ref_opt, ref_graph);
        }
    }
}
