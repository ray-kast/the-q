use std::{collections::BTreeMap, fmt, hash::Hash};

use hashbrown::HashMap;

use super::Dfa;
use crate::{
    dfa::{collect_state_keys, collect_states, State, DFA_START},
    egraph::{self, prelude::*, trace::dot, EGraphTrace, ENode},
    free::Succ,
    partition_map::Partition,
    union_find::ClassId,
};

// NOTE: Ord is not exactly mathematically sound here, but in this case I need
// it to make this insertable into BTreeMaps
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Op<I, T, E> {
    Node {
        accept: T,
        edges: BTreeMap<Partition<I>, E>,
    },
    Impostor(usize),
}

impl<I: fmt::Debug, T: fmt::Debug, E: fmt::Debug> dot::Format for Op<I, T, E> {
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Node;

pub type Graph<I, T, E> = egraph::fast::EGraph<Op<I, T, E>, Node>;
pub type Output<I, T, E, G = Graph<I, T, E>> = (Dfa<I, T, E>, G, HashMap<usize, ClassId<Node>>);

#[inline]
pub(super) fn run_default<
    I: Clone + Ord + Hash + Succ,
    T: Clone + Ord + Hash,
    E: Clone + Ord + Hash,
>(
    dfa: &Dfa<I, T, E>,
) -> Output<I, T, E> {
    run::<I, T, E, Graph<I, T, E>, ()>(dfa, Graph::default(), &mut ())
}

pub fn run<
    I: Clone + Ord + Hash + Succ,
    T: Clone + Ord + Hash,
    E: Clone + Ord,
    G: EGraphUpgradeTrace<FuncSymbol = Op<I, T, E>, Class = Node>,
    R: EGraphTrace<Op<I, T, E>, Node>,
>(
    dfa: &Dfa<I, T, E>,
    mut eg: G,
    t: &mut R,
) -> Output<I, T, E, G> {
    enum Command<N> {
        Explore(N),
        Add(N),
    }

    let mut stk = Vec::new();
    let mut classes = BTreeMap::new();
    let mut impostors = BTreeMap::new();

    stk.push(Command::Explore(DFA_START));

    while let Some(node) = stk.pop() {
        match node {
            Command::Explore(n) => {
                use std::collections::btree_map::Entry;

                match classes.entry(n) {
                    Entry::Occupied(_) => continue,
                    Entry::Vacant(v) => drop(v.insert(None)),
                }

                let state = &dfa.0[n];
                stk.push(Command::Add(n));
                for (_, n) in state.0.values().cloned() {
                    if !classes.contains_key(&n) {
                        stk.push(Command::Explore(n));
                    }
                }
            },
            Command::Add(n) => {
                let State(ref edges, ref accept) = dfa.0[n];
                let enode = ENode::new(
                    Op::Node {
                        accept: accept.clone(),
                        edges: edges
                            .ranges()
                            .map(|(k, (e, _))| (k.to_owned(), e.clone()))
                            .collect(),
                    }
                    .into(),
                    edges
                        .values()
                        .cloned()
                        .map(|(_, n)| {
                            classes[&n].unwrap_or_else(|| {
                                *impostors.entry(n).or_insert_with(|| {
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

    let nodes = eg.class_nodes();
    let state_ids = collect_state_keys(
        nodes.keys().copied(),
        &eg.find(*classes.get(&DFA_START).unwrap()).unwrap(),
    );

    let states = collect_states(
        &state_ids,
        eg.class_nodes().into_iter().map(|(s, mut n)| {
            n.retain(|n| !matches!(n.op(), Op::Impostor(_)));
            assert!(n.len() == 1);

            let node = &n.into_iter().next().unwrap();
            let Op::Node { accept, edges } = node.op() else {
                unreachable!();
            };
            let args = node.args();

            (
                s,
                State(
                    edges
                        .iter()
                        .enumerate()
                        .map(|(i, (e, t))| {
                            (e.clone(), (t.clone(), *state_ids.get(&args[i]).unwrap()))
                        })
                        .collect(),
                    accept.clone(),
                ),
            )
        }),
    );

    drop(nodes);

    let n_ids = state_ids.len();
    let state_classes: HashMap<_, _> = state_ids.into_iter().map(|(k, v)| (v, k)).collect();

    debug_assert!(state_classes.len() == n_ids);

    (Dfa::new(states), eg, state_classes)
}

#[cfg(test)]
mod test {
    use std::{fmt, hash::Hash};

    use foldhash::fast::FixedState;
    use hashbrown::HashMap;
    use proptest::prelude::*;

    use super::EGraphUpgrade;
    use crate::{
        egraph::{self, congr, fast, reference, test_tools::EGraphParts},
        free::Succ,
        range_map::RangeMap,
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
        G: EGraphUpgrade<FuncSymbol = super::Op<I, T, E>, Class = super::Node>,
        I: Clone + Ord + Hash + Succ + fmt::Debug,
        T: Clone + Ord + Hash + fmt::Debug,
        E: Clone + Ord,
    >(
        dfa: &super::Dfa<I, T, E>,
        egraph: G,
    ) -> super::Output<I, T, E, G> {
        super::run::<_, _, _, G, _>(dfa, egraph, &mut ())
    }

    #[expect(clippy::type_complexity, reason = "chill out man, it's a test helper")]
    fn run_ref<I: Clone + Ord + Hash + Succ, T: Clone + Ord + Hash, E: Clone + Ord + Hash>(
        dfa: &super::Dfa<I, T, E>,
    ) -> super::Output<I, T, E, reference::EGraph<super::Op<I, T, E>, super::Node>> {
        super::run(dfa, reference::EGraph::default(), &mut ())
    }

    fn assert_equiv<
        I: fmt::Debug + Clone + Ord,
        T: fmt::Debug + Clone + Ord,
        E: fmt::Debug + Clone + Ord,
        L: Into<EGraphParts<super::Op<I, T, E>, super::Node>>,
        R: Into<EGraphParts<super::Op<I, T, E>, super::Node>>,
    >(
        lhs: super::Output<I, T, E, L>,
        rhs: super::Output<I, T, E, R>,
    ) {
        let (lhs, lhs_graph, lhs_cm) = lhs;
        let (rhs, rhs_graph, rhs_cm) = rhs;

        let mapping = egraph::test_tools::assert_equiv(lhs_graph, rhs_graph);

        let lhs: HashMap<_, (RangeMap<_, _>, _)> = lhs
            .into_states()
            .into_iter()
            .enumerate()
            .map(|(i, super::State(e, t))| {
                (
                    lhs_cm[&i],
                    (
                        e.into_ranges()
                            .map(|(i, (e, s))| (i, (e, lhs_cm[&s])))
                            .collect(),
                        t,
                    ),
                )
            })
            .collect();

        let rhs: HashMap<_, (RangeMap<_, _>, _)> = rhs
            .into_states()
            .into_iter()
            .enumerate()
            .map(|(i, super::State(e, t))| {
                (
                    rhs_cm[&i],
                    (
                        e.into_ranges()
                            .map(|(i, (e, s))| (i, (e, rhs_cm[&s])))
                            .collect(),
                        t,
                    ),
                )
            })
            .collect();

        let lhs_mapped: HashMap<_, _> = lhs
            .iter()
            .map(|(s, (e, t))| {
                (
                    *mapping.image(s).unwrap(),
                    (
                        e.ranges()
                            .map(|(k, (e, v))| {
                                (k.to_owned(), (e.clone(), *mapping.image(v).unwrap()))
                            })
                            .collect(),
                        t.clone(),
                    ),
                )
            })
            .collect();

        let rhs_mapped: HashMap<_, _> = rhs
            .iter()
            .map(|(s, (e, t))| {
                (
                    *mapping.preimage(s).unwrap(),
                    (
                        e.ranges()
                            .map(|(k, (e, v))| {
                                (k.to_owned(), (e.clone(), *mapping.preimage(v).unwrap()))
                            })
                            .collect(),
                        t.clone(),
                    ),
                )
            })
            .collect();

        assert_eq!(lhs, rhs_mapped);
        assert_eq!(lhs_mapped, rhs);
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
            let nfa = r.compile();
            let dfa = nfa.compile_moore();
            // let mut t = FlushOnDrop::new();

            run::<reference::EGraph<_, _>, _, _, _>(&dfa, reference::EGraph::default());
        }

        #[test]
        fn congr(r in kleene::re(
            8,
            64,
            8,
            0..16,
            crate::prop::symbol(),
        )) {
            let nfa = r.compile();
            let dfa = nfa.compile_moore();
            // let mut t = FlushOnDrop::new();

            let lhs = run::<congr::EGraph<_, _>, _, _, _>(
                &dfa,
                congr::EGraph::default(),
            );
            let rhs = run_ref(&dfa);
            assert_equiv(lhs, rhs);
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
            let nfa = r.compile();
            let dfa = nfa.compile_moore();
            // let mut t = FlushOnDrop::new();

            let lhs = run::<fast::EGraph<_, _, _>, _, _, _>(
                &dfa,
                fast::EGraph::with_hasher(FixedState::with_seed(seed)),
            );
            let rhs = run_ref(&dfa);
            assert_equiv(lhs, rhs);
        }
    }
}
