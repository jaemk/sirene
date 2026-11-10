//! Collects an XY chart's axes and series (MXY-2).

use super::{Series, SeriesKind, XAxis, XyChart, format_number};
use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxNode, SyntaxToken};

pub(crate) fn lower(root: &SyntaxNode) -> (XyChart, Vec<Diagnostic>) {
    let mut c = XyChart::default();
    let mut diagnostics = Vec::new();
    let mut title = None;
    for el in root.children_with_tokens() {
        if el.kind() == FRONTMATTER
            && let Some(tok) = el.as_token()
        {
            title = text::frontmatter_title(tok.text());
        }
    }
    let mut series_nodes = Vec::new();
    for node in root.children() {
        match node.kind() {
            HEADER => {
                c.horizontal = token(&node, ORIENTATION).is_some_and(|t| t.text() == "horizontal");
            }
            TITLE_STMT => title = name(&node),
            AXIS_STMT => axis(&mut c, &node, &mut diagnostics),
            SERIES_STMT => {
                if token(&node, R_BRACKET).is_none() {
                    continue;
                }
                let kind = match token(&node, SERIES_KW)
                    .map(|t| t.text().to_owned())
                    .as_deref()
                {
                    Some("bar") => SeriesKind::Bar,
                    _ => SeriesKind::Line,
                };
                let mut values = Vec::new();
                let mut labels = Vec::new();
                let mut seen_bracket = false;
                for t in tokens(&node) {
                    match t.kind() {
                        L_BRACKET => seen_bracket = true,
                        NUMBER => {
                            values.push(t.text().parse::<f64>().unwrap_or(0.0));
                            labels.push(None);
                        }
                        STRING if seen_bracket => {
                            if let Some(l) = labels.last_mut() {
                                *l = Some(text::unquote(t.text()).to_owned());
                            }
                        }
                        _ => {}
                    }
                }
                let name = tokens(&node)
                    .take_while(|t| t.kind() != L_BRACKET)
                    .find(|t| matches!(t.kind(), TEXT | STRING))
                    .map(|t| text::unquote(t.text()).trim().to_owned())
                    .filter(|s| !s.is_empty());
                c.series.push(Series {
                    kind,
                    name,
                    values,
                    labels,
                });
                series_nodes.push(node);
            }
            ACC_TITLE_STMT => c.acc_title = text_of(&node, ACC_TEXT),
            ACC_DESCR_STMT => c.acc_descr = text_of(&node, ACC_TEXT),
            _ => {}
        }
    }
    if let XAxis::Categories(cats) = &c.x_axis
        && !cats.is_empty()
    {
        for (s, node) in c.series.iter().zip(&series_nodes) {
            check_len(s, cats.len(), node, &mut diagnostics);
        }
    }
    c.title = title.filter(|t| !t.is_empty());
    (c, diagnostics)
}

/// A series with more values than there are categories: the extra values are not drawn.
fn check_len(s: &Series, n: usize, node: &SyntaxNode, diagnostics: &mut Vec<Diagnostic>) {
    if s.values.len() > n {
        let kind = match s.kind {
            SeriesKind::Line => "line",
            SeriesKind::Bar => "bar",
        };
        diagnostics.push(Diagnostic::warning(
            node.text_range(),
            format!(
                "this `{kind}` has {} values for {n} categories; the last {} are not drawn",
                s.values.len(),
                s.values.len() - n
            ),
        ));
    }
}

fn axis(c: &mut XyChart, node: &SyntaxNode, diagnostics: &mut Vec<Diagnostic>) {
    let x = token(node, AXIS_KW).is_some_and(|t| t.text() == "x-axis");
    // A list or range that did not parse leaves the axis as it was, apart from its title.
    let title = tokens(node)
        .take_while(|t| !matches!(t.kind(), L_BRACKET | NUMBER))
        .find(|t| matches!(t.kind(), TEXT | STRING))
        .map(|t| text::unquote(t.text()).trim().to_owned())
        .filter(|s| !s.is_empty());
    let numbers: Vec<f64> = tokens(node)
        .filter(|t| t.kind() == NUMBER)
        .map(|t| t.text().parse::<f64>().unwrap_or(0.0))
        .collect();
    let range = match numbers[..] {
        [lo, hi] if lo > hi => {
            diagnostics.push(Diagnostic::warning(
                node.text_range(),
                format!(
                    "the range runs down from {} to {}; drawing it from {} to {}",
                    format_number(lo),
                    format_number(hi),
                    format_number(hi),
                    format_number(lo)
                ),
            ));
            Some((hi, lo))
        }
        [lo, hi] => Some((lo, hi)),
        _ => None,
    };
    let list = token(node, L_BRACKET).is_some();
    if x {
        c.x_title = title;
        if list && token(node, R_BRACKET).is_some() {
            let cats = tokens(node)
                .skip_while(|t| t.kind() != L_BRACKET)
                .filter(|t| matches!(t.kind(), TEXT | STRING))
                .map(|t| text::unquote(t.text()).trim().to_owned())
                .collect();
            c.x_axis = XAxis::Categories(cats);
        } else if let Some((min, max)) = range {
            c.x_axis = XAxis::Range { min, max };
        }
    } else {
        c.y_title = title;
        if list {
            diagnostics.push(Diagnostic::new(
                node.text_range(),
                "the y-axis takes a range (`min --> max`), not categories",
            ));
        }
        if range.is_some() {
            c.y_range = range;
        }
    }
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

/// The first `TEXT` or `STRING`, unquoted and trimmed.
fn name(node: &SyntaxNode) -> Option<String> {
    tokens(node)
        .find(|t| matches!(t.kind(), TEXT | STRING))
        .map(|t| text::unquote(t.text()).trim().to_owned())
}
