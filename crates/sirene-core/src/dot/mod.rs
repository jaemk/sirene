//! Graphviz DOT (DOT-*), lowered to the graph model.

mod lower;
mod parser;
mod x11;

use rowan::GreenNode;

pub(crate) use parser::is_dot;

use crate::diagnostic::Diagnostic;
use crate::flowchart::Flowchart;
use crate::scene::Color;
use crate::syntax::SyntaxNode;

/// The result of parsing DOT source: a lossless syntax tree plus diagnostics.
#[derive(Debug, Clone)]
pub struct Parse {
    green: GreenNode,
    diagnostics: Vec<Diagnostic>,
}

/// A DOT graph in the graph model.
#[derive(Debug, Clone, PartialEq)]
pub struct Lowered {
    pub flowchart: Flowchart,
    /// The parser's and the lowering's diagnostics, ordered by position.
    pub diagnostics: Vec<Diagnostic>,
    /// The graph's `bgcolor`.
    pub background: Option<Color>,
}

impl Parse {
    /// Root of the syntax tree. Its text is always identical to the parsed source.
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Lowers the first graph to the graph model.
    pub fn lower(&self) -> Lowered {
        let out = lower::lower(&self.syntax());
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(out.diagnostics);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered a DOT graph: {} nodes, {} edges, {} clusters; {} diagnostics",
            out.flowchart.nodes.len(),
            out.flowchart.edges.len(),
            out.flowchart.subgraphs.len(),
            diagnostics.len()
        );
        Lowered {
            flowchart: out.flowchart,
            diagnostics,
            background: out.background,
        }
    }
}

/// Parses DOT source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}
