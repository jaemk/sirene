//! PlantUML sequence diagrams (PSEQ-*), lowered to the sequence diagram model.

mod lower;
mod parser;

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::sequence::Sequence;
use crate::syntax::SyntaxNode;

/// The result of parsing PlantUML source: a lossless syntax tree plus diagnostics.
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
        let (seq, lowering) = lower::lower(&self.syntax());
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(lowering);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered a PlantUML diagram: {} participants, {} events; {} diagnostics",
            seq.participants.len(),
            seq.events.len(),
            diagnostics.len()
        );
        (seq, diagnostics)
    }
}

/// Parses PlantUML source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}

/// Whether `src` has a PlantUML `@start...` line, which may follow other text as PlantUML
/// allows (PSEQ-1, LANGS-1).
pub fn is_plantuml(src: &str) -> bool {
    src[scan::bom_len(src)..]
        .lines()
        .any(|l| l.trim_start().starts_with("@start"))
}
