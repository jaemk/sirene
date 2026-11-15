//! Collects a packet's fields, checking that they run on from each other (MPKT-2).

use super::{Field, MAX_BIT, Packet};
use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::lines::unquote_escaped;
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxNode, SyntaxToken};

// A field label is copied into each row crossed by the field. Bound that expansion for
// parsed input before building the rows or rendering them.
const LABEL_BYTES_LIMIT: usize = 1024 * 1024;

pub(crate) fn lower(root: &SyntaxNode) -> (Packet, Vec<Diagnostic>) {
    lower_with_limit(root, LABEL_BYTES_LIMIT)
}

fn lower_with_limit(root: &SyntaxNode, limit: usize) -> (Packet, Vec<Diagnostic>) {
    let mut p = Packet::default();
    let mut diagnostics = Vec::new();
    let mut title = None;
    for el in root.children_with_tokens() {
        if el.kind() == FRONTMATTER
            && let Some(tok) = el.as_token()
        {
            title = text::frontmatter_title(tok.text());
            for (k, v) in text::frontmatter_config(tok.text(), "packet") {
                let ok = match k.as_str() {
                    "bitsPerRow" => v
                        .parse::<u32>()
                        .ok()
                        .filter(|&n| (1..=256).contains(&n))
                        .map(|n| p.bits_per_row = n),
                    "showBits" => v.parse::<bool>().ok().map(|b| p.show_bits = b),
                    "bitOrder" => match v.as_str() {
                        "descending" | "ascending" => {
                            p.descending = v == "descending";
                            Some(())
                        }
                        _ => None,
                    },
                    _ => Some(()),
                };
                if ok.is_none() {
                    diagnostics.push(Diagnostic::warning(
                        tok.text_range(),
                        format!("ignoring `{k}: {v}`: not a valid value"),
                    ));
                }
            }
        }
    }
    // The next bit a field should start at.
    let mut next = 0u32;
    let mut label_bytes = 0usize;
    for stmt in root.children() {
        match stmt.kind() {
            TITLE_STMT => title = text_of(&stmt, TEXT).filter(|s| !s.is_empty()),
            FIELD => {
                let Some(label) = token(&stmt, STRING) else {
                    continue;
                };
                let numbers: Vec<u32> = tokens(&stmt)
                    .filter(|t| t.kind() == NUMBER)
                    .map(|t| t.text().parse().unwrap_or(u32::MAX))
                    .collect();
                let range = stmt.text_range();
                let written = || {
                    let t = stmt.to_string();
                    t[..t.find(':').unwrap_or(t.len())].trim().to_owned()
                };
                let (start, end) = if token(&stmt, PLUS).is_some() {
                    let bits = numbers[0];
                    if bits == 0 {
                        diagnostics.push(Diagnostic::new(
                            range,
                            format!("`{}` has no bits; a field holds at least one", written()),
                        ));
                        continue;
                    }
                    (next, next.saturating_add(bits - 1))
                } else {
                    let start = numbers[0];
                    (start, numbers.get(1).copied().unwrap_or(start))
                };
                if end < start {
                    diagnostics.push(Diagnostic::new(
                        range,
                        format!("`{}` ends before it starts", written()),
                    ));
                    continue;
                }
                if end > MAX_BIT {
                    diagnostics.push(Diagnostic::new(
                        range,
                        format!(
                            "`{}` runs past bit {MAX_BIT}, the last a packet draws",
                            written()
                        ),
                    ));
                    continue;
                }
                if start != next {
                    diagnostics.push(Diagnostic::new(
                        range,
                        format!(
                            "`{}` does not follow the field before it; expected it to start at \
                             bit {next}",
                            written()
                        ),
                    ));
                    // A gap is drawn empty; an overlap cannot be drawn.
                    if start < next {
                        continue;
                    }
                }
                let label = unquote_escaped(label.text());
                let rows = (end / p.bits_per_row - start / p.bits_per_row + 1) as usize;
                let expanded = label.len().checked_mul(rows);
                let total = expanded.and_then(|bytes| label_bytes.checked_add(bytes));
                let Some(total) = total.filter(|&total| total <= limit) else {
                    diagnostics.push(Diagnostic::new(
                        range,
                        "packet expanded label size limit exceeded; ignoring field",
                    ));
                    continue;
                };
                label_bytes = total;
                p.fields.push(Field { start, end, label });
                next = end + 1;
            }
            ACC_TITLE_STMT => p.acc_title = text_of(&stmt, ACC_TEXT),
            ACC_DESCR_STMT => p.acc_descr = text_of(&stmt, ACC_TEXT),
            _ => {}
        }
    }
    p.title = title;
    (p, diagnostics)
}

fn tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    node.children_with_tokens().filter_map(|e| e.into_token())
}

fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    tokens(node).find(|t| t.kind() == kind)
}

fn text_of(node: &SyntaxNode, kind: SyntaxKind) -> Option<String> {
    token(node, kind).map(|t| t.text().trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanded_labels_have_a_cumulative_limit() {
        let src = "---\nconfig:\n  packet:\n    bitsPerRow: 1\n---\npacket\n0-1: \"ab\"\n+1: \"cd\"\n+2: \"xyz\"\n+1: \"e\"\n";
        let parsed = crate::packet::parse(src);
        assert!(parsed.diagnostics().is_empty());
        let (packet, diagnostics) = lower_with_limit(&parsed.syntax(), 7);
        assert_eq!(
            packet
                .fields
                .iter()
                .map(|f| (f.start, f.end, f.label.as_str()))
                .collect::<Vec<_>>(),
            [(0, 1, "ab"), (2, 2, "cd"), (3, 3, "e")]
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("expanded label size limit"));
    }

    #[test]
    fn mixed_relative_and_range_syntax_has_no_field() {
        let parsed = crate::packet::parse("packet\n+2-100: \"x\"\n+1: \"ok\"\n");
        let (packet, diagnostics) = parsed.lower();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(packet.fields.len(), 1);
        assert_eq!(packet.fields[0].label, "ok");
        assert_eq!(packet.fields[0].start, 0);
    }

    #[test]
    fn long_label_crossing_many_rows_hits_production_limit() {
        let src = format!(
            "---\nconfig:\n  packet:\n    bitsPerRow: 1\n---\npacket\n0-9999: \"{}\"\n",
            "x".repeat(105)
        );
        let (packet, diagnostics) = crate::packet::parse(&src).lower();
        assert!(packet.fields.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("expanded label size limit"));
    }
}
