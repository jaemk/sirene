//! Drawing edits to packet diagram source (WYSIWY-28): adding, renaming, and deleting fields,
//! named by their index. Fields written with bit numbers (`16-31: "x"`) after an added or
//! deleted field are renumbered, so they still follow on; `+BITS` fields need nothing.

use std::ops::Range;

use super::{Packet, lower};
use crate::edits::{combine, normalize, span, statement_span};
use crate::flowchart::edit::TextEdit;
use crate::items::{insert_line, statement_range};
use crate::syntax::SyntaxKind::*;
use crate::syntax::SyntaxNode;

/// The bits a new field takes.
const NEW_BITS: u32 = 8;

fn lowered(src: &str) -> (Packet, Vec<SyntaxNode>) {
    let (p, _, statements) = lower::lower_statements(&super::parse(src).syntax());
    (p, statements)
}

/// Each field's label and bit count.
fn fields(p: &Packet) -> Vec<(String, u32)> {
    p.fields
        .iter()
        .map(|f| (f.label.clone(), f.bits()))
        .collect()
}

fn read(src: &str) -> Vec<(String, u32)> {
    fields(&lowered(src).0)
}

/// The label for `text` and its source, quoted with `"` and `\` escaped. `None` when it is
/// empty or has a line break.
fn quoted(text: &str) -> Option<(String, String)> {
    let want = normalize(text);
    if want.is_empty() || want.contains('\n') {
        return None;
    }
    let source = format!("\"{}\"", want.replace('\\', "\\\\").replace('"', "\\\""));
    Some((want, source))
}

/// Edits moving field statement `stmt` by `delta` bits, when it is written with bit numbers.
fn shift(stmt: &SyntaxNode, delta: i64) -> Vec<(Range<usize>, String)> {
    let relative = stmt.children_with_tokens().any(|e| e.kind() == PLUS);
    if delta == 0 || relative {
        return Vec::new();
    }
    (stmt.children_with_tokens())
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind() == NUMBER)
        .filter_map(|t| {
            let n = t.text().parse::<i64>().ok()? + delta;
            Some((span(t.text_range()), n.max(0).to_string()))
        })
        .collect()
}

/// The field labels, in order.
pub fn labels(src: &str) -> Vec<String> {
    lowered(src).0.fields.into_iter().map(|f| f.label).collect()
}

/// The source of field `i`: its line.
pub fn range(src: &str, i: usize) -> Option<Range<usize>> {
    lowered(src).1.get(i).map(statement_range)
}

/// Labels field `i` `text`. `None` when there is no such field, `text` can't be written, or
/// nothing changes.
pub fn set_label(src: &str, i: usize, text: &str) -> Option<TextEdit> {
    let (p, statements) = lowered(src);
    let (want, source) = quoted(text)?;
    let mut expect = fields(&p);
    if expect.get(i)?.0 == want {
        return None;
    }
    expect[i].0 = want;
    let label = (statements[i].children_with_tokens())
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == STRING)?;
    let edit = TextEdit {
        range: span(label.text_range()),
        text: source,
    };
    (read(&edit.apply(src)) == expect).then_some(edit)
}

/// Adds an 8-bit field labeled `text` after field `after`, or at the end, written as the field
/// before it is (`START-END` or `+8`). Returns its index and the edit; `None` when `text` can't
/// be written, there is no such field, or the fields after it would run past the last bit.
pub fn add(src: &str, after: Option<usize>, text: &str) -> Option<(usize, TextEdit)> {
    let (p, statements) = lowered(src);
    let (want, source) = quoted(text)?;
    let n = statements.len();
    let i = after.map_or(n, |a| a + 1);
    let before = match after {
        Some(a) => Some(statements.get(a)?),
        None => statements.last(),
    };
    let relative = before.is_some_and(|s| s.children_with_tokens().any(|e| e.kind() == PLUS));
    let line = if relative {
        format!("+{NEW_BITS}: {source}")
    } else {
        let start = i.checked_sub(1).map_or(0, |b| p.fields[b].end + 1);
        format!("{start}-{}: {source}", start + NEW_BITS - 1)
    };
    let last = super::parse(src).syntax().children().last();
    let insert = insert_line(src, after.and(before), last.as_ref(), &line);
    let mut edits = vec![(insert.range, insert.text)];
    for s in &statements[i..] {
        edits.extend(shift(s, i64::from(NEW_BITS)));
    }
    let edit = combine(src, edits)?;
    let mut expect = fields(&p);
    expect.insert(i, (want, NEW_BITS));
    (read(&edit.apply(src)) == expect).then_some((i, edit))
}

/// Removes fields `remove`, moving the ones after them back. `None` when nothing is removed.
pub fn delete(src: &str, remove: &[usize]) -> Option<TextEdit> {
    let (p, statements) = lowered(src);
    let mut edits = Vec::new();
    let mut expect = Vec::new();
    let mut removed = 0i64;
    for (i, (s, f)) in statements.iter().zip(&p.fields).enumerate() {
        if remove.contains(&i) {
            edits.push((statement_span(src, s.text_range()), String::new()));
            removed += i64::from(f.bits());
        } else {
            edits.extend(shift(s, -removed));
            expect.push((f.label.clone(), f.bits()));
        }
    }
    if removed == 0 {
        return None;
    }
    let edit = combine(src, edits)?;
    (read(&edit.apply(src)) == expect).then_some(edit)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str =
        "packet\n  0-15: \"Source\"\n  16-31: \"Dest\"\n  32: \"Flag\"\n  33-40: \"Rest\"\n";

    #[test]
    fn ranges_and_labels() {
        assert_eq!(labels(SRC), ["Source", "Dest", "Flag", "Rest"]);
        assert_eq!(&SRC[range(SRC, 2).unwrap()], "32: \"Flag\"");
    }

    #[test]
    fn renames() {
        let out = set_label(SRC, 1, "To \"x\"").unwrap().apply(SRC);
        assert!(out.contains("  16-31: \"To \\\"x\\\"\"\n"), "{out}");
        assert_eq!(labels(&out)[1], "To \"x\"");
        assert!(set_label(SRC, 1, "a\nb").is_none());
        assert!(set_label(SRC, 1, "Dest").is_none());
        assert!(set_label(SRC, 9, "x").is_none());
    }

    #[test]
    fn adds_and_renumbers() {
        let (i, e) = add(SRC, Some(0), "Len").unwrap();
        assert_eq!(i, 1);
        assert_eq!(
            e.apply(SRC),
            "packet\n  0-15: \"Source\"\n  16-23: \"Len\"\n  24-39: \"Dest\"\n  40: \"Flag\"\n  41-48: \"Rest\"\n"
        );
        let (i, e) = add(SRC, None, "End").unwrap();
        assert_eq!(i, 4);
        assert!(
            e.apply(SRC)
                .ends_with("  33-40: \"Rest\"\n  41-48: \"End\"\n")
        );
        let src = "packet\n  +16: \"A\"\n  +8: \"B\"\n";
        let (_, e) = add(src, Some(0), "C").unwrap();
        assert_eq!(
            e.apply(src),
            "packet\n  +16: \"A\"\n  +8: \"C\"\n  +8: \"B\"\n"
        );
        let (_, e) = add("packet\n", None, "A").unwrap();
        assert_eq!(e.apply("packet\n"), "packet\n    0-7: \"A\"\n");
        assert!(add(SRC, Some(9), "x").is_none());
    }

    #[test]
    fn deletes_and_renumbers() {
        let out = delete(SRC, &[1, 2]).unwrap().apply(SRC);
        assert_eq!(out, "packet\n  0-15: \"Source\"\n  16-23: \"Rest\"\n");
        assert!(delete(SRC, &[9]).is_none());
    }
}
