//! Mermaid architecture diagrams (MARCH-*): services and junctions, nested groups, and edges
//! that leave and enter by a named side.

pub mod layout;
mod lower;
mod parser;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::flowchart::Flowchart;
use crate::syntax::SyntaxNode;

/// A side of a service, junction, or group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    pub fn from_char(c: char) -> Option<Side> {
        Some(match c {
            'L' => Side::Left,
            'R' => Side::Right,
            'T' => Side::Top,
            'B' => Side::Bottom,
            _ => return None,
        })
    }

    pub fn is_x(self) -> bool {
        matches!(self, Side::Left | Side::Right)
    }
}

/// What a node of the chart is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// A service, with its icon name such as `database` or `logos:aws-s3`.
    Service { icon: Option<String> },
    /// A point where edges meet.
    Junction,
}

/// Where an edge leaves and enters (`chart.edges[i]` is `ports[i]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Port {
    /// `chart.nodes` index of each end's service or junction.
    pub from: usize,
    pub to: usize,
    pub from_side: Side,
    pub to_side: Side,
    /// `{group}`: the end is the boundary of the end's group, beside it.
    pub from_group: bool,
    pub to_group: bool,
}

/// `align row ...` or `align column ...`: nodes kept in one row or column, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Align {
    pub row: bool,
    pub nodes: Vec<usize>,
}

/// An architecture diagram (MARCH-2). Services and junctions are the chart's nodes and groups
/// its subgraphs, so they draw as flowchart nodes and clusters do; `layout` places them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Architecture {
    pub chart: Flowchart,
    /// For each node, what it is.
    pub items: Vec<Item>,
    /// For each node, the subgraph it is in.
    pub node_parent: Vec<Option<usize>>,
    /// For each subgraph, its icon and the subgraph it is in.
    pub group_icons: Vec<Option<String>>,
    pub group_parent: Vec<Option<usize>>,
    pub ports: Vec<Port>,
    pub aligns: Vec<Align>,
}

/// The result of parsing architecture diagram source: a lossless syntax tree plus diagnostics.
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

    /// Lowers the tree to an [`Architecture`]. The returned diagnostics include the parser's,
    /// ordered by position.
    pub fn lower(&self) -> (Architecture, Vec<Diagnostic>) {
        let (arch, lowering) = lower::lower(&self.syntax());
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(lowering);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered an architecture diagram: {} nodes, {} groups, {} edges; {} diagnostics",
            arch.chart.nodes.len(),
            arch.chart.subgraphs.len(),
            arch.chart.edges.len(),
            diagnostics.len()
        );
        (arch, diagnostics)
    }
}

/// Parses architecture diagram source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}
