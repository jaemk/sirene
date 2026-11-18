//! Drawing edits to user journey source (WYSIWY-28): adding, renaming, and deleting tasks,
//! named by their place among every section's tasks, in order.

use std::ops::Range;

use super::{Journey, Task, lower};
use crate::edits::{normalize, span, token};
use crate::flowchart::edit::TextEdit;
use crate::items::{delete_statements, insert_line, statement_range};
use crate::syntax::SyntaxKind::*;
use crate::syntax::SyntaxNode;

fn lowered(src: &str) -> (Journey, Vec<SyntaxNode>) {
    let (j, _, statements) = lower::lower_statements(&super::parse(src).syntax());
    (j, statements)
}

fn tasks(j: &Journey) -> Vec<&Task> {
    j.sections.iter().flat_map(|s| &s.tasks).collect()
}

/// The task names, in order.
pub fn labels(src: &str) -> Vec<String> {
    let j = lowered(src).0;
    tasks(&j).into_iter().map(|t| t.name.clone()).collect()
}

/// The source of task `i`: its line.
pub fn range(src: &str, i: usize) -> Option<Range<usize>> {
    lowered(src).1.get(i).map(statement_range)
}

/// A task name for `text`, which has no escapes: `None` when it is empty or has a line break,
/// `#`, `:`, or `;`.
fn name(text: &str) -> Option<String> {
    let text = normalize(text);
    let ok = !text.is_empty() && !text.contains(['\n', '#', ':', ';']);
    ok.then_some(text)
}

/// Whether task `i` of `src` is named `want`, with `count` tasks in all.
fn reads(src: &str, i: usize, want: &str, count: usize) -> bool {
    let j = lowered(src).0;
    let tasks = tasks(&j);
    tasks.len() == count && tasks.get(i).is_some_and(|t| t.name == want)
}

/// Names task `i` `text`. `None` when there is no such task, `text` can't be written, or
/// nothing changes.
pub fn set_label(src: &str, i: usize, text: &str) -> Option<TextEdit> {
    let (j, statements) = lowered(src);
    let count = tasks(&j).len();
    let want = name(text)?;
    if tasks(&j).get(i)?.name == want {
        return None;
    }
    let edit = TextEdit {
        range: span(token(&statements[i], TASK_NAME)?.text_range()),
        text: want.clone(),
    };
    reads(&edit.apply(src), i, &want, count).then_some(edit)
}

/// Adds a task named `text` after task `after`, with its score and actors, or at the end with a
/// score of 3. Returns its index and the edit; `None` when `text` can't be written or there is
/// no such task.
pub fn add(src: &str, after: Option<usize>, text: &str) -> Option<(usize, TextEdit)> {
    let (j, statements) = lowered(src);
    let count = tasks(&j).len();
    let want = name(text)?;
    let i = after.map_or(count, |a| a + 1);
    let (after, data) = match after {
        Some(a) => {
            let stmt = statements.get(a)?;
            let data = token(stmt, TASK_DATA).map(|t| t.text().trim().to_owned());
            (Some(stmt), data.filter(|d| !d.is_empty()))
        }
        None => (None, None),
    };
    let data = data.unwrap_or_else(|| "3".to_owned());
    let last = super::parse(src).syntax().children().last();
    let edit = insert_line(src, after, last.as_ref(), &format!("{want}: {data}"));
    reads(&edit.apply(src), i, &want, count + 1).then_some((i, edit))
}

/// Removes tasks `tasks`. `None` when nothing is removed.
pub fn delete(src: &str, tasks: &[usize]) -> Option<TextEdit> {
    let statements = lowered(src).1;
    delete_statements(src, tasks.iter().filter_map(|&i| statements.get(i)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "journey\n  title My day\n  section Go to work\n    Make tea: 5: Me\n    Go upstairs: 3: Me, Cat\n  section Go home\n    Sit down: 2\n";

    #[test]
    fn ranges_and_labels() {
        assert_eq!(labels(SRC), ["Make tea", "Go upstairs", "Sit down"]);
        assert_eq!(&SRC[range(SRC, 2).unwrap()], "Sit down: 2");
    }

    #[test]
    fn renames() {
        let out = set_label(SRC, 1, "Go downstairs").unwrap().apply(SRC);
        assert!(out.contains("    Go downstairs: 3: Me, Cat\n"), "{out}");
        assert!(set_label(SRC, 1, "a: b").is_none());
        assert!(set_label(SRC, 1, "a # b").is_none());
        assert!(set_label(SRC, 1, "section x").is_none());
        assert!(set_label(SRC, 1, "Go upstairs").is_none());
        assert!(set_label(SRC, 7, "x").is_none());
    }

    #[test]
    fn adds_and_deletes() {
        let (i, e) = add(SRC, Some(1), "Rest").unwrap();
        assert_eq!(i, 2);
        let out = e.apply(SRC);
        assert!(
            out.contains("Me, Cat\n    Rest: 3: Me, Cat\n  section Go home"),
            "{out}"
        );
        let (i, e) = add(SRC, None, "Sleep").unwrap();
        assert_eq!(i, 3);
        assert!(e.apply(SRC).ends_with("    Sit down: 2\n    Sleep: 3\n"));
        let (i, e) = add("journey\n", None, "A").unwrap();
        assert_eq!(
            (i, e.apply("journey\n")),
            (0, "journey\n    A: 3\n".to_owned())
        );
        let out = delete(SRC, &[0, 2]).unwrap().apply(SRC);
        assert_eq!(
            out,
            "journey\n  title My day\n  section Go to work\n    Go upstairs: 3: Me, Cat\n  section Go home\n"
        );
    }
}
