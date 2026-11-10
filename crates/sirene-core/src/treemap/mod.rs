//! Mermaid treemaps (MTREE-*): nested sections and leaves, as rectangles sized by value.

mod format;
mod layout;
mod lower;
mod parser;

use rowan::GreenNode;

pub use format::Format;
pub use layout::Cell;
pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::flowchart::Style;
use crate::syntax::SyntaxNode;

/// A section (no value, holding items) or a leaf (a value).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Item {
    pub name: String,
    /// A leaf's value; `None` for a section.
    pub value: Option<f64>,
    /// The `:::class` applied, if any.
    pub class: Option<String>,
    /// The class's `classDef` styles.
    pub styles: Vec<Style>,
    pub parent: Option<usize>,
    /// Indexes of the items under this one, in source order.
    pub children: Vec<usize>,
}

impl Item {
    pub fn is_section(&self) -> bool {
        self.value.is_none()
    }
}

/// A treemap (MTREE-2).
#[derive(Debug, Clone, PartialEq)]
pub struct Treemap {
    /// The `title` statement, else frontmatter `title`.
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    /// Every item, in source order.
    pub items: Vec<Item>,
    /// `padding`: space between sibling items, and inside a section; 10 unless set.
    pub padding: f32,
    /// `diagramPadding`: space around the drawing; 8 unless set.
    pub diagram_padding: f32,
    /// `showValues`, on unless `false`.
    pub show_values: bool,
    /// The drawing's size: `nodeWidth` and `nodeHeight` times 10, so 1000 by 400.
    pub width: f32,
    pub height: f32,
    /// `valueFormat`, `,` unless set.
    pub value_format: Format,
}

impl Default for Treemap {
    fn default() -> Self {
        Treemap {
            title: None,
            acc_title: None,
            acc_descr: None,
            items: Vec::new(),
            padding: 10.0,
            diagram_padding: 8.0,
            show_values: true,
            width: 1000.0,
            height: 400.0,
            value_format: Format::default(),
        }
    }
}

impl Treemap {
    /// The top-level items.
    pub fn roots(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.items.len()).filter(|&i| self.items[i].parent.is_none())
    }

    /// Each item's value: a leaf's own, a section's the sum of its items'.
    pub fn values(&self) -> Vec<f64> {
        let mut out = vec![0.0; self.items.len()];
        // Children always come after their parent, so summing backwards sees them first.
        for i in (0..self.items.len()).rev() {
            let item = &self.items[i];
            out[i] += item.value.unwrap_or(0.0).max(0.0);
            if let Some(p) = item.parent {
                out[p] += out[i];
            }
        }
        out
    }

    /// `ids`, largest value first, ties in source order.
    pub fn by_value(&self, ids: impl Iterator<Item = usize>, values: &[f64]) -> Vec<usize> {
        let mut v: Vec<usize> = ids.collect();
        v.sort_by(|&a, &b| values[b].total_cmp(&values[a]));
        v
    }

    /// A value as shown, by `valueFormat`.
    pub fn format_value(&self, v: f64) -> String {
        self.value_format.apply(v)
    }
}

/// The result of parsing treemap source: a lossless syntax tree plus diagnostics.
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

    /// Lowers the tree to a [`Treemap`]. The returned diagnostics include the parser's, ordered
    /// by position.
    pub fn lower(&self) -> (Treemap, Vec<Diagnostic>) {
        let (treemap, lowering) = lower::lower(&self.syntax());
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(lowering);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered a treemap: {} items; {} diagnostics",
            treemap.items.len(),
            diagnostics.len()
        );
        (treemap, diagnostics)
    }
}

/// Parses treemap source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_and_order() {
        let item = |name: &str, value: Option<f64>, parent: Option<usize>| Item {
            name: name.into(),
            value,
            parent,
            ..Item::default()
        };
        let t = Treemap {
            items: vec![
                item("a", None, None),
                item("b", Some(2.0), Some(0)),
                item("c", Some(5.0), Some(0)),
                item("d", Some(1.0), None),
            ],
            ..Treemap::default()
        };
        let v = t.values();
        assert_eq!(v, [7.0, 2.0, 5.0, 1.0]);
        assert_eq!(t.roots().collect::<Vec<_>>(), [0, 3]);
        assert_eq!(t.by_value([1, 2].into_iter(), &v), [2, 1]);
        assert_eq!(t.format_value(1234.0), "1,234");
    }
}
