//! Mermaid block diagrams (MBLOCK-*): blocks placed in a grid of columns, nested groups, and
//! edges between blocks.

pub mod layout;
mod lower;
mod parser;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::flowchart::Flowchart;
use crate::syntax::SyntaxNode;

/// A way a block arrow points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heading {
    Right,
    Left,
    Up,
    Down,
    /// Both left and right.
    X,
    /// Both up and down.
    Y,
}

impl Heading {
    pub fn from_text(s: &str) -> Option<Heading> {
        Some(match s {
            "right" => Heading::Right,
            "left" => Heading::Left,
            "up" => Heading::Up,
            "down" => Heading::Down,
            "x" => Heading::X,
            "y" => Heading::Y,
            _ => return None,
        })
    }
}

/// What fills one place in a group's grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellKind {
    /// `chart.nodes[i]`.
    Node(usize),
    /// An empty column, from `space`.
    Space,
    /// `groups[i]`.
    Group(usize),
}

/// A place in a group's grid, `width` columns wide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub kind: CellKind,
    pub width: usize,
}

/// The diagram, or a `block` ... `end` inside it: cells in rows of `columns`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Group {
    /// `chart.subgraphs[i]`; `None` for the diagram itself.
    pub subgraph: Option<usize>,
    /// `columns N`; `None` (or `columns auto`) puts every cell in one row.
    pub columns: Option<usize>,
    pub cells: Vec<Cell>,
}

/// A block diagram (MBLOCK-2). The blocks, edges, groups, and styles are a flowchart's, so they
/// draw as flowchart nodes do; the grid places them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Block {
    pub chart: Flowchart,
    /// `groups[0]` is the diagram; group `i + 1` is `chart.subgraphs[i]`.
    pub groups: Vec<Group>,
    /// For each node, the directions it points when it is a block arrow.
    pub arrows: Vec<Option<Vec<Heading>>>,
    /// `config.block.padding`: space between blocks and inside groups; 8 unless set.
    pub padding: f32,
}

impl Block {
    /// The group drawn for `chart.subgraphs[i]`.
    pub fn group_of(&self, subgraph: usize) -> Option<usize> {
        self.groups
            .iter()
            .position(|g| g.subgraph == Some(subgraph))
    }
}

/// The result of parsing block diagram source: a lossless syntax tree plus diagnostics.
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

    /// Lowers the tree to a [`Block`]. The returned diagnostics include the parser's, ordered by
    /// position.
    pub fn lower(&self) -> (Block, Vec<Diagnostic>) {
        let (block, lowering) = lower::lower(&self.syntax());
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(lowering);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered a block diagram: {} blocks, {} groups, {} edges; {} diagnostics",
            block.chart.nodes.len(),
            block.chart.subgraphs.len(),
            block.chart.edges.len(),
            diagnostics.len()
        );
        (block, diagnostics)
    }
}

/// Parses block diagram source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}
