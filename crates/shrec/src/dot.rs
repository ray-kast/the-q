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
            nodes: IndexMap::new(),
            edges: IndexMap::new(),
        }
    }

    pub fn style(&mut self, style: Cow<'a, str>) { self.style = Some(style); }

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
        let ty = match (self.ty, sub_id) {
            (ty, None) => {
                let ty = ty.unwrap();
                write!(f, "{ty}")?;

                if let Some(id) = &self.id {
                    write!(f, " {id:?}")?;
                }

                ty
            },
            (ty, sub) => {
                assert!(ty.is_none() && self.id.is_none());
                let (ty, sub) = sub.unwrap();

                write!(f, "subgraph {sub:?}")?;

                ty
            },
        };

        f.write_str(" {")?;

        if let Some(ref style) = self.style {
            write!(f, "style={style};")?;
        }

        for (id, node) in &self.nodes {
            match node {
                NodeLike::Node(Node {
                    style,
                    shape,
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

        for ((l, r), edges) in &self.edges {
            for Edge {
                style,
                minlen,
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
    label: Option<Cow<'a, str>>,
    peripheries: Option<u8>,
    _p: std::marker::PhantomData<&'a ()>,
}

impl<'a> Node<'a> {
    pub fn style(&mut self, style: Cow<'a, str>) { self.style = Some(style); }

    pub fn shape(&mut self, shape: Cow<'a, str>) { self.shape = Some(shape); }

    pub fn label(&mut self, label: Cow<'a, str>) { self.label = Some(label); }

    pub fn border_count(&mut self, count: u8) { self.peripheries = Some(count); }
}

#[derive(Debug, Default)]
pub struct Edge<'a> {
    style: Option<Cow<'a, str>>,
    minlen: Option<Cow<'a, str>>,
    label: Option<Cow<'a, str>>,
}

impl<'a> Edge<'a> {
    pub fn style(&mut self, style: Cow<'a, str>) { self.style = Some(style); }

    pub fn minlen(&mut self, minlen: Cow<'a, str>) { self.minlen = Some(minlen); }

    pub fn label(&mut self, label: Cow<'a, str>) { self.label = Some(label); }
}
