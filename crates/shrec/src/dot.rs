use std::{
    borrow::Cow,
    fmt::{self, Display},
};

use indexmap::IndexMap;

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
    style: Option<Cow<'a, str>>,
    penwidth: Option<Cow<'a, str>>,
    label: Option<Cow<'a, str>>,
    nodes: IndexMap<Cow<'a, str>, NodeLike<'a>>,
    edges: IndexMap<(Cow<'a, str>, Cow<'a, str>), Vec<Edge<'a>>>,
}

impl<'a> Graph<'a> {
    #[must_use]
    #[inline]
    pub fn new(ty: GraphType, id: Option<Cow<'a, str>>) -> Self { Self::new_impl(Some(ty), id) }

    fn new_impl(ty: Option<GraphType>, id: Option<Cow<'a, str>>) -> Self {
        Self {
            ty,
            id,
            style: None,
            penwidth: None,
            label: None,
            nodes: IndexMap::new(),
            edges: IndexMap::new(),
        }
    }

    pub fn style(&mut self, style: Cow<'a, str>) { self.style = Some(style); }

    pub fn label(&mut self, label: Cow<'a, str>) { self.label = Some(label); }

    pub fn penwidth(&mut self, penwidth: Cow<'a, str>) { self.penwidth = Some(penwidth); }

    #[inline]
    pub fn node(&mut self, id: Cow<'a, str>) -> &mut Node<'a> {
        use indexmap::map::Entry;

        let entry = match self.nodes.entry(id) {
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

        if let NodeLike::Node(ref mut n) = entry {
            n
        } else {
            unreachable!()
        }
    }

    #[inline]
    pub fn subgraph(&mut self, id: Cow<'a, str>) -> &mut Graph<'a> {
        use indexmap::map::Entry;

        let entry = match self.nodes.entry(id) {
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

        if let NodeLike::Subgraph(ref mut g) = entry {
            g
        } else {
            unreachable!()
        }
    }

    #[inline]
    pub fn edge(&mut self, l: Cow<'a, str>, r: Cow<'a, str>) -> &mut Edge<'a> {
        self.node(l.clone());
        self.node(r.clone());
        let edges = self.edges.entry((l, r)).or_default();
        edges.push(Edge::default());
        edges.last_mut().unwrap_or_else(|| unreachable!())
    }

    pub(crate) fn state_machine<
        I,
        S,
        T,
        IN: IntoIterator<Item = (S, IE, Option<T>)>,
        IE: IntoIterator<Item = (I, IO)>,
        IO: IntoIterator<Item = S>,
        G: FnMut(&S) -> u32,
        FI: Fn(I) -> Cow<'a, str>,
        FS: Fn(S) -> Cow<'a, str>,
        FT: Fn(T) -> Option<Cow<'a, str>>,
    >(
        nodes: IN,
        start: &S,
        mut get_id: G,
        fmt_input: FI,
        fmt_state: FS,
        fmt_tok: FT,
    ) -> Self {
        let mut graph = Self::new(GraphType::Directed, None);

        for (state, edges, accept) in nodes {
            let id = Cow::from(get_id(&state).to_string());
            let node = graph.node(id.clone());

            let mut label = fmt_state(state);
            if let Some(tok) = accept {
                if let Some(tok) = fmt_tok(tok) {
                    label = format!("{label}:{tok}").into();
                }

                node.border_count(2);
            }

            node.label(label);

            for (input, outputs) in edges {
                let input = fmt_input(input);

                for next_state in outputs {
                    let edge = graph.edge(id.clone(), get_id(&next_state).to_string().into());

                    edge.label(input.clone());
                }
            }
        }

        let start_id = Cow::from("_start");
        let start_node = graph.node(start_id.clone());
        start_node.style("invis".into());
        start_node.shape("point".into());
        start_node.label("".into());
        graph.edge(start_id, get_id(start).to_string().into());

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
            style,
            penwidth,
            label,
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

        if let Some(ref style) = style {
            write!(f, "style={style};")?;
        }

        if let Some(ref penwidth) = penwidth {
            write!(f, "penwidth={penwidth};")?;
        }

        if let Some(ref label) = label {
            write!(f, "label={label};")?;
        }

        for (id, node) in nodes {
            match node {
                NodeLike::Node(Node {
                    style,
                    shape,
                    margin,
                    label,
                    peripheries,
                    _p,
                }) => {
                    let mut attrs = AttrState::default();
                    write!(f, "{id:?}")?;

                    if let Some(style) = style {
                        attrs.write_one(f, "style", |f| write!(f, "{style:?}"))?;
                    }

                    if let Some(shape) = shape {
                        attrs.write_one(f, "shape", |f| write!(f, "{shape:?}"))?;
                    }

                    if let Some(margin) = margin {
                        attrs.write_one(f, "margin", |f| write!(f, "{margin:?}"))?;
                    }

                    if let Some(label) = label {
                        attrs.write_one(f, "label", |f| write!(f, "{label:?}"))?;
                    }

                    if let Some(peripheries) = peripheries {
                        attrs.write_one(f, "peripheries", |f| write!(f, "{peripheries}"))?;
                    }

                    attrs.finish(f)?;
                },
                NodeLike::Subgraph(graph) => {
                    graph.fmt_impl(f, Some((ty, id)))?;
                },
            }

            f.write_str(";")?;
        }

        for ((l, r), edges) in edges {
            for Edge {
                style,
                minlen,
                constraint,
                label,
            } in edges
            {
                let mut attrs = AttrState::default();
                write!(f, "{l:?}{}{r:?}", match ty {
                    GraphType::Undirected => "--",
                    GraphType::Directed => "->",
                })?;

                if let Some(style) = style {
                    attrs.write_one(f, "style", |f| write!(f, "{style:?}"))?;
                }

                if let Some(minlen) = minlen {
                    attrs.write_one(f, "minlen", |f| write!(f, "{minlen:?}"))?;
                }

                if let Some(constraint) = constraint {
                    attrs.write_one(f, "constraint", |f| write!(f, "{constraint:?}"))?;
                }

                if let Some(label) = label {
                    attrs.write_one(f, "label", |f| write!(f, "{label:?}"))?;
                }

                attrs.finish(f)?;
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
    style: Option<Cow<'a, str>>,
    shape: Option<Cow<'a, str>>,
    margin: Option<Cow<'a, str>>,
    label: Option<Cow<'a, str>>,
    peripheries: Option<u8>,
    _p: std::marker::PhantomData<&'a ()>,
}

impl<'a> Node<'a> {
    pub fn style(&mut self, style: Cow<'a, str>) { self.style = Some(style); }

    pub fn shape(&mut self, shape: Cow<'a, str>) { self.shape = Some(shape); }

    pub fn margin(&mut self, margin: Cow<'a, str>) { self.margin = Some(margin); }

    pub fn label(&mut self, label: Cow<'a, str>) { self.label = Some(label); }

    pub fn border_count(&mut self, count: u8) { self.peripheries = Some(count); }
}

#[derive(Debug, Default)]
pub struct Edge<'a> {
    style: Option<Cow<'a, str>>,
    minlen: Option<Cow<'a, str>>,
    constraint: Option<Cow<'a, str>>,
    label: Option<Cow<'a, str>>,
}

impl<'a> Edge<'a> {
    pub fn style(&mut self, style: Cow<'a, str>) { self.style = Some(style); }

    pub fn minlen(&mut self, minlen: Cow<'a, str>) { self.minlen = Some(minlen); }

    pub fn constraint(&mut self, constraint: Cow<'a, str>) { self.constraint = Some(constraint); }

    pub fn label(&mut self, label: Cow<'a, str>) { self.label = Some(label); }
}
