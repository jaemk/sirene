//! Drawing edits to pie chart source (WYSIWY-28): adding, renaming, and deleting slices, named
//! by their index in [`Pie::slices`].

use std::ops::Range;

use super::{Pie, lower};
use crate::edits::{normalize, span, token};
use crate::flowchart::edit::TextEdit;
use crate::items::{delete_statements, insert_line, statement_range};
use crate::syntax::SyntaxKind::*;
use crate::syntax::SyntaxNode;

fn lowered(src: &str) -> (Pie, Vec<SyntaxNode>) {
    let (pie, _, statements) = lower::lower_statements(&super::parse(src).syntax());
    (pie, statements)
}

/// The slice labels.
pub fn labels(src: &str) -> Vec<String> {
    lowered(src).0.slices.into_iter().map(|s| s.label).collect()
}

/// The source of slice `i`: its line.
pub fn range(src: &str, i: usize) -> Option<Range<usize>> {
    lowered(src).1.get(i).map(statement_range)
}

/// A quoted label for `text`, which has no escapes: `None` when it is empty, has a line break,
/// or has both quotes.
fn quoted(text: &str) -> Option<String> {
    let text = normalize(text);
    if text.is_empty() || text.contains('\n') {
        return None;
    }
    if !text.contains('"') {
        Some(format!("\"{text}\""))
    } else if !text.contains('\'') {
        Some(format!("'{text}'"))
    } else {
        None
    }
}

/// Whether slice `i` of `src` is labeled `want`, with `count` slices in all.
fn reads(src: &str, i: usize, want: &str, count: usize) -> bool {
    let pie = lowered(src).0;
    pie.slices.len() == count && pie.slices.get(i).is_some_and(|s| s.label == want)
}

/// Labels slice `i` `text`. `None` when there is no such slice, `text` can't be written or is
/// another slice's label, or nothing changes.
pub fn set_label(src: &str, i: usize, text: &str) -> Option<TextEdit> {
    let (pie, statements) = lowered(src);
    let want = normalize(text);
    if pie.slices.get(i)?.label == want {
        return None;
    }
    let edit = TextEdit {
        range: span(token(&statements[i], STRING)?.text_range()),
        text: quoted(text)?,
    };
    reads(&edit.apply(src), i, &want, pie.slices.len()).then_some(edit)
}

/// Adds a slice labeled `text` after slice `after`, or after the last statement, with the mean
/// value (at least 1) so that it shows. Returns its index and the edit; `None` when `text` can't
/// be written or is another slice's label, or there is no such slice.
pub fn add(src: &str, after: Option<usize>, text: &str) -> Option<(usize, TextEdit)> {
    let (pie, statements) = lowered(src);
    let label = quoted(text)?;
    let n = pie.slices.len();
    let value = if n == 0 {
        1.0
    } else {
        (pie.total() / n as f64).round().max(1.0)
    };
    let i = after.map_or(n, |a| a + 1);
    let after = match after {
        Some(a) => Some(statements.get(a)?),
        None => None,
    };
    let last = super::parse(src).syntax().children().last();
    let edit = insert_line(src, after, last.as_ref(), &format!("{label} : {value}"));
    reads(&edit.apply(src), i, &normalize(text), n + 1).then_some((i, edit))
}

/// Removes slices `slices`. `None` when nothing is removed.
pub fn delete(src: &str, slices: &[usize]) -> Option<TextEdit> {
    let statements = lowered(src).1;
    delete_statements(src, slices.iter().filter_map(|&i| statements.get(i)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "pie title Pets\n    \"Dogs\" : 386\n    \"Cats\" : 85\n    'Rats' : 15\n";

    #[test]
    fn ranges_and_labels() {
        assert_eq!(&SRC[range(SRC, 1).unwrap()], "\"Cats\" : 85");
        assert_eq!(labels(SRC), ["Dogs", "Cats", "Rats"]);
        assert!(range(SRC, 3).is_none());
    }

    #[test]
    fn renames() {
        let out = set_label(SRC, 2, "Mice").unwrap().apply(SRC);
        assert!(out.contains("    \"Mice\" : 15\n"), "{out}");
        let out = set_label(SRC, 0, "Big \"dogs\"").unwrap().apply(SRC);
        assert!(out.contains("    'Big \"dogs\"' : 386\n"), "{out}");
        assert!(set_label(SRC, 0, "Cats").is_none());
        assert!(set_label(SRC, 0, "Dogs").is_none());
        assert!(set_label(SRC, 0, "a\nb").is_none());
        assert!(set_label(SRC, 0, "a\"'").is_none());
        assert!(set_label(SRC, 0, " ").is_none());
        assert!(set_label(SRC, 9, "x").is_none());
    }

    #[test]
    fn adds() {
        let (i, e) = add(SRC, Some(0), "Fish").unwrap();
        assert_eq!(i, 1);
        let out = e.apply(SRC);
        assert!(
            out.contains("\"Dogs\" : 386\n    \"Fish\" : 162\n    \"Cats\""),
            "{out}"
        );
        let (i, e) = add(SRC, None, "Fish").unwrap();
        assert_eq!(i, 3);
        assert!(e.apply(SRC).ends_with("'Rats' : 15\n    \"Fish\" : 162\n"));
        assert!(add(SRC, None, "Cats").is_none());
        assert!(add(SRC, Some(5), "x").is_none());
        let (i, e) = add("pie\n", None, "A").unwrap();
        assert_eq!(
            (i, e.apply("pie\n")),
            (0, "pie\n    \"A\" : 1\n".to_owned())
        );
    }

    #[test]
    fn deletes() {
        let out = delete(SRC, &[0, 2]).unwrap().apply(SRC);
        assert_eq!(out, "pie title Pets\n    \"Cats\" : 85\n");
        assert!(delete(SRC, &[7]).is_none());
    }
}
