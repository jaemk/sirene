//! Drawing edits to kanban board source (WYSIWY-28): adding, renaming, and deleting columns and
//! cards, named by their place in order: a column, then its cards.

use std::ops::Range;

use super::{Kanban, lower};
use crate::edits::{combine, indent, line_start, newline, normalize};
use crate::flowchart::edit::TextEdit;
use crate::items::statement_range;
use crate::mindmap::edit::{label_sources, node_sources};
use crate::syntax::SyntaxKind::*;
use crate::syntax::SyntaxNode;

/// A parsed source, its board, and the statement of each column and card.
struct Doc<'a> {
    src: &'a str,
    root: SyntaxNode,
    board: Kanban,
    statements: Vec<SyntaxNode>,
}

impl<'a> Doc<'a> {
    fn new(src: &'a str) -> Doc<'a> {
        let root = super::parse(src).syntax();
        let (board, _, statements) = lower::lower_statements(&root);
        Doc {
            src,
            root,
            board,
            statements,
        }
    }

    fn start(&self, i: usize) -> usize {
        usize::from(self.statements[i].text_range().start())
    }

    /// The byte after item `i`'s line and the `::icon` and `:::` lines after it.
    fn end(&self, i: usize) -> usize {
        let mut end = usize::from(self.statements[i].text_range().end());
        for s in self.root.children() {
            if usize::from(s.text_range().start()) < end {
                continue;
            }
            if !matches!(s.kind(), ICON_STMT | CLASS_SHORTHAND) {
                break;
            }
            end = usize::from(s.text_range().end());
        }
        match self.src[end..].find('\n') {
            Some(n) => end + n + 1,
            None => self.src.len(),
        }
    }

    /// The indentation of a card in the column of item `i`: its own for a card, else the first
    /// card's, else the column's and a step.
    fn card_indent(&self, items: &[(bool, String)], i: usize) -> String {
        let own = indent(self.src, self.start(i));
        if !items[i].0 {
            own.to_owned()
        } else if items.get(i + 1).is_some_and(|(column, _)| !column) {
            indent(self.src, self.start(i + 1)).to_owned()
        } else {
            let step = if own.contains('\t') { "\t" } else { "  " };
            format!("{own}{step}")
        }
    }
}

/// Each column and card in order: whether it is a column, and its label.
fn items(board: &Kanban) -> Vec<(bool, String)> {
    let mut out = Vec::new();
    for c in &board.columns {
        out.push((true, c.label.text.clone()));
        out.extend(c.cards.iter().map(|card| (false, card.label.text.clone())));
    }
    out
}

fn read(src: &str) -> Vec<(bool, String)> {
    items(&super::parse(src).lower().0)
}

/// The last item of item `i`'s column when `i` is a column, else `i`.
fn last_of(items: &[(bool, String)], i: usize) -> usize {
    if !items[i].0 {
        return i;
    }
    let mut j = i;
    while items.get(j + 1).is_some_and(|(column, _)| !column) {
        j += 1;
    }
    j
}

/// The column and card labels, in order.
pub fn labels(src: &str) -> Vec<String> {
    let doc = Doc::new(src);
    items(&doc.board).into_iter().map(|(_, l)| l).collect()
}

/// The source of item `i`: its line.
pub fn range(src: &str, i: usize) -> Option<Range<usize>> {
    Doc::new(src).statements.get(i).map(statement_range)
}

/// Shows item `i` as `text`, keeping its shape, id, and metadata. `None` when there is no such
/// item, the text is empty or would not read back, or nothing changes.
pub fn set_label(src: &str, i: usize, text: &str) -> Option<TextEdit> {
    let want = normalize(text);
    if want.is_empty() {
        return None;
    }
    let doc = Doc::new(src);
    let mut expect = items(&doc.board);
    if expect.get(i)?.1 == want {
        return None;
    }
    expect[i].1 = want;
    let (range, sources) = label_sources(&doc.statements[i], text)?;
    sources.into_iter().find_map(|text| {
        let edit = TextEdit {
            range: range.clone(),
            text,
        };
        (read(&edit.apply(src)) == expect).then_some(edit)
    })
}

/// Adds a card shown as `text`: after item `after` (first in it, for a column), or last in the
/// last column; a column in an empty board. Returns its index and the edit; `None` when there is
/// no such item or the text is empty or would not read back.
pub fn add(src: &str, after: Option<usize>, text: &str) -> Option<(usize, TextEdit)> {
    let want = normalize(text);
    if want.is_empty() {
        return None;
    }
    let doc = Doc::new(src);
    let mut expect = items(&doc.board);
    let n = expect.len();
    let (at, pad, i, column) = match after {
        None if n == 0 => (src.len(), "  ".to_owned(), 0, true),
        None => (doc.end(n - 1), doc.card_indent(&expect, n - 1), n, false),
        Some(a) => {
            expect.get(a)?;
            (doc.end(a), doc.card_indent(&expect, a), a + 1, false)
        }
    };
    expect.insert(i, (column, want));
    let nl = newline(src);
    let lead = if at == 0 || src[..at].ends_with('\n') {
        ""
    } else {
        nl
    };
    node_sources(text).into_iter().find_map(|line| {
        let edit = TextEdit {
            range: at..at,
            text: format!("{lead}{pad}{line}{nl}"),
        };
        (read(&edit.apply(src)) == expect).then_some((i, edit))
    })
}

/// Removes items `remove`, a column with its cards. `None` when nothing is removed.
pub fn delete(src: &str, remove: &[usize]) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let all = items(&doc.board);
    let edits = (remove.iter().filter(|&&i| i < all.len()))
        .map(|&i| {
            let start = line_start(src, doc.start(i));
            (start..doc.end(last_of(&all, i)), String::new())
        })
        .collect();
    combine(src, edits)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "kanban\n  todo[Todo]\n    a[Write docs]@{ ticket: 1 }\n    b[Test]\n  done[Done]\n    c[Ship]\n    ::icon(x)\n";

    #[test]
    fn ranges_and_labels() {
        assert_eq!(labels(SRC), ["Todo", "Write docs", "Test", "Done", "Ship"]);
        assert_eq!(&SRC[range(SRC, 3).unwrap()], "done[Done]");
    }

    #[test]
    fn renames() {
        let out = set_label(SRC, 1, "Read docs").unwrap().apply(SRC);
        assert!(
            out.contains("    a[\"Read docs\"]@{ ticket: 1 }\n"),
            "{out}"
        );
        let out = set_label(SRC, 0, "Later").unwrap().apply(SRC);
        assert!(out.contains("  todo[\"Later\"]\n"), "{out}");
        assert!(set_label(SRC, 1, "Write docs").is_none());
        assert!(set_label(SRC, 1, " ").is_none());
        assert!(set_label(SRC, 9, "x").is_none());
    }

    #[test]
    fn adds() {
        let (i, e) = add(SRC, Some(3), "Review").unwrap();
        assert_eq!(i, 4);
        assert!(
            e.apply(SRC)
                .contains("  done[Done]\n    Review\n    c[Ship]")
        );
        let (i, e) = add(SRC, Some(4), "Tag").unwrap();
        assert_eq!(i, 5);
        assert!(e.apply(SRC).ends_with("    ::icon(x)\n    Tag\n"));
        let (i, e) = add(SRC, None, "Tag").unwrap();
        assert_eq!((i, e.apply(SRC)), (5, format!("{SRC}    Tag\n")));
        let src = "kanban\n  todo\n";
        assert_eq!(
            add(src, None, "x").unwrap().1.apply(src),
            "kanban\n  todo\n    x\n"
        );
        let (i, e) = add("kanban\n", None, "Todo").unwrap();
        assert_eq!((i, e.apply("kanban\n")), (0, "kanban\n  Todo\n".to_owned()));
        assert!(add(SRC, Some(9), "x").is_none());
    }

    #[test]
    fn deletes() {
        let out = delete(SRC, &[4]).unwrap().apply(SRC);
        assert_eq!(
            out,
            "kanban\n  todo[Todo]\n    a[Write docs]@{ ticket: 1 }\n    b[Test]\n  done[Done]\n"
        );
        let out = delete(SRC, &[0, 1]).unwrap().apply(SRC);
        assert_eq!(out, "kanban\n  done[Done]\n    c[Ship]\n    ::icon(x)\n");
        assert!(delete(SRC, &[9]).is_none());
    }
}
