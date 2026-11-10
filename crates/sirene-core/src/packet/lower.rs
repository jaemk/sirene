//! Collects a packet's fields, checking that they run on from each other (MPKT-2).

use super::{Field, MAX_BIT, Packet};
use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxNode, SyntaxToken};

pub(crate) fn lower(root: &SyntaxNode) -> (Packet, Vec<Diagnostic>) {
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
                p.fields.push(Field {
                    start,
                    end,
                    label: text::unquote(label.text()).to_owned(),
                });
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
