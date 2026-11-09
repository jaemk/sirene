//! Collects a timeline's sections, periods, and events (TIMELI-2).

use super::{Direction, Period, Section, Text, Timeline};
use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxNode, SyntaxToken};

pub(crate) fn lower(root: &SyntaxNode) -> (Timeline, Vec<Diagnostic>) {
    let mut t = Timeline::default();
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
            HEADER => {
                if token(&stmt, DIR).is_some_and(|d| d.text() == "TD") {
                    t.direction = Direction::TopDown;
                }
            }
            TITLE_STMT => {
                title = text_of(&stmt, TEXT).filter(|s| !s.is_empty());
            }
            SECTION_STMT => match text_of(&stmt, TEXT).filter(|s| !s.is_empty()) {
                Some(name) => t.sections.push(Section {
                    name: Some(lines(&name)),
                    periods: Vec::new(),
                }),
                None => diagnostics.push(Diagnostic::new(
                    stmt.text_range(),
                    "expected a section name",
                )),
            },
            PERIOD_STMT => {
                let label = token(&stmt, PERIOD).map_or_else(Vec::new, |p| lines(p.text()));
                let events = events(&stmt, &mut diagnostics);
                if t.sections.is_empty() {
                    t.sections.push(Section::default());
                }
                let section = t.sections.last_mut().expect("one section");
                section.periods.push(Period { label, events });
            }
            EVENTS_STMT => {
                let events = events(&stmt, &mut diagnostics);
                match t.sections.last_mut().and_then(|s| s.periods.last_mut()) {
                    Some(p) => p.events.extend(events),
                    None => diagnostics.push(Diagnostic::new(
                        stmt.text_range(),
                        "an event needs a time period before it",
                    )),
                }
            }
            ACC_TITLE_STMT => t.acc_title = text_of(&stmt, ACC_TEXT),
            ACC_DESCR_STMT => t.acc_descr = text_of(&stmt, ACC_TEXT),
            _ => {}
        }
    }
    t.title = title;
    (t, diagnostics)
}

fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == kind)
}

fn text_of(node: &SyntaxNode, kind: SyntaxKind) -> Option<String> {
    token(node, kind).map(|t| t.text().trim().to_owned())
}

/// `s` split into lines at `<br>` (any case, with an optional `/`).
fn lines(s: &str) -> Text {
    let mut out = Vec::new();
    let mut rest = s;
    loop {
        let lower = rest.to_ascii_lowercase();
        let found = ["<br>", "<br/>", "<br />"]
            .iter()
            .filter_map(|b| lower.find(b).map(|i| (i, b.len())))
            .min();
        match found {
            Some((i, n)) => {
                out.push(rest[..i].trim().to_owned());
                rest = &rest[i + n..];
            }
            None => {
                out.push(rest.trim().to_owned());
                return out;
            }
        }
    }
}

/// A statement's events; an empty one is a warning and skipped.
fn events(stmt: &SyntaxNode, diagnostics: &mut Vec<Diagnostic>) -> Vec<Text> {
    let mut out = Vec::new();
    let mut tokens = stmt
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .peekable();
    while let Some(t) = tokens.next() {
        if t.kind() != COLON {
            continue;
        }
        while tokens.peek().is_some_and(|n| n.kind() == WHITESPACE) {
            tokens.next();
        }
        match tokens.peek() {
            Some(e) if e.kind() == EVENT => {
                out.push(lines(e.text()));
            }
            _ => diagnostics.push(Diagnostic::warning(
                t.text_range(),
                "skipping an empty event",
            )),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breaks() {
        assert_eq!(lines("a<br>b<BR/>c <br /> d"), ["a", "b", "c", "d"]);
        assert_eq!(lines("plain"), ["plain"]);
    }
}
