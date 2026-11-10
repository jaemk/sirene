//! Mermaid C4 diagrams (MC4-*): people, systems, containers, components, and deployment nodes
//! in nested boundaries, with relations between them.

pub mod layout;
mod lower;
mod parser;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::flowchart::Flowchart;
use crate::syntax::SyntaxNode;

/// What an element is, by the macro that declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base {
    Person,
    System,
    Container,
    Component,
}

/// An element's outline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Body {
    Box,
    /// `...Db`: a cylinder.
    Db,
    /// `...Queue`: a cylinder on its side.
    Queue,
}

/// A person, system, container, or component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub base: Base,
    pub body: Body,
    /// `..._Ext`.
    pub external: bool,
    /// A container's or component's technology.
    pub techn: Option<String>,
    pub descr: Option<String>,
    /// `$bgColor`, `$fontColor`, and `$borderColor` from `UpdateElementStyle`.
    pub colors: Colors,
}

impl Element {
    /// Mermaid's name for the element's type, shown as `<<name>>`: `person`,
    /// `external_system_db`, ...
    pub fn type_name(&self) -> String {
        let base = match self.base {
            Base::Person => "person",
            Base::System => "system",
            Base::Container => "container",
            Base::Component => "component",
        };
        let body = match self.body {
            Body::Box => "",
            Body::Db => "_db",
            Body::Queue => "_queue",
        };
        let ext = if self.external { "external_" } else { "" };
        format!("{ext}{base}{body}")
    }
}

/// Colors set by `UpdateElementStyle` or `UpdateRelStyle`, as CSS colors.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Colors {
    /// An element's fill.
    pub background: Option<String>,
    /// Text.
    pub font: Option<String>,
    /// An element's border, or a relation's line.
    pub line: Option<String>,
}

/// A boundary or deployment node holding elements and other boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Boundary {
    /// The type shown under the label as `[type]`: `ENTERPRISE`, `SYSTEM`, `CONTAINER`, a
    /// deployment node's type, or the `type` argument of `Boundary`.
    pub kind: Option<String>,
    pub descr: Option<String>,
    /// A deployment node, drawn solid rather than dashed.
    pub deployment: bool,
}

/// A relation's text beyond its label (`chart.edges[i]` is `rels[i]`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rel {
    pub techn: Option<String>,
    pub descr: Option<String>,
    pub colors: Colors,
}

/// A C4 diagram (MC4-2). Elements are the chart's nodes, boundaries its subgraphs, and
/// relations its edges, so they draw as flowchart nodes, clusters, and edges do; `layout` places
/// them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct C4 {
    pub chart: Flowchart,
    /// For each node.
    pub elements: Vec<Element>,
    pub node_parent: Vec<Option<usize>>,
    /// For each subgraph.
    pub boundaries: Vec<Boundary>,
    pub boundary_parent: Vec<Option<usize>>,
    /// For each edge.
    pub rels: Vec<Rel>,
    /// `UpdateLayoutConfig`: elements per row (4) and boundaries per row (2).
    pub shapes_in_row: usize,
    pub boundaries_in_row: usize,
}

/// The result of parsing C4 diagram source: a lossless syntax tree plus diagnostics.
#[derive(Debug, Clone)]
pub struct Parse {
    green: GreenNode,
    diagnostics: Vec<Diagnostic>,
}

impl Parse {
    /// Root of the syntax tree. Its text is always identical to the parsed source.
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Lowers the tree to a [`C4`]. The returned diagnostics include the parser's, ordered by
    /// position.
    pub fn lower(&self) -> (C4, Vec<Diagnostic>) {
        let (c4, lowering) = lower::lower(&self.syntax());
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(lowering);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered a C4 diagram: {} elements, {} boundaries, {} relations; {} diagnostics",
            c4.chart.nodes.len(),
            c4.chart.subgraphs.len(),
            c4.chart.edges.len(),
            diagnostics.len()
        );
        (c4, diagnostics)
    }
}

/// Parses C4 diagram source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}
