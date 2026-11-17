//! Source edits for changes made on a state diagram's drawing (WYSIWY-22).
//!
//! States are named by their node id in the lowered chart and transitions by their index among
//! its edges, as [`flowchart::edit`](crate::flowchart::edit) names them. A start or end state
//! (`[*]`) has the id `CONTAINER:[*]start` or `CONTAINER:[*]end`, with an empty container for the
//! diagram itself.

use std::collections::HashSet;
use std::ops::Range;

use crate::flowchart::Flowchart;
use crate::flowchart::edit::TextEdit;
use crate::sequence::edit::{indent, line_start, span, statement_span};
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxNode, SyntaxToken};

/// Why [`add_transition`] refused a transition.
pub const TRANSITION_REFUSED: &str = "a transition cannot go into a start state, out of an end state, or between the start and end states of different composite states";

/// A parsed source and the chart it lowers to.
struct Doc<'a> {
    src: &'a str,
    root: SyntaxNode,
    chart: Flowchart,
    newline: &'static str,
}

impl<'a> Doc<'a> {
    fn new(src: &'a str) -> Self {
        let parse = super::parse(src);
        Doc {
            src,
            root: parse.syntax(),
            chart: parse.lower().flowchart,
            newline: if src.contains("\r\n") { "\r\n" } else { "\n" },
        }
    }

    /// Every statement without a syntax error, at any depth, in source order.
    fn statements(&self) -> impl Iterator<Item = SyntaxNode> + '_ {
        self.root.descendants().filter(|s| {
            matches!(
                s.kind(),
                STATE_STMT
                    | DESCRIPTION_STMT
                    | TRANSITION_STMT
                    | NOTE_STMT
                    | CLASS_STMT
                    | STYLE_STMT
            ) && !s.descendants().any(|n| n.kind() == ERROR)
        })
    }

    /// The statement of transition `edge`.
    fn transition(&self, edge: usize) -> Option<SyntaxNode> {
        let range = self.chart.edges.get(edge)?.range;
        self.root
            .descendants()
            .find(|s| s.kind() == TRANSITION_STMT && s.text_range() == range)
    }

    /// Adds `line` at the end of the diagram, indented as its last statement.
    fn append(&self, line: &str) -> TextEdit {
        let src = self.src;
        let indent = self
            .root
            .children()
            .last()
            .map(|s| indent(src, usize::from(s.text_range().start())).to_owned())
            .filter(|i| !i.is_empty())
            .unwrap_or_else(|| "    ".to_owned());
        let sep = if src.is_empty() || src.ends_with('\n') {
            ""
        } else {
            self.newline
        };
        TextEdit {
            range: src.len()..src.len(),
            text: format!("{sep}{indent}{line}{}", self.newline),
        }
    }

    /// Adds `line` as the last statement of composite state `id`.
    fn append_in(&self, id: &str, line: &str) -> Option<TextEdit> {
        let composite = self
            .root
            .descendants()
            .find(|c| c.kind() == COMPOSITE && token(c, IDENT).is_some_and(|t| t.text() == id))?;
        let close = token(&composite, R_BRACE)?;
        let src = self.src;
        let at = usize::from(close.text_range().start());
        let inner = composite
            .children()
            .last()
            .map(|s| indent(src, usize::from(s.text_range().start())).to_owned())
            .unwrap_or_else(|| {
                format!(
                    "{}    ",
                    indent(src, usize::from(composite.text_range().start()))
                )
            });
        let nl = self.newline;
        Some(if src[line_start(src, at)..at].trim().is_empty() {
            let start = line_start(src, at);
            TextEdit {
                range: start..start,
                text: format!("{inner}{line}{nl}"),
            }
        } else {
            TextEdit {
                range: at..at,
                text: format!("{nl}{inner}{line}{nl}"),
            }
        })
    }

    /// A state id not yet in use: `S1`, `S2`, ...
    fn fresh_id(&self) -> String {
        let used: HashSet<&str> = (self.chart.nodes.iter().map(|n| n.id.as_str()))
            .chain(self.chart.subgraphs.iter().map(|s| s.id.as_str()))
            .collect();
        // One of the first `used.len() + 1` is free.
        (1..=used.len() + 1)
            .map(|n| format!("S{n}"))
            .find(|id| !used.contains(id.as_str()))
            .unwrap_or_default()
    }
}

/// A transition end: a named state, or the start or end state of a container.
enum End<'a> {
    State(&'a str),
    Star { container: &'a str, start: bool },
}

impl<'a> End<'a> {
    fn of(id: &'a str) -> End<'a> {
        if let Some(container) = id.strip_suffix(":[*]start") {
            End::Star {
                container,
                start: true,
            }
        } else if let Some(container) = id.strip_suffix(":[*]end") {
            End::Star {
                container,
                start: false,
            }
        } else {
            End::State(id)
        }
    }

    fn text(&self) -> &str {
        match self {
            End::State(id) => id,
            End::Star { .. } => "[*]",
        }
    }

    fn container(&self) -> Option<&str> {
        match self {
            End::State(_) => None,
            End::Star { container, .. } => Some(container),
        }
    }
}

/// The state ids a statement names, besides class names.
fn ids(stmt: &SyntaxNode) -> Vec<SyntaxToken> {
    let mut out: Vec<SyntaxToken> = stmt
        .children()
        .filter(|n| n.kind() == STATE_REF)
        .filter_map(|r| r.first_token())
        .filter(|t| t.kind() == IDENT)
        .collect();
    if matches!(stmt.kind(), STATE_STMT | NOTE_STMT)
        && out.is_empty()
        && let Some(t) = token(stmt, IDENT)
    {
        out.push(t);
    }
    out
}

fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == kind)
}

/// Label text as the editor gives it: control characters as spaces and each line trimmed.
fn normalize(text: &str) -> String {
    let text: String = text
        .chars()
        .map(|c| if c.is_control() && c != '\n' { ' ' } else { c })
        .collect();
    text.trim()
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Description text (after `:`) for `text`: lines joined with `<br>`, `#` as an entity code.
pub fn description_source(text: &str) -> String {
    normalize(text).replace('#', "#35;").replace('\n', "<br>")
}

/// A quoted state label (`state "..." as id`) for `text`.
fn quoted(text: &str) -> String {
    format!("\"{}\"", description_source(text).replace('"', "#quot;"))
}

/// Applies `edits`, which must not overlap, and returns the change as one edit.
fn combine(src: &str, mut edits: Vec<(Range<usize>, String)>) -> Option<TextEdit> {
    edits.sort_by_key(|(r, _)| r.start);
    let mut out = String::with_capacity(src.len());
    let mut at = 0;
    for (r, text) in edits {
        if r.start < at {
            continue;
        }
        out.push_str(&src[at..r.start]);
        out.push_str(&text);
        at = r.end;
    }
    out.push_str(&src[at..]);
    TextEdit::between(src, &out)
}

/// Adds a state shown as `label` (just its id when `label` is empty). Returns its id and the
/// edit.
pub fn add_state(src: &str, label: &str) -> (String, TextEdit) {
    let doc = Doc::new(src);
    let id = doc.fresh_id();
    let text = normalize(label);
    let line = if text.is_empty() || text == id {
        id.clone()
    } else {
        format!("state {} as {id}", quoted(&text))
    };
    (id, doc.append(&line))
}

/// Adds a transition from `from` to `to`. One into a start state, out of an end state, or
/// between start and end states of different containers is refused with `None`, as is one at a
/// concurrent region's start or end. A transition at a start or end state is added inside its
/// composite state.
pub fn add_transition(src: &str, from: &str, to: &str) -> Option<TextEdit> {
    let (a, b) = (End::of(from), End::of(to));
    if matches!(a, End::Star { start: false, .. }) || matches!(b, End::Star { start: true, .. }) {
        return None;
    }
    let container = match (a.container(), b.container()) {
        (Some(x), Some(y)) if x != y => return None,
        (x, y) => x.or(y),
    };
    let doc = Doc::new(src);
    let line = format!("{} --> {}", a.text(), b.text());
    match container {
        None | Some("") => Some(doc.append(&line)),
        // A region's id has its composite's id and the region's number.
        Some(c) if c.contains(':') => None,
        Some(c) => doc.append_in(c, &line),
    }
}

/// Shows state `id` as `label`: the quoted label of `state "..." as id` or the text of its first
/// description, with any later descriptions removed; or a new description. An empty label, or
/// one equal to the id, removes them. `None` when there is no such state or nothing changes.
pub fn set_state_label(src: &str, id: &str, label: &str) -> Option<TextEdit> {
    let doc = Doc::new(src);
    if !matches!(End::of(id), End::State(_)) || doc.chart.node(id).is_none() {
        return None;
    }
    let text = normalize(label);
    let plain = text.is_empty() || text == id;
    let carriers: Vec<SyntaxNode> = doc
        .statements()
        .filter(|s| {
            let names = ids(s);
            let named = names.first().is_some_and(|t| t.text() == id);
            named
                && match s.kind() {
                    STATE_STMT => token(s, STRING).is_some() || token(s, TEXT).is_some(),
                    DESCRIPTION_STMT => true,
                    _ => false,
                }
        })
        .collect();
    if carriers.is_empty() {
        if plain {
            return None;
        }
        return Some(doc.append(&format!("{id} : {}", description_source(&text))));
    }
    let mut edits = Vec::new();
    for (k, s) in carriers.iter().enumerate() {
        let keep = k == 0 && !plain;
        match (s.kind(), token(s, STRING)) {
            (DESCRIPTION_STMT, _) if !keep => {
                edits.push((statement_span(src, s.text_range()), String::new()));
            }
            // `state "label" as id`: the label, or `state id`.
            (STATE_STMT, Some(string)) => {
                if keep {
                    edits.push((span(string.text_range()), quoted(&text)));
                } else {
                    let end = token(s, IDENT)?.text_range().start();
                    edits.push((
                        usize::from(string.text_range().start())..usize::from(end),
                        String::new(),
                    ));
                }
            }
            // `state id : text` or `id : text`: the text, or no description.
            _ => {
                let colon = token(s, COLON)?;
                let end = usize::from(s.text_range().end());
                if keep {
                    let start = usize::from(colon.text_range().end());
                    edits.push((start..end, format!(" {}", description_source(&text))));
                } else {
                    let start = src[..usize::from(colon.text_range().start())]
                        .trim_end()
                        .len();
                    edits.push((start..end, String::new()));
                }
            }
        }
    }
    combine(src, edits)
}

/// Sets the label of transition `edge`; an empty label removes it. `None` when there is no such
/// transition or nothing changes.
pub fn set_transition_label(src: &str, edge: usize, label: &str) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let stmt = doc.transition(edge)?;
    let text = description_source(label);
    let end = usize::from(stmt.text_range().end());
    let edit = match token(&stmt, COLON) {
        Some(colon) if text.is_empty() => {
            let start = src[..usize::from(colon.text_range().start())]
                .trim_end()
                .len();
            TextEdit {
                range: start..end,
                text: String::new(),
            }
        }
        Some(colon) => TextEdit {
            range: usize::from(colon.text_range().end())..end,
            text: format!(" {text}"),
        },
        None if text.is_empty() => return None,
        None => {
            let to = stmt.children().filter(|n| n.kind() == STATE_REF).last()?;
            let at = usize::from(to.text_range().end());
            TextEdit {
                range: at..at,
                text: format!(" : {text}"),
            }
        }
    };
    (edit.apply(src) != src).then_some(edit)
}

/// Removes transitions `edges` and states `states`, with every statement naming one of them
/// (and their ids from `class` and `style` lists). A start or end state takes its transitions.
/// `None` when nothing is removed.
pub fn delete(src: &str, states: &[&str], edges: &[usize]) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let mut doomed: Vec<rowan::TextRange> = edges
        .iter()
        .filter_map(|&i| doc.chart.edges.get(i).map(|e| e.range))
        .collect();
    let (stars, named): (Vec<&str>, Vec<&str>) = states
        .iter()
        .partition(|id| matches!(End::of(id), End::Star { .. }));
    doomed.extend(
        doc.chart
            .edges
            .iter()
            .filter(|e| stars.contains(&e.from.as_str()) || stars.contains(&e.to.as_str()))
            .map(|e| e.range),
    );
    let mut edits = Vec::new();
    for s in doc.statements() {
        match s.kind() {
            CLASS_STMT | STYLE_STMT => {
                let Some(list) = s.children().find(|n| n.kind() == ID_LIST) else {
                    continue;
                };
                let all: Vec<String> = list
                    .children_with_tokens()
                    .filter_map(|e| e.into_token())
                    .filter(|t| t.kind() == IDENT)
                    .map(|t| t.text().to_owned())
                    .collect();
                let kept: Vec<&str> = all
                    .iter()
                    .map(String::as_str)
                    .filter(|id| !named.contains(id))
                    .collect();
                if kept.is_empty() {
                    doomed.push(s.text_range());
                } else if kept.len() < all.len() {
                    edits.push((span(list.text_range()), kept.join(",")));
                }
            }
            _ if ids(&s).iter().any(|t| named.contains(&t.text())) => doomed.push(s.text_range()),
            _ => {}
        }
    }
    doomed.sort_by_key(|r| r.start());
    doomed.dedup();
    edits.extend(
        doomed
            .into_iter()
            .map(|r| (statement_span(src, r), String::new())),
    );
    if edits.is_empty() {
        return None;
    }
    combine(src, edits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chart(src: &str) -> Flowchart {
        super::super::parse(src).lower().flowchart
    }

    fn apply(src: &str, e: Option<TextEdit>) -> String {
        e.expect("an edit").apply(src)
    }

    const SRC: &str = "stateDiagram-v2\n    [*] --> Idle\n    Idle --> Busy : go\n    state Work {\n        [*] --> Busy\n    }\n";

    #[test]
    fn adds_states() {
        let (id, e) = add_state(SRC, "New \"one\"");
        assert_eq!(id, "S1");
        let out = e.apply(SRC);
        assert!(
            out.ends_with("    }\n    state \"New #quot;one#quot;\" as S1\n"),
            "{out}"
        );
        let c = chart(&out);
        assert_eq!(c.node("S1").unwrap().label.text, "New \"one\"");
        let (id, e) = add_state("stateDiagram-v2\n    S1\n", "");
        assert_eq!(id, "S2");
        assert_eq!(
            e.apply("stateDiagram-v2\n    S1\n"),
            "stateDiagram-v2\n    S1\n    S2\n"
        );
    }

    #[test]
    fn adds_transitions() {
        let out = apply(SRC, add_transition(SRC, "Busy", "Idle"));
        assert!(out.ends_with("    }\n    Busy --> Idle\n"), "{out}");
        assert_eq!(chart(&out).edges.len(), 4);
        // To the diagram's end state, and from a composite's start state, inside it.
        let out = apply(SRC, add_transition(SRC, "Busy", ":[*]end"));
        assert!(out.ends_with("    Busy --> [*]\n"), "{out}");
        let out = apply(SRC, add_transition(SRC, "Work:[*]start", "Idle"));
        assert!(
            out.ends_with("        [*] --> Busy\n        [*] --> Idle\n    }\n"),
            "{out}"
        );
        let c = chart(&out);
        assert!(
            c.edges
                .iter()
                .any(|e| e.from == "Work:[*]start" && e.to == "Idle")
        );
        // Into a start state, out of an end state, or across containers' stars: refused.
        assert!(add_transition(SRC, "Idle", ":[*]start").is_none());
        assert!(add_transition(SRC, ":[*]end", "Idle").is_none());
        assert!(add_transition(SRC, ":[*]start", "Work:[*]end").is_none());
        // A one-line composite takes the line before its `}`.
        let one = "stateDiagram-v2\n    state W { a }\n";
        let out = apply(one, add_transition(one, "W:[*]start", "a"));
        assert_eq!(chart(&out).edges.len(), 1, "{out}");
    }

    #[test]
    fn labels_states() {
        let out = apply(SRC, set_state_label(SRC, "Idle", "Waiting\nhere #1"));
        assert!(out.ends_with("    Idle : Waiting<br>here #35;1\n"), "{out}");
        assert_eq!(
            chart(&out).node("Idle").unwrap().label.text,
            "Waiting\nhere #1"
        );
        // The first description is replaced and later ones removed.
        let two = "stateDiagram-v2\n    A : one\n    A --> B\n    A : two\n";
        assert_eq!(
            apply(two, set_state_label(two, "A", "three")),
            "stateDiagram-v2\n    A : three\n    A --> B\n"
        );
        assert_eq!(
            apply(two, set_state_label(two, "A", "")),
            "stateDiagram-v2\n    A --> B\n"
        );
        // `state "..." as id`, and `state id : text`.
        let alias = "stateDiagram-v2\n    state \"Old\" as A\n    state B : desc\n";
        assert_eq!(
            apply(alias, set_state_label(alias, "A", "New")),
            "stateDiagram-v2\n    state \"New\" as A\n    state B : desc\n"
        );
        assert_eq!(
            apply(alias, set_state_label(alias, "A", "A")),
            "stateDiagram-v2\n    state A\n    state B : desc\n"
        );
        assert_eq!(
            apply(alias, set_state_label(alias, "B", "")),
            "stateDiagram-v2\n    state \"Old\" as A\n    state B\n"
        );
        assert!(set_state_label(SRC, "Idle", "Idle").is_none());
        assert!(set_state_label(SRC, ":[*]start", "x").is_none());
        assert!(set_state_label(SRC, "Nope", "x").is_none());
    }

    #[test]
    fn labels_transitions() {
        assert_eq!(
            apply(SRC, set_transition_label(SRC, 1, "stop")),
            SRC.replace(": go", ": stop")
        );
        assert_eq!(
            apply(SRC, set_transition_label(SRC, 1, "")),
            SRC.replace(" : go", "")
        );
        assert_eq!(
            apply(SRC, set_transition_label(SRC, 0, "begin")),
            SRC.replace("[*] --> Idle", "[*] --> Idle : begin")
        );
        assert!(set_transition_label(SRC, 0, "").is_none());
        assert!(set_transition_label(SRC, 9, "x").is_none());
    }

    #[test]
    fn deletes() {
        // A state takes every statement naming it, and its id from class lists.
        let src = "stateDiagram-v2\n    [*] --> A\n    A --> B\n    A : desc\n    note left of A : n\n    class A,B hot\n    class A cold\n    B --> [*]\n";
        assert_eq!(
            apply(src, delete(src, &["A"], &[])),
            "stateDiagram-v2\n    class B hot\n    B --> [*]\n"
        );
        // A transition alone, and a start state with its transitions.
        assert_eq!(
            apply(src, delete(src, &[], &[1])),
            src.replace("    A --> B\n", "")
        );
        assert_eq!(
            apply(src, delete(src, &[":[*]start"], &[])),
            src.replace("    [*] --> A\n", "")
        );
        // Inside a composite state.
        assert_eq!(
            apply(SRC, delete(SRC, &["Busy"], &[])),
            "stateDiagram-v2\n    [*] --> Idle\n    state Work {\n    }\n"
        );
        assert!(delete(src, &["Z"], &[]).is_none());
    }
}
