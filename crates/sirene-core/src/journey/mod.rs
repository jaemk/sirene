//! Mermaid user journeys (MJOUR-*): tasks in sections, each scored and done by actors.

mod lower;
mod parser;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::syntax::SyntaxNode;

/// The lowest and highest scores.
pub const MIN_SCORE: f64 = 1.0;
pub const MAX_SCORE: f64 = 5.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    pub name: String,
    /// From 1 (unhappy) to 5 (happy).
    pub score: f64,
    pub actors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Section {
    /// `None` for the tasks before the first `section`.
    pub name: Option<String>,
    pub tasks: Vec<Task>,
}

/// A user journey (MJOUR-2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Journey {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub sections: Vec<Section>,
}

impl Journey {
    /// Every task in order, with the index of its section.
    pub fn tasks(&self) -> impl Iterator<Item = (usize, &Task)> {
        self.sections
            .iter()
            .enumerate()
            .flat_map(|(s, sec)| sec.tasks.iter().map(move |t| (s, t)))
    }

    /// Every actor once, in the order they first appear.
    pub fn actors(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for (_, t) in self.tasks() {
            for a in &t.actors {
                if !out.contains(&a.as_str()) {
                    out.push(a);
                }
            }
        }
        out
    }
}

/// The result of parsing journey source: a lossless syntax tree plus diagnostics.
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

    /// Lowers the tree to a [`Journey`]. The returned diagnostics include the parser's, ordered
    /// by position.
    pub fn lower(&self) -> (Journey, Vec<Diagnostic>) {
        let (journey, lowering) = lower::lower(&self.syntax());
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(lowering);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered a journey: {} sections, {} tasks; {} diagnostics",
            journey.sections.len(),
            journey.tasks().count(),
            diagnostics.len()
        );
        (journey, diagnostics)
    }
}

/// Parses journey source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}
