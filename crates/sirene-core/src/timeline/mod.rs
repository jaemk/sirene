//! Mermaid timelines (TIMELI-*): time periods with their events, grouped in sections.

mod lower;
mod parser;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::syntax::SyntaxNode;

/// Which way the periods run (TIMELI-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    #[default]
    LeftRight,
    TopDown,
}

/// Text split into lines at `<br>`.
pub type Text = Vec<String>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Period {
    pub label: Text,
    pub events: Vec<Text>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Section {
    /// `None` for the periods before the first `section`.
    pub name: Option<Text>,
    pub periods: Vec<Period>,
}

/// A timeline (TIMELI-2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Timeline {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub direction: Direction,
    pub sections: Vec<Section>,
}

impl Timeline {
    /// Every period in order, with the index of its section.
    pub fn periods(&self) -> impl Iterator<Item = (usize, &Period)> {
        self.sections
            .iter()
            .enumerate()
            .flat_map(|(s, sec)| sec.periods.iter().map(move |p| (s, p)))
    }

    /// Whether any section has a name; if none does, each period gets its own color.
    pub fn has_named_sections(&self) -> bool {
        self.sections.iter().any(|s| s.name.is_some())
    }
}

/// The result of parsing timeline source: a lossless syntax tree plus diagnostics.
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

    /// Lowers the tree to a [`Timeline`]. The returned diagnostics include the parser's,
    /// ordered by position.
    pub fn lower(&self) -> (Timeline, Vec<Diagnostic>) {
        let (timeline, lowering) = lower::lower(&self.syntax());
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(lowering);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered a timeline: {} sections, {} periods; {} diagnostics",
            timeline.sections.len(),
            timeline.periods().count(),
            diagnostics.len()
        );
        (timeline, diagnostics)
    }
}

/// Parses timeline source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}
