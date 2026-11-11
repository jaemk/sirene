//! Collects a pie chart's title and slices (MPIE-2).

use std::collections::HashSet;

use super::{Pie, Slice};
use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxNode, SyntaxToken};

pub(crate) fn lower(root: &SyntaxNode) -> (Pie, Vec<Diagnostic>) {
    let mut pie = Pie::default();
    let mut diagnostics = Vec::new();
    let mut title = None;
    let mut labels = HashSet::new();
    for el in root.children_with_tokens() {
        if el.kind() == FRONTMATTER
            && let Some(tok) = el.as_token()
        {
            title = text::frontmatter_title(tok.text());
        }
    }
    for stmt in root.children() {
        match stmt.kind() {
            HEADER => pie.show_data = token(&stmt, SHOW_DATA_KW).is_some(),
            TITLE_STMT => title = text_of(&stmt, TEXT).filter(|s| !s.is_empty()),
            SLICE_STMT => {
                let (Some(label), Some(number)) = (token(&stmt, STRING), token(&stmt, NUMBER))
                else {
                    continue;
                };
                let text = label.text();
                let label = text[1..text.len() - 1].to_owned();
                let value: f64 = number.text().parse().expect("a lexed number parses");
                if value < 0.0 {
                    diagnostics.push(Diagnostic::new(
                        number.text_range(),
                        format!("`{label}` has a negative value; a slice must be 0 or more"),
                    ));
                } else if !labels.insert(label.clone()) {
                    diagnostics.push(Diagnostic::warning(
                        stmt.text_range(),
                        format!("`{label}` is already a slice; skipping this one"),
                    ));
                } else {
                    pie.slices.push(Slice { label, value });
                }
            }
            ACC_TITLE_STMT => pie.acc_title = text_of(&stmt, ACC_TEXT),
            ACC_DESCR_STMT => pie.acc_descr = text_of(&stmt, ACC_TEXT),
            _ => {}
        }
    }
    pie.title = title;
    (pie, diagnostics)
}

fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == kind)
}

fn text_of(node: &SyntaxNode, kind: SyntaxKind) -> Option<String> {
    token(node, kind).map(|t| t.text().trim().to_owned())
}
