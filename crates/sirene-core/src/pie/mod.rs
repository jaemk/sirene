//! Mermaid pie charts (MPIE-*): labeled values drawn as slices of a circle.

mod lower;
mod parser;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::syntax::SyntaxNode;

#[derive(Debug, Clone, PartialEq)]
pub struct Slice {
    pub label: String,
    pub value: f64,
}

/// A pie chart (MPIE-2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pie {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    /// `showData`: the legend and the text form show each value.
    pub show_data: bool,
    /// In source order.
    pub slices: Vec<Slice>,
}

impl Pie {
    /// The sum of the values.
    pub fn total(&self) -> f64 {
        self.slices.iter().map(|s| s.value).sum()
    }

    /// The slices largest first, ties in source order: the order they are drawn and listed.
    pub fn ordered(&self) -> Vec<&Slice> {
        let mut out: Vec<&Slice> = self.slices.iter().collect();
        out.sort_by(|a, b| b.value.total_cmp(&a.value));
        out
    }
}

/// `value`'s share of `total` as a whole percent, `0` when the total is zero.
pub fn percent(value: f64, total: f64) -> u32 {
    if total > 0.0 {
        (value / total * 100.0).round() as u32
    } else {
        0
    }
}

/// The result of parsing pie chart source: a lossless syntax tree plus diagnostics.
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

    /// Lowers the tree to a [`Pie`]. The returned diagnostics include the parser's, ordered by
    /// position.
    pub fn lower(&self) -> (Pie, Vec<Diagnostic>) {
        let (pie, lowering) = lower::lower(&self.syntax());
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(lowering);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered a pie chart: {} slices; {} diagnostics",
            pie.slices.len(),
            diagnostics.len()
        );
        (pie, diagnostics)
    }
}

/// Parses pie chart source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}
