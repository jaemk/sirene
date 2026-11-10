//! Collects a journey's sections and scored tasks (MJOUR-2).

use super::{Journey, MAX_SCORE, MIN_SCORE, Section, Task};
use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxNode, SyntaxToken};

pub(crate) fn lower(root: &SyntaxNode) -> (Journey, Vec<Diagnostic>) {
    let mut j = Journey::default();
    let mut diagnostics = Vec::new();
    let mut title = None;
    for el in root.children_with_tokens() {
        if el.kind() == FRONTMATTER
            && let Some(tok) = el.as_token()
        {
            title = text::frontmatter_title(tok.text());
        }
    }
    for stmt in root.children() {
        match stmt.kind() {
            TITLE_STMT => title = text_of(&stmt, TEXT).filter(|s| !s.is_empty()),
            SECTION_STMT => match text_of(&stmt, TEXT).filter(|s| !s.is_empty()) {
                Some(name) => j.sections.push(Section {
                    name: Some(name),
                    tasks: Vec::new(),
                }),
                None => diagnostics.push(Diagnostic::new(
                    stmt.text_range(),
                    "expected a section name",
                )),
            },
            TASK_STMT => {
                if token(&stmt, COLON).is_none() {
                    continue;
                }
                if let Some(task) = task(&stmt, &mut diagnostics) {
                    if j.sections.is_empty() {
                        j.sections.push(Section::default());
                    }
                    j.sections.last_mut().expect("one section").tasks.push(task);
                }
            }
            ACC_TITLE_STMT => j.acc_title = text_of(&stmt, ACC_TEXT),
            ACC_DESCR_STMT => j.acc_descr = text_of(&stmt, ACC_TEXT),
            _ => {}
        }
    }
    j.title = title;
    (j, diagnostics)
}

/// A task from its name and `score: actor, actor` data.
fn task(stmt: &SyntaxNode, diagnostics: &mut Vec<Diagnostic>) -> Option<Task> {
    let name = text_of(stmt, TASK_NAME).unwrap_or_default();
    let Some(data) = token(stmt, TASK_DATA) else {
        diagnostics.push(Diagnostic::new(
            stmt.text_range(),
            format!("`{name}` has no score; expected a number from 1 to 5"),
        ));
        return None;
    };
    let mut parts = data.text().split(':');
    let raw = parts.next().unwrap_or_default().trim();
    let Some(mut score) = raw.parse::<f64>().ok().filter(|s| s.is_finite()) else {
        diagnostics.push(Diagnostic::new(
            data.text_range(),
            format!("`{raw}` is not a score; expected a number from 1 to 5"),
        ));
        return None;
    };
    if !(MIN_SCORE..=MAX_SCORE).contains(&score) {
        let clamped = score.clamp(MIN_SCORE, MAX_SCORE);
        diagnostics.push(Diagnostic::warning(
            data.text_range(),
            format!("`{raw}` is outside the scores 1 to 5; drawing it as {clamped}"),
        ));
        score = clamped;
    }
    let actors = parts
        .next()
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(str::to_owned)
        .collect();
    let rest: Vec<&str> = parts.collect();
    if !rest.is_empty() {
        diagnostics.push(Diagnostic::warning(
            data.text_range(),
            format!("ignoring `:{}` after the actors", rest.join(":")),
        ));
    }
    Some(Task {
        name,
        score,
        actors,
    })
}

fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == kind)
}

fn text_of(node: &SyntaxNode, kind: SyntaxKind) -> Option<String> {
    token(node, kind).map(|t| t.text().trim().to_owned())
}
