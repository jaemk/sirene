//! Mermaid flowchart parsing.

mod parser;
mod scan;

use rowan::GreenNode;

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

    /// Indented dump of the tree with kinds, ranges, and token text.
    pub fn debug_tree(&self) -> String {
        format!("{:#?}", self.syntax())
    }
}

/// Parses flowchart source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}
