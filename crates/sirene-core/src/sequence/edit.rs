//! Source edits for changes made on a sequence diagram's drawing (WYSIWY-21).
//!
//! Participants are named by id and messages by their place among the diagram's messages, as
//! [`Sequence::messages`](super::Sequence::messages) lists them.

use std::ops::Range;

use rowan::TextRange;

use crate::flowchart::edit::TextEdit;
use crate::syntax::SyntaxKind::*;
use crate::syntax::{SyntaxNode, SyntaxToken};

/// Arrow written by [`add_message`].
pub const NEW_ARROW: &str = "->>";

/// A parsed source and what lowering found in it.
struct Doc<'a> {
    src: &'a str,
    root: SyntaxNode,
    /// The statement of each message, in order.
    messages: Vec<TextRange>,
    newline: &'static str,
}

impl<'a> Doc<'a> {
    fn new(src: &'a str) -> Self {
        let root = super::parse(src).syntax();
        let (_, messages) = super::lower::lower_with_messages(&root);
        Doc {
            src,
            root,
            messages,
            newline: if src.contains("\r\n") { "\r\n" } else { "\n" },
        }
    }

    fn statements(&self) -> impl Iterator<Item = SyntaxNode> + '_ {
        self.root
            .children()
            .filter(|s| !s.descendants().any(|n| n.kind() == ERROR))
    }

    fn message(&self, n: usize) -> Option<SyntaxNode> {
        let range = *self.messages.get(n)?;
        self.root.children().find(|s| s.text_range() == range)
    }

    /// The indent of the last statement, or four spaces.
    fn indent(&self) -> String {
        self.statements()
            .last()
            .map(|s| indent(self.src, usize::from(s.text_range().start())).to_owned())
            .filter(|i| !i.is_empty())
            .unwrap_or_else(|| "    ".to_owned())
    }

    /// Adds `line` at the end, indented as the statements are.
    fn append(&self, line: &str) -> TextEdit {
        let src = self.src;
        let sep = if src.is_empty() || src.ends_with('\n') {
            ""
        } else {
            self.newline
        };
        TextEdit {
            range: src.len()..src.len(),
            text: format!("{sep}{}{line}{}", self.indent(), self.newline),
        }
    }

    /// A participant id not yet in use: `P1`, `P2`, ...
    fn fresh_id(&self) -> String {
        let used: std::collections::HashSet<String> = self
            .statements()
            .flat_map(|s| names(&s))
            .map(|t| t.text().trim().to_owned())
            .collect();
        // One of the first `used.len() + 1` is free.
        (1..=used.len() + 1)
            .map(|n| format!("P{n}"))
            .find(|id| !used.contains(id))
            .unwrap_or_default()
    }
}

/// The participant names a statement mentions.
fn names(stmt: &SyntaxNode) -> Vec<SyntaxToken> {
    stmt.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind() == IDENT)
        .collect()
}

fn token(node: &SyntaxNode, kind: crate::syntax::SyntaxKind) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == kind)
}

/// Source text for a participant alias or message text: lines joined with `<br>`, and the
/// characters that would end or change the statement written as entity codes.
pub fn text_source(text: &str) -> String {
    let mut out = String::new();
    for (i, line) in text.trim().lines().enumerate() {
        if i > 0 {
            out.push_str("<br>");
        }
        for c in line.trim().chars() {
            match c {
                ';' => out.push_str("#59;"),
                '#' => out.push_str("#35;"),
                '<' => out.push_str("#lt;"),
                c if c.is_control() => out.push(' '),
                c => out.push(c),
            }
        }
    }
    out
}

/// The source of message `n`: its statement.
pub fn message_range(src: &str, n: usize) -> Option<Range<usize>> {
    Doc::new(src).messages.get(n).map(|r| span(*r))
}

/// The source of participant `id`: its name in its first mention.
pub fn participant_range(src: &str, id: &str) -> Option<Range<usize>> {
    let doc = Doc::new(src);
    doc.statements()
        .flat_map(|s| names(&s))
        .find(|t| t.text().trim() == id)
        .map(|t| span(t.text_range()))
}

/// Adds a participant shown as `label` (just its id when `label` is empty). Returns its id and
/// the edit.
pub fn add_participant(src: &str, label: &str) -> (String, TextEdit) {
    let doc = Doc::new(src);
    let id = doc.fresh_id();
    let text = text_source(label);
    let line = if text.is_empty() || text == id {
        format!("participant {id}")
    } else {
        format!("participant {id} as {text}")
    };
    (id, doc.append(&line))
}

/// Adds a message from `from` to `to` with `text`, after the last statement.
pub fn add_message(src: &str, from: &str, to: &str, text: &str) -> TextEdit {
    let text = text_source(text);
    // A message without text still takes the colon, so the line reads as Mermaid writes it.
    Doc::new(src).append(format!("{from}{NEW_ARROW}{to}: {text}").trim_end())
}

/// Shows participant `id` as `label`: the alias of its last declaration, or a new declaration
/// before its first mention. `None` when there is no such participant or nothing changes.
pub fn set_participant_label(src: &str, id: &str, label: &str) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let text = text_source(label);
    let mentions: Vec<SyntaxNode> = doc
        .statements()
        .filter(|s| names(s).iter().any(|t| t.text().trim() == id))
        .collect();
    let first = mentions.first()?;
    let declaration = mentions.iter().rev().find(|s| {
        s.kind() == PARTICIPANT_STMT && names(s).first().is_some_and(|t| t.text().trim() == id)
    });
    let Some(decl) = declaration else {
        if text.is_empty() || text == id {
            return None;
        }
        let at = line_start(doc.src, usize::from(first.text_range().start()));
        let keyword = "participant";
        return Some(TextEdit {
            range: at..at,
            text: format!(
                "{}{keyword} {id} as {text}{}",
                indent(doc.src, at),
                doc.newline
            ),
        });
    };
    let edit = match token(decl, TEXT) {
        // `as` then the alias: replace the alias, or drop `as` and it for an empty label.
        Some(alias) if text.is_empty() || text == id => {
            let from = token(decl, AS_KW).map_or(alias.text_range(), |a| a.text_range());
            // Back over the space before `as`.
            let start = doc.src[..usize::from(from.start())].trim_end().len();
            TextEdit {
                range: start..usize::from(alias.text_range().end()),
                text: String::new(),
            }
        }
        Some(alias) => TextEdit {
            range: span(alias.text_range()),
            text,
        },
        None if text.is_empty() || text == id => return None,
        None => {
            let end = usize::from(decl.text_range().end());
            let end = doc.src[..end].trim_end().len();
            TextEdit {
                range: end..end,
                text: format!(" as {text}"),
            }
        }
    };
    (edit.apply(src) != src).then_some(edit)
}

/// Sets the text of message `n`. `None` when there is no such message or nothing changes.
pub fn set_message_text(src: &str, n: usize, text: &str) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let stmt = doc.message(n)?;
    let text = text_source(text);
    let edit = match (token(&stmt, COLON), token(&stmt, TEXT)) {
        (_, Some(t)) => TextEdit {
            range: span(t.text_range()),
            text,
        },
        (Some(colon), None) => {
            let end = usize::from(colon.text_range().end());
            TextEdit {
                range: end..end,
                text: format!(" {text}"),
            }
        }
        (None, None) => {
            let to = names(&stmt).get(1)?.text_range().end();
            let end = usize::from(to);
            TextEdit {
                range: end..end,
                text: format!(": {text}"),
            }
        }
    };
    (edit.apply(src) != src).then_some(edit)
}

/// Removes messages `messages` and participants `participants`, with every statement naming one
/// of them. `None` when nothing is removed.
pub fn delete(src: &str, participants: &[&str], messages: &[usize]) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let mut doomed: Vec<TextRange> = messages
        .iter()
        .filter_map(|&n| doc.messages.get(n).copied())
        .collect();
    doomed.extend(
        doc.statements()
            .filter(|s| {
                names(s)
                    .iter()
                    .any(|t| participants.contains(&t.text().trim()))
            })
            .map(|s| s.text_range()),
    );
    if doomed.is_empty() {
        return None;
    }
    doomed.sort_by_key(|r| r.start());
    doomed.dedup();
    let mut out = String::with_capacity(src.len());
    let mut at = 0;
    for r in doomed {
        let range = statement_span(src, r);
        if range.start < at {
            continue;
        }
        out.push_str(&src[at..range.start]);
        at = range.end;
    }
    out.push_str(&src[at..]);
    TextEdit::between(src, &out)
}

/// The bytes that removing the statement at `r` takes: its whole line when it is alone on it,
/// otherwise the statement and a `;` after it.
fn statement_span(src: &str, r: TextRange) -> Range<usize> {
    let (start, end) = (usize::from(r.start()), usize::from(r.end()));
    let line = line_start(src, start);
    let rest = &src[end..];
    let after = rest.trim_start_matches([' ', '\t']);
    let alone_before = src[line..start].trim().is_empty();
    if alone_before && (after.is_empty() || after.starts_with('\n') || after.starts_with("\r\n")) {
        let eol = end + (rest.len() - after.len());
        let eol = if after.starts_with("\r\n") {
            eol + 2
        } else if after.starts_with('\n') {
            eol + 1
        } else {
            eol
        };
        return line..eol;
    }
    if let Some(next) = after.strip_prefix(';') {
        // Up to the next statement on the line.
        let next = next.trim_start_matches([' ', '\t']);
        return start..src.len() - next.len();
    }
    // The last of several on a line: take the `;` before it.
    let before = src[..start].trim_end_matches([' ', '\t']);
    match before.strip_suffix(';') {
        Some(kept) if !alone_before => kept.len()..end,
        _ => start..end,
    }
}

/// Leading whitespace of the line containing byte `at`.
fn indent(src: &str, at: usize) -> &str {
    let line = &src[line_start(src, at)..];
    &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
}

/// The start of the line containing byte `at`.
fn line_start(src: &str, at: usize) -> usize {
    src[..at].rfind('\n').map_or(0, |i| i + 1)
}

fn span(r: TextRange) -> Range<usize> {
    usize::from(r.start())..usize::from(r.end())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seq(src: &str) -> super::super::Sequence {
        super::super::parse(src).lower().0
    }

    fn apply(src: &str, e: Option<TextEdit>) -> String {
        e.expect("an edit").apply(src)
    }

    const SRC: &str = "sequenceDiagram\n    participant A as Alice\n    A->>B: hi\n    note over B: n\n    B-->>A: back; A->>A: self\n";

    #[test]
    fn adds_participants_and_messages() {
        let (id, e) = add_participant(SRC, "New; one");
        assert_eq!(id, "P1");
        let out = e.apply(SRC);
        assert!(
            out.ends_with("    participant P1 as New#59; one\n"),
            "{out}"
        );
        let s = seq(&out);
        assert_eq!(s.participants[2].label, ["New; one"]);

        let out = add_message(&out, "A", "P1", "two\nlines").apply(&out);
        assert!(out.ends_with("    A->>P1: two<br>lines\n"), "{out}");
        let s = seq(&out);
        assert_eq!(s.messages().last().unwrap().text, ["two", "lines"]);
        let out = add_message("sequenceDiagram", "A", "B", "").apply("sequenceDiagram");
        assert_eq!(out, "sequenceDiagram\n    A->>B:\n");
        assert_eq!(seq(&out).messages().count(), 1);
    }

    #[test]
    fn labels_participants() {
        // The alias of the declaration.
        let out = apply(SRC, set_participant_label(SRC, "A", "Ann"));
        assert!(out.contains("participant A as Ann\n"), "{out}");
        // Back to the id drops `as`.
        let out = apply(SRC, set_participant_label(SRC, "A", "A"));
        assert!(out.contains("participant A\n"), "{out}");
        assert_eq!(seq(&out).participants[0].label, ["A"]);
        // Undeclared: a declaration before its first mention, keeping its place.
        let out = apply(SRC, set_participant_label(SRC, "B", "Bob"));
        assert!(
            out.contains("    participant B as Bob\n    A->>B: hi\n"),
            "{out}"
        );
        let s = seq(&out);
        assert_eq!(s.participants[1].id, "B");
        assert_eq!(s.participants[1].label, ["Bob"]);
        assert_eq!(set_participant_label(SRC, "B", "B"), None);
        assert_eq!(set_participant_label(SRC, "Z", "Zed"), None);
        assert_eq!(set_participant_label(SRC, "A", "Alice"), None);
    }

    #[test]
    fn sets_message_text() {
        let out = apply(SRC, set_message_text(SRC, 1, "again"));
        assert!(out.contains("B-->>A: again; A->>A: self"), "{out}");
        let out = apply(SRC, set_message_text(SRC, 2, "me"));
        assert!(out.contains("A->>A: me\n"), "{out}");
        let src = "sequenceDiagram\nA->>B\nA->>B:\n";
        let out = apply(src, set_message_text(src, 0, "x"));
        let out = apply(&out, set_message_text(&out, 1, "y"));
        assert_eq!(out, "sequenceDiagram\nA->>B: x\nA->>B: y\n");
        assert_eq!(set_message_text(SRC, 3, "x"), None);
        assert_eq!(set_message_text(SRC, 0, "hi"), None);
    }

    #[test]
    fn ranges() {
        let at = |r: Option<Range<usize>>| &SRC[r.unwrap()];
        assert_eq!(at(message_range(SRC, 0)), "A->>B: hi");
        assert_eq!(at(message_range(SRC, 2)), "A->>A: self");
        assert_eq!(message_range(SRC, 3), None);
        assert_eq!(at(participant_range(SRC, "B")), "B");
        let r = participant_range(SRC, "A").unwrap();
        assert_eq!(r.start, SRC.find("A as").unwrap());
        assert_eq!(participant_range(SRC, "Z"), None);
    }

    #[test]
    fn messages_count_as_lowering_does() {
        // A message the parser could not read is not one of the diagram's messages.
        let src = "sequenceDiagram\n->>B: broken\nA->>B: one\n";
        assert_eq!(seq(src).messages().count(), 1);
        let out = apply(src, set_message_text(src, 0, "first"));
        assert!(out.ends_with("A->>B: first\n"), "{out}");
    }

    #[test]
    fn deletes() {
        let out = apply(SRC, delete(SRC, &[], &[0]));
        assert_eq!(
            out,
            "sequenceDiagram\n    participant A as Alice\n    note over B: n\n    B-->>A: back; A->>A: self\n"
        );
        let out = apply(SRC, delete(SRC, &[], &[1]));
        assert!(out.ends_with("    A->>A: self\n"), "{out}");
        let out = apply(SRC, delete(SRC, &[], &[2]));
        assert!(out.ends_with("    B-->>A: back\n"), "{out}");
        // A participant takes every statement naming it.
        let out = apply(SRC, delete(SRC, &["B"], &[]));
        assert_eq!(
            out,
            "sequenceDiagram\n    participant A as Alice\n    A->>A: self\n"
        );
        let s = seq(&out);
        assert_eq!(s.participants.len(), 1);
        assert_eq!(delete(SRC, &["Z"], &[9]), None);
    }
}
