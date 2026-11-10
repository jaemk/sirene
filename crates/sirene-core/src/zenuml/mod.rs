//! ZenUML sequence diagrams (MZEN-*), lowered to the sequence diagram model.

mod lower;
mod parser;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;

use crate::diagnostic::{Diagnostic, Severity};
use crate::sequence::Sequence;
use crate::syntax::SyntaxNode;

/// The result of parsing ZenUML source: a lossless syntax tree plus diagnostics.
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

    /// Lowers the tree to a [`Sequence`]. The returned diagnostics include the parser's,
    /// ordered by position.
    pub fn lower(&self) -> (Sequence, Vec<Diagnostic>) {
        let errors = self
            .diagnostics
            .iter()
            // An unclosed block ends at the end of input, not on a statement's line.
            .filter(|d| d.severity == Severity::Error && !d.message.starts_with("unclosed block"))
            .map(|d| d.range.start())
            .collect();
        let (seq, lowering) = lower::lower(&self.syntax(), errors);
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(lowering);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered a ZenUML diagram: {} participants, {} events; {} diagnostics",
            seq.participants.len(),
            seq.events.len(),
            diagnostics.len()
        );
        (seq, diagnostics)
    }
}

/// Parses ZenUML source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}
