//! Mermaid state diagrams (MSTATE-*), lowered to the flowchart model.

mod lower;
mod parser;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::flowchart::Lowered;
use crate::syntax::SyntaxNode;

/// The result of parsing state diagram source: a lossless syntax tree plus diagnostics.
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

    /// Indented dump of the tree with kinds, ranges, and token text.
    pub fn debug_tree(&self) -> String {
        format!("{:#?}", self.syntax())
    }

    /// Lowers the tree to the graph model. The returned diagnostics include the parser's,
    /// ordered by position.
    pub fn lower(&self) -> Lowered {
        let (flowchart, lowering) = lower::lower(&self.syntax());
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(lowering);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered a state diagram: {} states, {} transitions, {} composites and regions; {} diagnostics",
            flowchart.nodes.len(),
            flowchart.edges.len(),
            flowchart.subgraphs.len(),
            diagnostics.len()
        );
        Lowered {
            flowchart,
            diagnostics,
        }
    }
}

/// Parses state diagram source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}
