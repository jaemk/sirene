//! Drawing edits to diagrams that are lists of labeled items (WYSIWY-28): each item is named by
//! its index in the lowered model and drawn as `Owner::Node(index)`, and can be renamed, added
//! after another or at the end, and deleted.

use std::ops::Range;

use crate::diagram::Kind;
use crate::edits::{combine, indent, newline, span, statement_span};
use crate::flowchart::edit::TextEdit;
use crate::pie;
use crate::syntax::SyntaxNode;

/// A diagram whose drawing edits are a list of items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Items {
    /// Pie chart slices.
    Pie,
}

impl Items {
    /// The item list `kind` is, if it is one.
    pub fn of(kind: Kind) -> Option<Items> {
        match kind {
            Kind::Pie => Some(Items::Pie),
            _ => None,
        }
    }

    /// The label a new item starts with.
    pub fn new_label(self) -> &'static str {
        match self {
            Items::Pie => "New slice",
        }
    }

    /// The menu items that add an item: at the end, and after the one selected item.
    pub fn add_labels(self) -> (&'static str, &'static str) {
        match self {
            Items::Pie => ("Add Slice", "Add Slice After"),
        }
    }

    /// The number of items in `src`.
    pub fn count(self, src: &str) -> usize {
        match self {
            Items::Pie => pie::edit::labels(src).len(),
        }
    }

    /// The label of item `i`, as the label field shows it.
    pub fn label(self, src: &str, i: usize) -> Option<String> {
        match self {
            Items::Pie => pie::edit::labels(src).into_iter().nth(i),
        }
    }

    /// The source of item `i`.
    pub fn range(self, src: &str, i: usize) -> Option<Range<usize>> {
        match self {
            Items::Pie => pie::edit::range(src, i),
        }
    }

    /// Shows item `i` as `text`. `None` when there is no such item, `text` can't be written, or
    /// nothing changes.
    pub fn set_label(self, src: &str, i: usize, text: &str) -> Option<TextEdit> {
        match self {
            Items::Pie => pie::edit::set_label(src, i, text),
        }
    }

    /// Adds an item shown as `text` after item `after`, or at the end. Returns its index and the
    /// edit; `None` when `text` can't be written or there is no such item.
    pub fn add(self, src: &str, after: Option<usize>, text: &str) -> Option<(usize, TextEdit)> {
        match self {
            Items::Pie => pie::edit::add(src, after, text),
        }
    }

    /// Removes items `items`. `None` when nothing is removed.
    pub fn delete(self, src: &str, items: &[usize]) -> Option<TextEdit> {
        match self {
            Items::Pie => pie::edit::delete(src, items),
        }
    }
}

/// The source of a statement.
pub(crate) fn statement_range(stmt: &SyntaxNode) -> Range<usize> {
    span(stmt.text_range())
}

/// Adds `line` on its own line after statement `after`, indented as it is; or after `last` (the
/// last statement), indented as it is or by four spaces, at the end of `src`.
pub(crate) fn insert_line(
    src: &str,
    after: Option<&SyntaxNode>,
    last: Option<&SyntaxNode>,
    line: &str,
) -> TextEdit {
    let nl = newline(src);
    let (at, pad) = match after {
        Some(s) => {
            let r = s.text_range();
            let end = usize::from(r.end());
            let at = match src[end..].find('\n') {
                Some(n) => end + n + 1,
                None => src.len(),
            };
            (at, indent(src, usize::from(r.start())).to_owned())
        }
        None => {
            let pad = last
                .map(|s| indent(src, usize::from(s.text_range().start())).to_owned())
                .filter(|p| !p.is_empty())
                .unwrap_or_else(|| "    ".to_owned());
            (src.len(), pad)
        }
    };
    let lead = if at == 0 || src[..at].ends_with('\n') {
        ""
    } else {
        nl
    };
    TextEdit {
        range: at..at,
        text: format!("{lead}{pad}{line}{nl}"),
    }
}

/// Removes statements `stmts`, each with its line when it is alone on it.
pub(crate) fn delete_statements<'a>(
    src: &str,
    stmts: impl IntoIterator<Item = &'a SyntaxNode>,
) -> Option<TextEdit> {
    let edits = (stmts.into_iter())
        .map(|s| (statement_span(src, s.text_range()), String::new()))
        .collect();
    combine(src, edits)
}
