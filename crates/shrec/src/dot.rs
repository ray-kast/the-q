use std::{
    borrow::Cow,
    collections::BTreeMap,
    fmt::{self, Display},
};

use indexmap::IndexMap;

macro_rules! attr {
    ($id:ident, $name:literal) => {
        pub fn $id<S: Into<Cow<'a, str>>>(&mut self, $id: S) {
            self.attrs.insert($name, $id.into());
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GraphType {
    Undirected,
    Directed,
}

impl Display for GraphType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Undirected => "graph",
            Self::Directed => "digraph",
        })
    }
}

#[derive(Debug)]
enum NodeLike<'a> {
    Node(Node<'a>),
    Subgraph(Graph<'a>),
}

#[derive(Debug)]
pub struct Graph<'a> {
    ty: Option<GraphType>,
    id: Option<Cow<'a, str>>,
    attrs: BTreeMap<&'static str, Cow<'a, str>>,
    nodes: IndexMap<Cow<'a, str>, NodeLike<'a>>,
    edges: IndexMap<(Cow<'a, str>, Cow<'a, str>), Vec<Edge<'a>>>,
}

impl<'a> Graph<'a> {
    attr!(style, "style");

    attr!(label, "label");

    attr!(penwidth, "penwidth");

    #[must_use]
    #[inline]
    pub fn new(ty: GraphType) -> Self { Self::new_impl(Some(ty), None) }

    #[must_use]
    #[inline]
    pub fn new_with_id<S: Into<Cow<'a, str>>>(ty: GraphType, id: S) -> Self {
        Self::new_impl(Some(ty), Some(id.into()))
    }

    fn new_impl(ty: Option<GraphType>, id: Option<Cow<'a, str>>) -> Self {
        Self {
            ty,
            id,
            attrs: BTreeMap::new(),
            nodes: IndexMap::new(),
            edges: IndexMap::new(),
        }
    }

    #[inline]
    pub fn node<S: Into<Cow<'a, str>>>(&mut self, id: S) -> &mut Node<'a> {
        use indexmap::map::Entry;

        let entry = match self.nodes.entry(id.into()) {
            Entry::Vacant(v) => v.insert(NodeLike::Node(Node::default())),
            Entry::Occupied(e) => {
                assert!(
                    matches!(e.get(), NodeLike::Node(_)),
                    "Invalid node-like {:?}, expected a node",
                    e.key()
                );
                e.into_mut()
            },
        };

        if let NodeLike::Node(n) = entry {
            n
        } else {
            unreachable!()
        }
    }

    #[inline]
    pub fn subgraph<S: Into<Cow<'a, str>>>(&mut self, id: S) -> &mut Graph<'a> {
        use indexmap::map::Entry;

        let entry = match self.nodes.entry(id.into()) {
            Entry::Vacant(v) => v.insert(NodeLike::Subgraph(Graph::new_impl(None, None))),
            Entry::Occupied(e) => {
                assert!(
                    matches!(e.get(), NodeLike::Subgraph(_)),
                    "Invalid node-like {:?}, expected a subgraph",
                    e.key()
                );
                e.into_mut()
            },
        };

        if let NodeLike::Subgraph(g) = entry {
            g
        } else {
            unreachable!()
        }
    }

    #[inline]
    pub fn edge<L: Into<Cow<'a, str>>, R: Into<Cow<'a, str>>>(
        &mut self,
        l: L,
        r: R,
    ) -> &mut Edge<'a> {
        let l = l.into();
        let r = r.into();
        self.node(l.clone());
        self.node(r.clone());
        let edges = self.edges.entry((l, r)).or_default();
        edges.push(Edge::default());
        edges.last_mut().unwrap_or_else(|| unreachable!())
    }

    pub(crate) fn state_machine<
        I,
        S,
        E,
        T,
        IN: IntoIterator<Item = (S, IE, Option<T>)>,
        IE: IntoIterator<Item = (I, IO)>,
        IO: IntoIterator<Item = (Option<E>, S)>,
        G: FnMut(&S) -> u32,
        FI: Fn(I) -> Cow<'a, str>,
        FS: Fn(S) -> Cow<'a, str>,
        FE: Fn(E) -> Option<Cow<'a, str>>,
        FT: Fn(T) -> Option<Cow<'a, str>>,
    >(
        nodes: IN,
        start: &S,
        mut get_id: G,
        fmt_input: FI,
        fmt_state: FS,
        fmt_edge: FE,
        fmt_tok: FT,
    ) -> Self {
        let mut graph = Self::new(GraphType::Directed);

        for (state, edges, accept) in nodes {
            let id = Cow::from(get_id(&state).to_string());
            let node = graph.node(id.clone());

            let mut label = fmt_state(state);
            if let Some(tok) = accept {
                if let Some(tok) = fmt_tok(tok) {
                    label = format!("{label}:{tok}").into();
                }

                node.border_count("2");
            }

            node.label(label);

            for (input, outputs) in edges {
                let input = fmt_input(input);

                for (out, next_state) in outputs {
                    let edge = graph.edge(id.clone(), get_id(&next_state).to_string());

                    let label = if let Some(out) = out.and_then(&fmt_edge) {
                        format!("{input}/{out}").into()
                    } else {
                        input.clone()
                    };

                    edge.label(label);
                }
            }
        }

        let start_id = Cow::from("_start");
        let start_node = graph.node(start_id.clone());
        start_node.style("invis");
        start_node.shape("point");
        start_node.label("");
        graph.edge(start_id, get_id(start).to_string());

        graph
    }
}

#[derive(Default)]
struct AttrState {
    any: bool,
}

impl AttrState {
    fn write_one(
        &mut self,
        f: &mut fmt::Formatter,
        key: &'static str,
        val: impl FnOnce(&mut fmt::Formatter) -> fmt::Result,
    ) -> fmt::Result {
        f.write_str(if self.any {
            ","
        } else {
            self.any = true;
            "["
        })?;

        f.write_str(key)?;
        f.write_str("=")?;
        val(f)
    }

    fn finish(self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.any {
            f.write_str("]")
        } else {
            Ok(())
        }
    }
}

impl Graph<'_> {
    fn fmt_impl(&self, f: &mut fmt::Formatter, sub_id: Option<(GraphType, &str)>) -> fmt::Result {
        let Self {
            ty,
            id,
            attrs,
            nodes,
            edges,
        } = self;

        let ty = match (ty, sub_id) {
            (ty, None) => {
                let ty = ty.unwrap();
                write!(f, "{ty}")?;

                if let Some(id) = id {
                    write!(f, " {id:?}")?;
                }

                ty
            },
            (ty, sub) => {
                assert!(ty.is_none() && id.is_none());
                let (ty, sub) = sub.unwrap();

                write!(f, "subgraph {sub:?}")?;

                ty
            },
        };

        f.write_str(" {")?;

        for (key, val) in attrs {
            write!(f, "{key}={val:?};")?;
        }

        for (id, node) in nodes {
            match node {
                NodeLike::Node(Node { attrs }) => {
                    let mut attr_state = AttrState::default();
                    write!(f, "{id:?}")?;

                    for (key, val) in attrs {
                        attr_state.write_one(f, key, |f| write!(f, "{val:?}"))?;
                    }

                    attr_state.finish(f)?;
                },
                NodeLike::Subgraph(graph) => {
                    graph.fmt_impl(f, Some((ty, id)))?;
                },
            }

            f.write_str(";")?;
        }

        for ((l, r), edges) in edges {
            for Edge { attrs } in edges {
                let mut attr_state = AttrState::default();
                write!(f, "{l:?}{}{r:?}", match ty {
                    GraphType::Undirected => "--",
                    GraphType::Directed => "->",
                })?;

                for (key, val) in attrs {
                    attr_state.write_one(f, key, |f| write!(f, "{val:?}"))?;
                }

                attr_state.finish(f)?;
                f.write_str(";")?;
            }
        }

        f.write_str("}")
    }
}

impl Display for Graph<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.fmt_impl(f, None) }
}

#[derive(Debug, Default)]
pub struct Node<'a> {
    attrs: BTreeMap<&'static str, Cow<'a, str>>,
}

impl<'a> Node<'a> {
    attr!(style, "style");

    attr!(shape, "shape");

    attr!(margin, "margin");

    attr!(label, "label");

    attr!(border_count, "peripheries");

    attr!(edge_ordering, "ordering");
}

#[derive(Debug, Default)]
pub struct Edge<'a> {
    attrs: BTreeMap<&'static str, Cow<'a, str>>,
}

impl<'a> Edge<'a> {
    attr!(style, "style");

    attr!(min_len, "minlen");

    attr!(constraint, "constraint");

    attr!(concentrate, "concentrate");

    attr!(label, "label");

    attr!(color, "color");
}
