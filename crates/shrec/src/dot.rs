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
pub struct Graph<'a> {
    ty: GraphType,
    id: Option<Cow<'a, str>>,
    nodes: IndexMap<Cow<'a, str>, Node<'a>>,
    edges: IndexMap<(Cow<'a, str>, Cow<'a, str>), Vec<Edge<'a>>>,
}

impl<'a> Graph<'a> {
    #[must_use]
    #[inline]
    pub fn new(ty: GraphType, id: Option<Cow<'a, str>>) -> Self {
        Self {
            ty,
            id,
            nodes: IndexMap::new(),
            edges: IndexMap::new(),
        }
    }

    #[inline]
    pub fn node(&mut self, id: Cow<'a, str>) -> &mut Node<'a> { self.nodes.entry(id).or_default() }

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

impl Display for Graph<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.ty)?;

        if let Some(id) = &self.id {
            write!(f, " {id:?}")?;
        }

        f.write_str(" {")?;

        for (
            id,
            Node {
                label,
                peripheries,
                _p,
            },
        ) in &self.nodes
        {
            let mut attrs = AttrState::default();
            write!(f, "{id:?}")?;

            if let Some(label) = label {
                attrs.write_one(f, "label", |f| write!(f, "{label:?}"))?;
            }

            if let Some(peripheries) = peripheries {
                attrs.write_one(f, "peripheries", |f| write!(f, "{peripheries}"))?;
            }

            attrs.finish(f)?;
            f.write_str(";")?;
        }

        for ((l, r), edges) in &self.edges {
            for Edge { label } in edges {
                let mut attrs = AttrState::default();
                write!(f, "{l:?}{}{r:?}", match self.ty {
                    GraphType::Undirected => "--",
                    GraphType::Directed => "->",
                })?;

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

#[derive(Debug, Default)]
pub struct Node<'a> {
    label: Option<Cow<'a, str>>,
    peripheries: Option<u8>,
    _p: std::marker::PhantomData<&'a ()>,
}

impl<'a> Node<'a> {
    pub fn label(&mut self, label: Cow<'a, str>) { self.label = Some(label); }

    pub fn border_count(&mut self, count: u8) { self.peripheries = Some(count); }
}

#[derive(Debug, Default)]
pub struct Edge<'a> {
    label: Option<Cow<'a, str>>,
}

impl<'a> Edge<'a> {
    pub fn label(&mut self, label: Cow<'a, str>) { self.label = Some(label); }
}
