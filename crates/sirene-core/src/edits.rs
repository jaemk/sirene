//! Helpers shared by the diagram edit modules (`flowchart::edit`, `sequence::edit`,
//! `state::edit`, `class::edit`, ...): spans of statements, indentation, label text, and
//! combining edits.

use std::collections::HashSet;
use std::ops::Range;

use rowan::TextRange;

use crate::flowchart::edit::TextEdit;
use crate::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

/// The newline `src` uses.
pub(crate) fn newline(src: &str) -> &'static str {
    if src.contains("\r\n") { "\r\n" } else { "\n" }
}

/// The bytes that removing the statement at `r` takes: its whole line when it is alone on it,
/// otherwise the statement and a `;` after it.
pub(crate) fn statement_span(src: &str, r: TextRange) -> Range<usize> {
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
pub(crate) fn indent(src: &str, at: usize) -> &str {
    let line = &src[line_start(src, at)..];
    &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
}

/// The start of the line containing byte `at`.
pub(crate) fn line_start(src: &str, at: usize) -> usize {
    src[..at].rfind('\n').map_or(0, |i| i + 1)
}

pub(crate) fn span(r: TextRange) -> Range<usize> {
    usize::from(r.start())..usize::from(r.end())
}

/// The first token of `kind` directly in `node`.
pub(crate) fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == kind)
}

/// Label text as the editor gives it: control characters as spaces and each line trimmed.
pub(crate) fn normalize(text: &str) -> String {
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

/// Label source for `text`: lines joined with `<br>`, `#` as an entity code.
pub(crate) fn label_source(text: &str) -> String {
    normalize(text).replace('#', "#35;").replace('\n', "<br>")
}

/// A quoted label for `text`, with `"` as an entity code.
pub(crate) fn quoted(text: &str) -> String {
    format!("\"{}\"", label_source(text).replace('"', "#quot;"))
}

/// Applies `edits`, which must not overlap (a later overlapping one is dropped), and returns the
/// change as one edit.
pub(crate) fn combine(src: &str, mut edits: Vec<(Range<usize>, String)>) -> Option<TextEdit> {
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

/// Adds `line` at the end of `src`, indented as `last` (the last statement) is, or by four
/// spaces.
pub(crate) fn append(src: &str, last: Option<&SyntaxNode>, line: &str) -> TextEdit {
    let nl = newline(src);
    let indent = last
        .map(|s| indent(src, usize::from(s.text_range().start())).to_owned())
        .filter(|i| !i.is_empty())
        .unwrap_or_else(|| "    ".to_owned());
    let sep = if src.is_empty() || src.ends_with('\n') {
        ""
    } else {
        nl
    };
    TextEdit {
        range: src.len()..src.len(),
        text: format!("{sep}{indent}{line}{nl}"),
    }
}

/// An id not in `used`: `PREFIX1`, `PREFIX2`, ...
pub(crate) fn fresh_id<'a>(prefix: &str, used: impl IntoIterator<Item = &'a str>) -> String {
    let used: HashSet<&str> = used.into_iter().collect();
    // One of the first `used.len() + 1` is free.
    (1..=used.len() + 1)
        .map(|n| format!("{prefix}{n}"))
        .find(|id| !used.contains(id.as_str()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_ids() {
        assert_eq!(fresh_id("S", []), "S1");
        assert_eq!(fresh_id("S", ["S1", "S3"]), "S2");
        assert_eq!(fresh_id("C", ["C1", "C2"]), "C3");
    }

    #[test]
    fn combines() {
        let e = combine("abcdef", vec![(4..5, "X".into()), (0..1, String::new())]).unwrap();
        assert_eq!(e.apply("abcdef"), "bcdXf");
        assert!(combine("ab", vec![(0..1, "a".into())]).is_none());
    }

    #[test]
    fn labels() {
        assert_eq!(normalize("  a \n\t b\u{7} "), "a\nb");
        assert_eq!(quoted("a \"b\"\n#1"), "\"a #quot;b#quot;<br>#35;1\"");
        assert_eq!(newline("a\r\nb"), "\r\n");
    }

    #[test]
    fn spans() {
        let src = "x\n    a; b\n    c\n";
        assert_eq!(
            statement_span(src, TextRange::new(15.into(), 16.into())),
            11..17
        );
        assert_eq!(
            statement_span(src, TextRange::new(6.into(), 7.into())),
            6..9
        );
        assert_eq!(indent(src, 15), "    ");
    }
}
