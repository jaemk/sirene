//! Mermaid flowchart parsing and lowering.

mod lower;
mod model;
mod parser;
mod scan;
mod shape;
mod text;

use rowan::GreenNode;

pub use model::{
    Arrow, ClassDef, Click, Direction, Edge, Flowchart, Label, Metadata, Node, Shape, Stroke,
    Style, Subgraph,
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
    Parse { green, diagnostics }
}
