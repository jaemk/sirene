//! Drawing edits to quadrant chart source (WYSIWY-28): adding, renaming, and deleting points,
//! named by their index in [`QuadrantChart::points`].

use std::ops::Range;

use super::{QuadrantChart, lower};
use crate::edits::{normalize, span};
use crate::flowchart::edit::TextEdit;
use crate::items::{delete_statements, insert_line, statement_range};
use crate::syntax::SyntaxKind::*;
use crate::syntax::SyntaxNode;

fn lowered(src: &str) -> (QuadrantChart, Vec<SyntaxNode>) {
    let (q, _, statements) = lower::lower_statements(&super::parse(src).syntax());
    (q, statements)
}

/// The point names.
pub fn labels(src: &str) -> Vec<String> {
    lowered(src).0.points.into_iter().map(|p| p.name).collect()
}

/// The source of point `i`: its line.
pub fn range(src: &str, i: usize) -> Option<Range<usize>> {
    lowered(src).1.get(i).map(statement_range)
}

/// Names for `text` to try in order: as written, then quoted. Names have no escapes or line
/// breaks.
fn sources(text: &str) -> Vec<String> {
    let text = normalize(text);
    if text.is_empty() || text.contains('\n') {
        return Vec::new();
    }
    let mut out = vec![text.clone()];
    if !text.contains('"') {
        out.push(format!("\"{text}\""));
    }
    out
}

/// Whether point `i` of `src` is named `want`, with `count` points in all.
fn reads(src: &str, i: usize, want: &str, count: usize) -> bool {
    let q = lowered(src).0;
    q.points.len() == count && q.points.get(i).is_some_and(|p| p.name == want)
}

/// Names point `i` `text`. `None` when there is no such point, `text` can't be written, or
/// nothing changes.
pub fn set_label(src: &str, i: usize, text: &str) -> Option<TextEdit> {
    let (q, statements) = lowered(src);
    let want = normalize(text);
    if q.points.get(i)?.name == want {
        return None;
    }
    let name = (statements[i].children_with_tokens())
        .filter_map(|e| e.into_token())
        .find(|t| matches!(t.kind(), TEXT | STRING))?;
    let range = span(name.text_range());
    sources(text).into_iter().find_map(|text| {
        let edit = TextEdit {
            range: range.clone(),
            text,
        };
        reads(&edit.apply(src), i, &want, q.points.len()).then_some(edit)
    })
}

/// Adds a point named `text` in the middle of the chart, after point `after` or after the last
/// statement. Returns its index and the edit; `None` when `text` can't be written or there is no
/// such point.
pub fn add(src: &str, after: Option<usize>, text: &str) -> Option<(usize, TextEdit)> {
    let (q, statements) = lowered(src);
    let n = q.points.len();
    let i = after.map_or(n, |a| a + 1);
    let after = match after {
        Some(a) => Some(statements.get(a)?),
        None => None,
    };
    let last = super::parse(src).syntax().children().last();
    let want = normalize(text);
    sources(text).into_iter().find_map(|name| {
        let line = format!("{name}: [0.5, 0.5]");
        let edit = insert_line(src, after, last.as_ref(), &line);
        reads(&edit.apply(src), i, &want, n + 1).then_some((i, edit))
    })
}

/// Removes points `points`. `None` when nothing is removed.
pub fn delete(src: &str, points: &[usize]) -> Option<TextEdit> {
    let statements = lowered(src).1;
    delete_statements(src, points.iter().filter_map(|&i| statements.get(i)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "quadrantChart\n    x-axis Low --> High\n    Campaign A: [0.3, 0.6]\n    \"B: two\":::hot: [0.45, 0.23] radius: 9\n";

    #[test]
    fn ranges_and_labels() {
        assert_eq!(labels(SRC), ["Campaign A", "B: two"]);
        assert_eq!(&SRC[range(SRC, 0).unwrap()], "Campaign A: [0.3, 0.6]");
        assert!(range(SRC, 2).is_none());
    }

    #[test]
    fn renames() {
        let out = set_label(SRC, 0, "Campaign Z").unwrap().apply(SRC);
        assert!(out.contains("    Campaign Z: [0.3, 0.6]\n"), "{out}");
        let out = set_label(SRC, 0, "a: b").unwrap().apply(SRC);
        assert!(out.contains("    \"a: b\": [0.3, 0.6]\n"), "{out}");
        assert_eq!(labels(&out)[0], "a: b");
        let out = set_label(SRC, 1, "C").unwrap().apply(SRC);
        assert!(
            out.contains("    C:::hot: [0.45, 0.23] radius: 9\n"),
            "{out}"
        );
        assert!(set_label(SRC, 0, "Campaign A").is_none());
        assert!(set_label(SRC, 0, "a\nb").is_none());
        assert!(set_label(SRC, 0, "").is_none());
        assert!(set_label(SRC, 5, "x").is_none());
    }

    #[test]
    fn adds_and_deletes() {
        let (i, e) = add(SRC, Some(0), "New point").unwrap();
        assert_eq!(i, 1);
        let out = e.apply(SRC);
        assert!(
            out.contains("[0.3, 0.6]\n    New point: [0.5, 0.5]\n    \"B: two\""),
            "{out}"
        );
        let (i, e) = add(SRC, None, "x;y").unwrap();
        assert_eq!(i, 2);
        assert!(
            e.apply(SRC)
                .ends_with("radius: 9\n    \"x;y\": [0.5, 0.5]\n")
        );
        let out = delete(SRC, &[1]).unwrap().apply(SRC);
        assert!(out.ends_with("[0.3, 0.6]\n"), "{out}");
        assert!(delete(SRC, &[4]).is_none());
    }
}
