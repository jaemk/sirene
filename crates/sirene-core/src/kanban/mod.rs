//! Mermaid kanban boards (MKAN-*): columns of cards, parsed with the mindmap grammar.

mod lower;

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::Label;
use crate::syntax::SyntaxNode;

pub(crate) const HEADERS: &[&str] = &["kanban"];

/// A card's `priority`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    VeryHigh,
    High,
    Low,
    VeryLow,
}

impl Priority {
    /// The priority named `s`, in any case.
    pub fn of(s: &str) -> Option<Priority> {
        match s.trim().to_ascii_lowercase().as_str() {
            "very high" => Some(Priority::VeryHigh),
            "high" => Some(Priority::High),
            "low" => Some(Priority::Low),
            "very low" => Some(Priority::VeryLow),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Priority::VeryHigh => "Very High",
            Priority::High => "High",
            Priority::Low => "Low",
            Priority::VeryLow => "Very Low",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Card {
    pub id: Option<String>,
    pub label: Label,
    pub assigned: Option<String>,
    pub ticket: Option<String>,
    pub priority: Option<Priority>,
    pub icon: Option<String>,
    pub classes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Column {
    pub id: Option<String>,
    pub label: Label,
    pub icon: Option<String>,
    pub classes: Vec<String>,
    pub cards: Vec<Card>,
}

/// A kanban board (MKAN-2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Kanban {
    pub title: Option<String>,
    /// Frontmatter `config.kanban.ticketBaseUrl`, with `#TICKET#` standing for a card's ticket.
    pub ticket_base_url: Option<String>,
    pub columns: Vec<Column>,
}

impl Kanban {
    /// The link for `ticket`, when there is a base URL.
    pub fn ticket_url(&self, ticket: &str) -> Option<String> {
        self.ticket_base_url
            .as_deref()
            .map(|base| base.replace("#TICKET#", ticket))
    }
}

/// The result of parsing kanban source: a lossless syntax tree plus diagnostics.
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

    /// Lowers the tree to a [`Kanban`]. The returned diagnostics include the parser's, ordered
    /// by position.
    pub fn lower(&self) -> (Kanban, Vec<Diagnostic>) {
        let (board, lowering) = lower::lower(&self.syntax());
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(lowering);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered a kanban board: {} columns; {} diagnostics",
            board.columns.len(),
            diagnostics.len()
        );
        (board, diagnostics)
    }
}

/// Parses kanban source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = crate::mindmap::parser::parse_with(src, HEADERS);
    Parse { green, diagnostics }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priorities() {
        assert_eq!(Priority::of("very HIGH"), Some(Priority::VeryHigh));
        assert_eq!(Priority::of(" Low "), Some(Priority::Low));
        assert_eq!(Priority::of("medium"), None);
        assert_eq!(Priority::VeryLow.name(), "Very Low");
    }

    #[test]
    fn ticket_urls() {
        let mut k = Kanban::default();
        assert_eq!(k.ticket_url("A-1"), None);
        k.ticket_base_url = Some("https://t/#TICKET#".into());
        assert_eq!(k.ticket_url("A-1").as_deref(), Some("https://t/A-1"));
    }
}
