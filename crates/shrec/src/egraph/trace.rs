use std::{collections::BTreeMap, fmt};

use super::ENode;
use crate::union_find::ClassId;

pub trait EGraphTrace<F: ?Sized, C: ?Sized> {
    type Graph: SnapshotEGraph<F, C>;

    fn graph<G: FnOnce(&mut Self::Graph)>(&mut self, f: G);

    fn pop_graph(&mut self);

    fn hl_class(&mut self, root: ClassId<C>);

    fn hl_merges<I: IntoIterator<Item = (ClassId<C>, ClassId<C>)>>(&mut self, it: I);
}

pub trait SnapshotEGraph<F: ?Sized, C: ?Sized> {
    type EquivClass<'a>: SnapshotEquivClass<
        F,
        C,
        Id = Self::ClassId,
        Node: SnapshotNode<F, C, Id = Self::NodeId>,
    >
    where Self: 'a;
    type ClassId: Clone;
    type NodeId: Clone;

    fn equiv_class(&mut self, root: ClassId<C>) -> Self::EquivClass<'_>;

    fn edge(&mut self, node: &Self::NodeId, klass: &Self::ClassId, op: &F, arg_idx: usize);
}

pub trait SnapshotEquivClass<F: ?Sized, C: ?Sized> {
    type Id: Clone;
    type Node: SnapshotNode<F, C>;

    fn id(&self) -> &Self::Id;

    fn node(&mut self, node: &ENode<F, C>) -> Self::Node;
}

pub trait SnapshotNode<F: ?Sized, C: ?Sized> {
    type Id: Clone;

    fn id(&self) -> &Self::Id;
}

impl<F: ?Sized, C: ?Sized> EGraphTrace<F, C> for () {
    type Graph = ();

    #[inline]
    fn graph<G: FnOnce(&mut Self::Graph)>(&mut self, _: G) {}

    #[inline]
    fn pop_graph(&mut self) {}

    #[inline]
    fn hl_class(&mut self, _: ClassId<C>) {}

    #[inline]
    fn hl_merges<I: IntoIterator<Item = (ClassId<C>, ClassId<C>)>>(&mut self, _: I) {}
}

impl<F: ?Sized, C: ?Sized> SnapshotEGraph<F, C> for () {
    type ClassId = ();
    type EquivClass<'a> = ();
    type NodeId = ();

    #[inline]
    fn equiv_class(&mut self, _: ClassId<C>) -> Self::EquivClass<'_> {}

    #[inline]
    fn edge(&mut self, (): &Self::NodeId, (): &Self::ClassId, _: &F, _: usize) {}
}

impl<F: ?Sized, C: ?Sized> SnapshotEquivClass<F, C> for () {
    type Id = ();
    type Node = ();

    #[inline]
    fn id(&self) -> &Self::Id { self }

    #[inline]
    fn node(&mut self, _: &ENode<F, C>) -> Self::Node {}
}

impl<F: ?Sized, C: ?Sized> SnapshotNode<F, C> for () {
    type Id = ();

    #[inline]
    fn id(&self) -> &Self::Id { self }
}

pub mod dot {
    use std::{borrow::Cow, fmt, marker::PhantomData};

    use super::{SnapshotEGraph, SnapshotEquivClass, SnapshotNode};
    use crate::{
        dot::{Graph as DotGraph, GraphType},
        egraph::ENode,
        union_find::ClassId,
    };

    pub trait Formatter<F: ?Sized>: Copy {
        fn fmt_node(&self, op: &F, f: &mut fmt::Formatter) -> fmt::Result;

        fn fmt_edge(&self, op: &F, idx: usize, f: &mut fmt::Formatter) -> fmt::Result;
    }

    pub trait Format {
        fn fmt_node(&self, f: &mut fmt::Formatter) -> fmt::Result;

        fn fmt_edge(&self, idx: usize, f: &mut fmt::Formatter) -> fmt::Result;
    }

    #[derive(Debug, Clone, Copy)]
    pub struct DebugFormatter;

    impl<F: ?Sized + fmt::Debug> Formatter<F> for DebugFormatter {
        #[inline]
        fn fmt_node(&self, op: &F, f: &mut fmt::Formatter) -> fmt::Result { fmt::Debug::fmt(op, f) }

        #[inline]
        fn fmt_edge(&self, _: &F, _: usize, _: &mut fmt::Formatter) -> fmt::Result { Ok(()) }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct RichFormatter;

    impl<F: ?Sized + Format> Formatter<F> for RichFormatter {
        #[inline]
        fn fmt_node(&self, op: &F, f: &mut fmt::Formatter) -> fmt::Result { op.fmt_node(f) }

        #[inline]
        fn fmt_edge(&self, op: &F, idx: usize, f: &mut fmt::Formatter) -> fmt::Result {
            op.fmt_edge(idx, f)
        }
    }

    pub struct ClosureFormatter<F, N, E>(N, E, PhantomData<fn(&F)>);

    impl<F, N: fmt::Debug, E: fmt::Debug> fmt::Debug for ClosureFormatter<F, N, E> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_tuple("ClosureFormatter")
                .field(&self.0)
                .field(&self.1)
                .finish()
        }
    }

    impl<F, N: Clone, E: Clone> Clone for ClosureFormatter<F, N, E> {
        fn clone(&self) -> Self { Self(self.0.clone(), self.1.clone(), PhantomData) }
    }

    impl<F, N: Copy, E: Copy> Copy for ClosureFormatter<F, N, E> {}

    impl<
            F,
            N: Fn(&F, &mut fmt::Formatter) -> fmt::Result + Copy,
            E: Fn(&F, usize, &mut fmt::Formatter) -> fmt::Result + Copy,
        > ClosureFormatter<F, N, E>
    {
        #[inline]
        #[must_use]
        pub const fn new(fmt_node: N, fmt_edge: E) -> Self { Self(fmt_node, fmt_edge, PhantomData) }
    }

    impl<
            F,
            N: Fn(&F, &mut fmt::Formatter) -> fmt::Result + Copy,
            E: Fn(&F, usize, &mut fmt::Formatter) -> fmt::Result + Copy,
        > Formatter<F> for ClosureFormatter<F, N, E>
    {
        #[inline]
        fn fmt_node(&self, op: &F, f: &mut fmt::Formatter) -> fmt::Result { self.0(op, f) }

        #[inline]
        fn fmt_edge(&self, op: &F, idx: usize, f: &mut fmt::Formatter) -> fmt::Result {
            self.1(op, idx, f)
        }
    }

    #[derive(Debug)]
    pub struct Snapshot {
        pub graph: DotGraph<'static>,
    }

    #[derive(Debug)]
    pub struct Graph<M>(pub(super) DotGraph<'static>, M);
    #[derive(Debug)]
    pub struct EquivClass<'a, M>(Cow<'static, str>, &'a mut DotGraph<'static>, M);
    #[derive(Debug)]
    pub struct Node(Cow<'static, str>);

    impl<M> Graph<M> {
        #[inline]
        pub fn new(f: M) -> Self { Self(DotGraph::new(GraphType::Directed, None), f) }
    }

    impl<F: ?Sized, C: ?Sized, M: Formatter<F>> SnapshotEGraph<F, C> for Graph<M> {
        type ClassId = Cow<'static, str>;
        type EquivClass<'a>
            = EquivClass<'a, M>
        where M: 'a;
        type NodeId = Cow<'static, str>;

        fn equiv_class(&mut self, root: ClassId<C>) -> Self::EquivClass<'_> {
            let sg = self.0.subgraph(format!("cluster_{}", root.id()).into());
            sg.style("filled".into());
            sg.label(format!("{}", root.id()).into());

            let rep_id = Cow::from(format!("class_{}", root.id()));
            let class_node = sg.node(rep_id.clone());
            class_node.style("invis".into());
            class_node.shape("point".into());
            class_node.label("".into());

            EquivClass(rep_id, sg, self.1)
        }

        fn edge(&mut self, node: &Self::NodeId, klass: &Self::ClassId, op: &F, arg_idx: usize) {
            struct Fmt<'a, F: ?Sized, M: ?Sized>(&'a F, usize, &'a M);

            impl<F: ?Sized, M: Formatter<F>> fmt::Display for Fmt<'_, F, M> {
                #[inline]
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    self.2.fmt_edge(self.0, self.1, f)
                }
            }

            let edge = self.0.edge(node.clone(), klass.clone());

            let s = format!("{}", Fmt(op, arg_idx, &self.1));
            if !s.is_empty() {
                edge.label(s.into());
            }
        }
    }

    impl<F: ?Sized, C: ?Sized, M: Formatter<F>> SnapshotEquivClass<F, C> for EquivClass<'_, M> {
        type Id = Cow<'static, str>;
        type Node = Node;

        #[inline]
        fn id(&self) -> &Self::Id { &self.0 }

        fn node(&mut self, node: &ENode<F, C>) -> Self::Node {
            struct Fmt<'a, F: ?Sized, M: ?Sized>(&'a F, &'a M);

            impl<F: ?Sized, M: Formatter<F>> fmt::Display for Fmt<'_, F, M> {
                #[inline]
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    self.1.fmt_node(self.0, f)
                }
            }

            let mut label = format!("{}(", Fmt(node.op(), &self.2));
            for (i, arg) in node.args().iter().enumerate() {
                if i > 0 {
                    label.push(',');
                }

                label.push_str(&arg.id().to_string());
            }
            label.push(')');
            let id = Cow::from(format!("{}_node_{label}", self.0));

            let node = self.1.node(id.clone());
            node.label(label.into());
            let edge = self.1.edge(self.0.clone(), id.clone());
            edge.style("invis".into());

            Node(id)
        }
    }

    impl<F: ?Sized, C: ?Sized> SnapshotNode<F, C> for Node {
        type Id = Cow<'static, str>;

        #[inline]
        fn id(&self) -> &Self::Id { &self.0 }
    }
}

#[derive(Debug)]
pub struct DotTracer<M>(Vec<dot::Snapshot>, M);

impl Default for DotTracer<dot::DebugFormatter> {
    #[inline]
    fn default() -> Self { Self::debug() }
}

impl Default for DotTracer<dot::RichFormatter> {
    #[inline]
    fn default() -> Self { Self::rich() }
}

impl<
        F,
        N: Fn(&F, &mut fmt::Formatter) -> fmt::Result + Copy,
        E: Fn(&F, usize, &mut fmt::Formatter) -> fmt::Result + Copy,
    > DotTracer<dot::ClosureFormatter<F, N, E>>
{
    #[must_use]
    pub fn new(fmt_node: N, fmt_edge: E) -> Self {
        Self(vec![], dot::ClosureFormatter::new(fmt_node, fmt_edge))
    }
}

impl<M> DotTracer<M> {
    pub fn flush<F: FnMut(dot::Snapshot)>(&mut self, mut f: F) {
        for snap in self.0.drain(..) {
            f(snap);
        }
    }
}

impl DotTracer<dot::DebugFormatter> {
    #[inline]
    #[must_use]
    pub fn debug() -> Self { Self(vec![], dot::DebugFormatter) }
}

impl DotTracer<dot::RichFormatter> {
    #[inline]
    #[must_use]
    pub fn rich() -> Self { Self(vec![], dot::RichFormatter) }
}

impl<F: ?Sized, C: ?Sized, M: dot::Formatter<F>> EGraphTrace<F, C> for DotTracer<M> {
    type Graph = dot::Graph<M>;

    fn graph<G: FnOnce(&mut Self::Graph)>(&mut self, f: G) {
        let mut graph = dot::Graph::new(self.1);
        f(&mut graph);
        self.0.push(dot::Snapshot { graph: graph.0 });
    }

    #[inline]
    fn pop_graph(&mut self) { self.0.pop(); }

    fn hl_class(&mut self, root: ClassId<C>) {
        self.0
            .last_mut()
            .unwrap()
            .graph
            .subgraph(format!("cluster_{}", root.id()).into())
            .penwidth("4.0".into());
    }

    fn hl_merges<I: IntoIterator<Item = (ClassId<C>, ClassId<C>)>>(&mut self, it: I) {
        let graph = &mut self.0.last_mut().unwrap().graph;

        for (a, b) in it {
            let edge = graph.edge(
                format!("class_{}", a.id()).into(),
                format!("class_{}", b.id()).into(),
            );

            edge.style("dashed".into());
            edge.constraint("false".into());
        }
    }
}

pub fn dot_graph<
    'a,
    'b,
    F: Ord + 'b,
    C: 'b,
    M: dot::Formatter<F>,
    IR: IntoIterator<Item = (ClassId<C>, IN)> + Clone,
    IN: IntoIterator<Item = &'b crate::egraph::ENode<F, C>>,
>(
    f: M,
    roots: IR,
) -> crate::dot::Graph<'static> {
    let mut graph = dot::Graph::new(f);
    snapshot_graph(&mut graph, roots);
    graph.0
}

pub fn snapshot_graph<
    'a,
    'b,
    F: Ord + 'b,
    C: 'b,
    S: SnapshotEGraph<F, C>,
    IR: IntoIterator<Item = (ClassId<C>, IN)> + Clone,
    IN: IntoIterator<Item = &'b crate::egraph::ENode<F, C>>,
>(
    graph: &mut S,
    roots: IR,
) {
    let mut class_reps = BTreeMap::new();
    let mut node_ids = BTreeMap::new();

    for (root, enodes) in roots.clone() {
        let mut cls = graph.equiv_class(root);
        assert!(class_reps.insert(root, cls.id().clone()).is_none());

        for enode in enodes {
            let node = cls.node(enode);
            assert!(
                node_ids
                    .insert(
                        enode,
                        <<S::EquivClass<'_> as SnapshotEquivClass<F, C>>::Node as SnapshotNode<
                            F,
                            C,
                        >>::id(&node)
                        .clone()
                    )
                    .is_none()
            );
        }
    }

    for (_, enodes) in roots {
        for enode in enodes {
            for (i, &edge) in enode.args().iter().enumerate() {
                let klass = class_reps
                    .entry(edge)
                    .or_insert_with(|| graph.equiv_class(edge).id().clone());

                graph.edge(node_ids.get(enode).unwrap(), klass, enode.op(), i);
            }
        }
    }
}
