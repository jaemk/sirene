//! Drawing edits to gantt chart source (WYSIWY-28): adding, renaming, and deleting tasks, named
//! by their place among every section's tasks, in order.

use std::ops::Range;

use super::{Gantt, lower, time};
use crate::edits::{normalize, span, token};
use crate::flowchart::edit::TextEdit;
use crate::items::{delete_statements, insert_line, statement_range};
use crate::syntax::SyntaxKind::*;
use crate::syntax::SyntaxNode;

fn lowered(src: &str) -> (Gantt, Vec<SyntaxNode>) {
    let (g, _, statements) = lower::lower_statements(&super::parse(src).syntax(), super::now());
    (g, statements)
}

/// The task names, in order.
pub fn labels(src: &str) -> Vec<String> {
    lowered(src).0.tasks().map(|t| t.name.clone()).collect()
}

/// The source of task `i`: its line.
pub fn range(src: &str, i: usize) -> Option<Range<usize>> {
    lowered(src).1.get(i).map(statement_range)
}

/// A task name for `text`, which has no escapes: `None` when it is empty or has a line break,
/// `:`, `#`, or `;`.
fn name(text: &str) -> Option<String> {
    let text = normalize(text);
    let ok = !text.is_empty() && !text.contains(['\n', ':', '#', ';']);
    ok.then_some(text)
}

/// The task names of `src`.
fn names(g: &Gantt) -> Vec<&str> {
    g.tasks().map(|t| t.name.as_str()).collect()
}

/// Names task `i` `text`. `None` when there is no such task, `text` can't be written, or
/// nothing changes.
pub fn set_label(src: &str, i: usize, text: &str) -> Option<TextEdit> {
    let (g, statements) = lowered(src);
    let mut want = names(&g);
    let new = name(text)?;
    if *want.get(i)? == new {
        return None;
    }
    want[i] = &new;
    let edit = TextEdit {
        range: span(token(&statements[i], TASK_NAME)?.text_range()),
        text: new.clone(),
    };
    (names(&lowered(&edit.apply(src)).0) == want).then_some(edit)
}

/// Adds a one-day task named `text` after task `after`, or at the end, starting where the task
/// before it ends (or today in an empty chart). Returns its index and the edit; `None` when
/// `text` can't be written or there is no such task.
pub fn add(src: &str, after: Option<usize>, text: &str) -> Option<(usize, TextEdit)> {
    let (g, statements) = lowered(src);
    let new = name(text)?;
    let n = statements.len();
    let i = after.map_or(n, |a| a + 1);
    let after = match after {
        Some(a) => Some(statements.get(a)?),
        None => None,
    };
    let data = if n == 0 {
        let today = time::format_dayjs(time::day_start(g.now), &g.date_format);
        format!("{today}, 1d")
    } else {
        "1d".to_owned()
    };
    let last = super::parse(src).syntax().children().last();
    let edit = insert_line(src, after, last.as_ref(), &format!("{new} : {data}"));
    let mut want = names(&g);
    want.insert(i, &new);
    (names(&lowered(&edit.apply(src)).0) == want).then_some((i, edit))
}

/// Removes tasks `tasks`. `None` when nothing is removed, or when another task depends on one
/// of them (`after ID`) and would no longer be drawn.
pub fn delete(src: &str, tasks: &[usize]) -> Option<TextEdit> {
    let (g, statements) = lowered(src);
    let edit = delete_statements(src, tasks.iter().filter_map(|&i| statements.get(i)))?;
    let want: Vec<&str> = (names(&g).into_iter().enumerate())
        .filter(|(i, _)| !tasks.contains(i))
        .map(|(_, n)| n)
        .collect();
    (names(&lowered(&edit.apply(src)).0) == want).then_some(edit)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "gantt\n    dateFormat YYYY-MM-DD\n    section A\n    Design :a1, 2014-01-01, 3d\n    Build :after a1, 2d\n    section B\n    Test : 2014-01-10, 1d\n";

    #[test]
    fn ranges_and_labels() {
        assert_eq!(labels(SRC), ["Design", "Build", "Test"]);
        assert_eq!(&SRC[range(SRC, 1).unwrap()], "Build :after a1, 2d");
    }

    #[test]
    fn renames() {
        let out = set_label(SRC, 0, "Plan").unwrap().apply(SRC);
        assert!(out.contains("    Plan :a1, 2014-01-01, 3d\n"), "{out}");
        assert!(set_label(SRC, 0, "a: b").is_none());
        assert!(set_label(SRC, 0, "section x").is_none());
        assert!(set_label(SRC, 0, "Design").is_none());
        assert!(set_label(SRC, 5, "x").is_none());
    }

    #[test]
    fn adds() {
        let (i, e) = add(SRC, Some(1), "Ship").unwrap();
        assert_eq!(i, 2);
        let out = e.apply(SRC);
        assert!(
            out.contains("after a1, 2d\n    Ship : 1d\n    section B"),
            "{out}"
        );
        let (i, e) = add(SRC, None, "Ship").unwrap();
        assert_eq!(i, 3);
        assert!(e.apply(SRC).ends_with("1d\n    Ship : 1d\n"));
        let src = "gantt\n    dateFormat YYYY-MM-DD\n";
        let (i, e) = add(src, None, "First").unwrap();
        assert_eq!(i, 0);
        assert_eq!(labels(&e.apply(src)), ["First"]);
    }

    #[test]
    fn deletes() {
        let out = delete(SRC, &[1, 2]).unwrap().apply(SRC);
        assert!(out.ends_with("2014-01-01, 3d\n    section B\n"), "{out}");
        // `Build` is after `a1`.
        assert!(delete(SRC, &[0]).is_none());
        assert!(delete(SRC, &[0, 1]).is_some());
        assert!(delete(SRC, &[9]).is_none());
    }
}
