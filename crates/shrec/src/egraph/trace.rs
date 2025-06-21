use std::{collections::BTreeMap, fmt, mem};

use super::ENode;
use crate::union_find::ClassId;

pub trait EGraphTrace<F: ?Sized, C: ?Sized> {
    type Graph: SnapshotEGraph<F, C>;

    fn graph<G: FnOnce(&mut Self::Graph)>(&mut self, f: G);

    fn hl_class(&mut self, root: ClassId<C>);

    fn hl_classes<I: IntoIterator<Item = ClassId<C>>>(&mut self, it: I);

    fn hl_merges<I: IntoIterator<Item = (ClassId<C>, ClassId<C>)>>(&mut self, it: I);
}

impl<T: EGraphTrace<F, C>, F, C> EGraphTrace<F, C> for &mut T {
    type Graph = T::Graph;

    #[inline]
    fn graph<G: FnOnce(&mut Self::Graph)>(&mut self, f: G) { T::graph(self, f) }

    #[inline]
    fn hl_class(&mut self, root: ClassId<C>) { T::hl_class(self, root) }

    #[inline]
    fn hl_classes<I: IntoIterator<Item = ClassId<C>>>(&mut self, it: I) { T::hl_classes(self, it) }

    #[inline]
    fn hl_merges<I: IntoIterator<Item = (ClassId<C>, ClassId<C>)>>(&mut self, it: I) {
        T::hl_merges(self, it);
    }
}

pub trait SnapshotEGraph<F: ?Sized, C: ?Sized> {
    type UnionFind<'a>: SnapshotUnionFind<GraphClassId = Self::ClassId, NodeId = Self::NodeId>
    where Self: 'a;
    type EquivClass<'a>: SnapshotEquivClass<
        F,
        C,
        Id = Self::ClassId,
        Node: SnapshotNode<Id = Self::NodeId>,
    >
    where Self: 'a;
    type ClassId: Clone;
    type NodeId: Clone;

    fn union_find(&mut self, name: &'static str) -> Self::UnionFind<'_>;

    fn equiv_class(&mut self, root: ClassId<C>) -> Self::EquivClass<'_>;

    fn edge(&mut self, node: &Self::NodeId, class: &Self::ClassId, op: &F, arg_idx: usize);

    fn parent_edge(
        &mut self,
        class: &Self::ClassId,
        parent: &Self::NodeId,
        parent_class: Option<&Self::ClassId>,
        label: Option<&str>,
    );
}

pub trait SnapshotEquivClass<F: ?Sized, C: ?Sized> {
    type Id: Clone;
    type Node: SnapshotNode;

    fn id(&self) -> &Self::Id;

    fn node(&mut self, node: &ENode<F, C>) -> Self::Node;
}

pub trait SnapshotNode {
    type Id: Clone;

    fn id(&self) -> &Self::Id;
}

pub trait SnapshotUnionFind {
    type Class: SnapshotUfClass<Id = Self::ClassId>;
    type GraphClassId: Clone;
    type ClassId: Clone;
    type NodeId: Clone;

    fn class(&mut self, fmt: fmt::Arguments) -> Self::Class;

    fn parent(&mut self, class: &Self::ClassId, parent: &Self::ClassId);

    fn link_from_graph_class(&mut self, from: &Self::GraphClassId, to: &Self::ClassId);

    fn link_to_node(&mut self, class: &Self::ClassId, node: &Self::NodeId);
}

pub trait SnapshotUfClass {
    type Id;

    fn id(&self) -> &Self::Id;
}

impl<F: ?Sized, C: ?Sized> EGraphTrace<F, C> for () {
    type Graph = ();

    #[inline]
    fn graph<G: FnOnce(&mut Self::Graph)>(&mut self, _: G) {}

    #[inline]
    fn hl_class(&mut self, _: ClassId<C>) {}

    #[inline]
    fn hl_classes<I: IntoIterator<Item = ClassId<C>>>(&mut self, _: I) {}

    #[inline]
    fn hl_merges<I: IntoIterator<Item = (ClassId<C>, ClassId<C>)>>(&mut self, _: I) {}
}

impl<F: ?Sized, C: ?Sized> SnapshotEGraph<F, C> for () {
    type ClassId = ();
    type EquivClass<'a> = ();
    type NodeId = ();
    type UnionFind<'a> = ();

    #[inline]
    fn union_find(&mut self, _: &str) -> Self::UnionFind<'_> {}

    #[inline]
    fn equiv_class(&mut self, _: ClassId<C>) -> Self::EquivClass<'_> {}

    #[inline]
    fn edge(&mut self, (): &Self::NodeId, (): &Self::ClassId, _: &F, _: usize) {}

    #[inline]
    fn parent_edge(
        &mut self,
        (): &Self::ClassId,
        (): &Self::NodeId,
        parent_class: Option<&Self::ClassId>,
        _: Option<&str>,
    ) {
        let () = parent_class.unwrap_or(&());
    }
}

impl<F: ?Sized, C: ?Sized> SnapshotEquivClass<F, C> for () {
    type Id = ();
    type Node = ();

    #[inline]
    fn id(&self) -> &Self::Id { self }

    #[inline]
    fn node(&mut self, _: &ENode<F, C>) -> Self::Node {}
}

impl SnapshotNode for () {
    type Id = ();

    #[inline]
    fn id(&self) -> &Self::Id { self }
}

impl SnapshotUnionFind for () {
    type Class = ();
    type ClassId = ();
    type GraphClassId = ();
    type NodeId = ();

    #[inline]
    fn class(&mut self, _: fmt::Arguments) -> Self::Class {}

    #[inline]
    fn parent(&mut self, (): &Self::ClassId, (): &Self::ClassId) {}

    #[inline]
    fn link_from_graph_class(&mut self, (): &Self::GraphClassId, (): &Self::ClassId) {}

    #[inline]
    fn link_to_node(&mut self, (): &Self::ClassId, (): &Self::NodeId) {}
}

impl SnapshotUfClass for () {
    type Id = ();

    #[inline]
    fn id(&self) -> &Self::Id { self }
}

pub mod dot {
    use std::{borrow::Cow, fmt, marker::PhantomData};

    use super::{
        SnapshotEGraph, SnapshotEquivClass, SnapshotNode, SnapshotUfClass, SnapshotUnionFind,
    };
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

    #[derive(Debug)]
    pub struct UnionFind<'a, M>(Cow<'static, str>, &'a mut DotGraph<'static>, M);
    #[derive(Debug)]
    pub struct UfClass(Cow<'static, str>);

    impl<M> Graph<M> {
        #[inline]
        pub fn new(f: M) -> Self { Self(DotGraph::new(GraphType::Directed), f) }

        pub fn from_parts(graph: DotGraph<'static>, f: M) -> Self { Self(graph, f) }
    }

    impl<F: ?Sized, C: ?Sized, M: Formatter<F>> SnapshotEGraph<F, C> for Graph<M> {
        type ClassId = Cow<'static, str>;
        type EquivClass<'a>
            = EquivClass<'a, M>
        where M: 'a;
        type NodeId = Cow<'static, str>;
        type UnionFind<'a>
            = UnionFind<'a, M>
        where M: 'a;

        fn union_find(&mut self, name: &'static str) -> Self::UnionFind<'_> {
            let id = Cow::from(format!("cluster_uf_{name}"));
            let sg = self.0.subgraph(id.clone());
            sg.style("solid");
            sg.label(name);

            UnionFind(id, sg, self.1)
        }

        fn equiv_class(&mut self, root: ClassId<C>) -> Self::EquivClass<'_> {
            let sg = self.0.subgraph(format!("cluster_{}", root.id()));
            sg.style("filled");
            sg.label(format!("{}", root.id()));

            let rep_id = Cow::from(format!("class_{}", root.id()));
            let class_node = sg.node(rep_id.clone());
            class_node.style("invis");
            class_node.shape("point");
            class_node.label("");

            EquivClass(rep_id, sg, self.1)
        }

        fn edge(&mut self, node: &Self::NodeId, class: &Self::ClassId, op: &F, arg_idx: usize) {
            struct Fmt<'a, F: ?Sized, M: ?Sized>(&'a F, usize, &'a M);

            impl<F: ?Sized, M: Formatter<F>> fmt::Display for Fmt<'_, F, M> {
                #[inline]
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    self.2.fmt_edge(self.0, self.1, f)
                }
            }

            let edge = self.0.edge(node.clone(), class.clone());

            let s = format!("{}", Fmt(op, arg_idx, &self.1));
            if !s.is_empty() {
                edge.label(s);
            }
        }

        fn parent_edge(
            &mut self,
            class: &Self::ClassId,
            parent: &Self::NodeId,
            parent_class: Option<&Self::ClassId>,
            label: Option<&str>,
        ) {
            let parent_edge = self.0.edge(class.clone(), parent.clone());

            parent_edge.constraint("false");
            parent_edge.color("blue");
            parent_edge.style("dashed");

            if let Some(label) = label {
                parent_edge.label(label.to_owned());
            }

            if let Some(parent_class) = parent_class {
                let class_edge = self.0.edge(parent.clone(), parent_class.clone());

                class_edge.constraint("false");
                class_edge.color("blue");
                class_edge.style("dotted");

                if let Some(label) = label {
                    class_edge.label(label.to_owned());
                }
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
            node.label(label);
            node.edge_ordering("out");
            let edge = self.1.edge(self.0.clone(), id.clone());
            edge.style("invis");

            Node(id)
        }
    }

    impl SnapshotNode for Node {
        type Id = Cow<'static, str>;

        #[inline]
        fn id(&self) -> &Self::Id { &self.0 }
    }

    impl<M> SnapshotUnionFind for UnionFind<'_, M> {
        type Class = UfClass;
        type ClassId = Cow<'static, str>;
        type GraphClassId = Cow<'static, str>;
        type NodeId = Cow<'static, str>;

        fn class(&mut self, fmt: fmt::Arguments) -> Self::Class {
            let label = fmt.to_string();
            let id = Cow::from(format!("{}_node_{label}", self.0));
            let node = self.1.node(id.clone());
            node.label(label);

            UfClass(id)
        }

        fn parent(&mut self, class: &Self::ClassId, parent: &Self::ClassId) {
            self.1.edge(class.clone(), parent.clone());
        }

        fn link_from_graph_class(&mut self, from: &Self::GraphClassId, to: &Self::ClassId) {
            let edge = self.1.edge(from.clone(), to.clone());
            edge.style("dashed");
            edge.constraint("false");
            edge.concentrate("false");
            edge.color("red");
        }

        fn link_to_node(&mut self, class: &Self::ClassId, node: &Self::NodeId) {
            let edge = self.1.edge(class.clone(), node.clone());
            edge.style("dashed");
            edge.color("blue");
        }
    }

    impl SnapshotUfClass for UfClass {
        type Id = Cow<'static, str>;

        #[inline]
        fn id(&self) -> &Self::Id { &self.0 }
    }
}

#[derive(Debug)]
pub struct DotTracer<M, F>(Option<dot::Snapshot>, M, F);

impl<
        F,
        N: Fn(&F, &mut fmt::Formatter) -> fmt::Result + Copy,
        E: Fn(&F, usize, &mut fmt::Formatter) -> fmt::Result + Copy,
        G,
    > DotTracer<dot::ClosureFormatter<F, N, E>, G>
{
    #[must_use]
    pub fn new(fmt_node: N, fmt_edge: E, f: G) -> Self {
        Self(None, dot::ClosureFormatter::new(fmt_node, fmt_edge), f)
    }
}

impl<M, F: FnMut(dot::Snapshot)> DotTracer<M, F> {
    pub fn flush(&mut self) {
        if let Some(snap) = self.0.take() {
            self.2(snap);
        }
    }
}

impl<F> DotTracer<dot::DebugFormatter, F> {
    #[inline]
    #[must_use]
    pub fn debug(f: F) -> Self { Self(None, dot::DebugFormatter, f) }
}

impl<F> DotTracer<dot::RichFormatter, F> {
    #[inline]
    #[must_use]
    pub fn rich(f: F) -> Self { Self(None, dot::RichFormatter, f) }
}

impl<F: ?Sized, C: ?Sized, M: dot::Formatter<F>, G: FnMut(dot::Snapshot)> EGraphTrace<F, C>
    for DotTracer<M, G>
{
    type Graph = dot::Graph<M>;

    #[inline]
    fn graph<H: FnOnce(&mut Self::Graph)>(&mut self, f: H) {
        let mut graph = dot::Graph::new(self.1);
        f(&mut graph);
        if let Some(snap) = mem::replace(&mut self.0, Some(dot::Snapshot { graph: graph.0 })) {
            self.2(snap);
        }
    }

    fn hl_class(&mut self, root: ClassId<C>) {
        self.0
            .as_mut()
            .unwrap()
            .graph
            .subgraph(format!("cluster_{}", root.id()))
            .penwidth("4.0");
    }

    #[inline]
    fn hl_classes<I: IntoIterator<Item = ClassId<C>>>(&mut self, it: I) {
        it.into_iter().for_each(|c| self.hl_class(c));
    }

    fn hl_merges<I: IntoIterator<Item = (ClassId<C>, ClassId<C>)>>(&mut self, it: I) {
        let graph = &mut self.0.as_mut().unwrap().graph;

        for (a, b) in it {
            let edge = graph.edge(format!("class_{}", a.id()), format!("class_{}", b.id()));

            edge.style("dashed");
            edge.constraint("false");
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

#[derive(Debug)]
pub struct SnapshotGraphNodes<'a, F, C, IC, IN> {
    pub class_reps: BTreeMap<ClassId<C>, IC>,
    pub node_ids: BTreeMap<&'a ENode<F, C>, IN>,
}

pub fn snapshot_graph<
    'a,
    'b,
    F: Ord + 'b,
    C: 'b,
    S: SnapshotEGraph<F, C>,
    IR: IntoIterator<Item = (ClassId<C>, IN)> + Clone,
    IN: IntoIterator<Item = &'b ENode<F, C>>,
>(
    graph: &mut S,
    roots: IR,
) -> SnapshotGraphNodes<'b, F, C, S::ClassId, S::NodeId> {
    let mut class_reps = BTreeMap::new();
    let mut node_ids = BTreeMap::new();

    for (root, enodes) in roots.clone() {
        let mut cls = graph.equiv_class(root);
        assert!(class_reps.insert(root, cls.id().clone()).is_none());

        for enode in enodes {
            let node = cls.node(enode);
            assert!(node_ids.insert(enode, node.id().clone()).is_none());
        }
    }

    for (_, enodes) in roots {
        for enode in enodes {
            for (i, &edge) in enode.args().iter().enumerate() {
                let class = class_reps
                    .entry(edge)
                    .or_insert_with(|| graph.equiv_class(edge).id().clone());

                graph.edge(node_ids.get(enode).unwrap(), class, enode.op(), i);
            }
        }
    }

    SnapshotGraphNodes {
        class_reps,
        node_ids,
    }
}
