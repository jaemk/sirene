//! Mermaid flowchart parsing and lowering.

pub mod edit;
mod graph;
mod lower;
mod model;
mod parser;
pub(crate) mod scan;
mod shape;
pub(crate) mod text;

use rowan::GreenNode;

pub use graph::Measure;
pub use model::{
    Arrow, ClassDef, Click, Direction, Edge, Flowchart, Label, Metadata, Node, Note, NoteSide, Row,
    Shape, Span, Stroke, Style, Subgraph, clean,
};

use crate::diagnostic::Diagnostic;
use crate::syntax::SyntaxNode;

/// The result of parsing flowchart source: a lossless syntax tree plus diagnostics.
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

    /// The keyword of the other Mermaid diagram type the source is, e.g. `pie` (FLOWCH-16).
    pub fn other_diagram(&self) -> Option<String> {
        let root = self.syntax();
        if root
            .children()
            .any(|n| n.kind() == crate::syntax::SyntaxKind::HEADER)
        {
            return None;
        }
        root.children()
            .filter(|n| n.kind() == crate::syntax::SyntaxKind::ERROR)
            .find_map(|n| parser::other_diagram(&n.text().to_string()).map(str::to_owned))
    }

    /// Indented dump of the tree with kinds, ranges, and token text.
    pub fn debug_tree(&self) -> String {
        format!("{:#?}", self.syntax())
    }

    /// Lowers the tree to the semantic model. The returned diagnostics include the parser's,
    /// ordered by position.
    pub fn lower(&self) -> Lowered {
        let (flowchart, lowering) = lower::lower(&self.syntax());
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(lowering);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered {} nodes, {} edges, {} subgraphs, {} class definitions; {} diagnostics",
            flowchart.nodes.len(),
            flowchart.edges.len(),
            flowchart.subgraphs.len(),
            flowchart.classes.len(),
            diagnostics.len()
        );
        Lowered {
            flowchart,
            diagnostics,
        }
    }
}

/// A flowchart model plus every diagnostic from parsing and lowering.
#[derive(Debug, Clone)]
pub struct Lowered {
    pub flowchart: Flowchart,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses flowchart source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    log::debug!(
        "parsed {} bytes, {} lines; {} syntax diagnostics",
        src.len(),
        src.lines().count(),
        diagnostics.len()
    );
    Parse { green, diagnostics }
}
