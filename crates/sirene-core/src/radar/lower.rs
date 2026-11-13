//! Collects a radar chart's axes, curves, and options (MRADAR-2).

use super::{Axis, Curve, Graticule, Radar};
use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::lines::unquote_escaped;
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxNode, SyntaxToken};

pub(crate) fn lower(root: &SyntaxNode) -> (Radar, Vec<Diagnostic>) {
    let mut r = Radar::default();
    let mut diagnostics = Vec::new();
    let mut title = None;
    for el in root.children_with_tokens() {
        if el.kind() == FRONTMATTER
            && let Some(tok) = el.as_token()
        {
            title = text::frontmatter_title(tok.text());
            for (k, v) in text::frontmatter_config(tok.text(), "radar") {
                let n = v.parse::<f32>().ok().filter(|n| n.is_finite());
                let positive = n.filter(|&n| n > 0.0);
                let at_least_0 = n.filter(|&n| n >= 0.0);
                let slot = match k.as_str() {
                    "width" => positive.map(|n| r.width = n),
                    "height" => positive.map(|n| r.height = n),
                    "marginTop" => at_least_0.map(|n| r.margins[0] = n),
                    "marginRight" => at_least_0.map(|n| r.margins[1] = n),
                    "marginBottom" => at_least_0.map(|n| r.margins[2] = n),
                    "marginLeft" => at_least_0.map(|n| r.margins[3] = n),
                    "axisScaleFactor" => at_least_0.map(|n| r.axis_scale_factor = n),
                    "axisLabelFactor" => at_least_0.map(|n| r.axis_label_factor = n),
                    "curveTension" => at_least_0.map(|n| r.curve_tension = n),
                    _ => Some(()),
                };
                if slot.is_none() {
                    diagnostics.push(Diagnostic::warning(
                        tok.text_range(),
                        format!("ignoring `{k}: {v}`: not a valid value"),
                    ));
                }
            }
        }
    }

    // Axes first, so a curve may come before the axes it has values for.
    for stmt in root.children().filter(|s| s.kind() == AXIS_STMT) {
        for e in stmt.children().filter(|e| e.kind() == ENTRY) {
            let (id, label) = id_label(&e);
            if r.axes.iter().any(|a| a.id == id) {
                diagnostics.push(Diagnostic::new(
                    e.text_range(),
                    format!("axis `{id}` is already defined; ignoring this one"),
                ));
                continue;
            }
            r.axes.push(Axis { id, label });
        }
    }

    for stmt in root.children() {
        match stmt.kind() {
            TITLE_STMT => title = text_of(&stmt, TEXT).filter(|s| !s.is_empty()),
            ACC_TITLE_STMT => r.acc_title = text_of(&stmt, ACC_TEXT),
            ACC_DESCR_STMT => r.acc_descr = text_of(&stmt, ACC_TEXT),
            SERIES_STMT => {
                for e in stmt.children().filter(|e| e.kind() == ENTRY) {
                    if token(&e, R_BRACE).is_some() {
                        let c = curve(&r, &e, &mut diagnostics);
                        r.curves.push(c);
                    }
                }
            }
            SETTING_STMT => {
                if let Some(d) = option(&mut r, &stmt) {
                    diagnostics.push(d);
                }
            }
            _ => {}
        }
    }
    if let Some(max) = r.max
        && max <= r.min
    {
        let at = root
            .children()
            .filter(|s| s.kind() == SETTING_STMT)
            .filter(|s| token(s, SETTING_KW).is_some_and(|k| k.text() == "max"))
            .last()
            .map_or(root.text_range(), |s| s.text_range());
        diagnostics.push(Diagnostic::new(
            at,
            format!(
                "`max {}` is not above the minimum, {}; using the largest value",
                format(max),
                format(r.min)
            ),
        ));
        r.max = None;
    }
    r.title = title;
    (r, diagnostics)
}

fn format(n: f64) -> String {
    crate::xychart::format_number(n)
}

/// An entry's id, and its label or else its id.
fn id_label(e: &SyntaxNode) -> (String, String) {
    let id = token(e, IDENT).map_or(String::new(), |t| t.text().to_owned());
    let label = token(e, STRING).map_or_else(|| id.clone(), |t| unquote_escaped(t.text()));
    (id, label)
}

/// A curve's values in axis order: listed in order, or keyed by axis id. A missing value sits
/// at the minimum.
fn curve(r: &Radar, e: &SyntaxNode, diagnostics: &mut Vec<Diagnostic>) -> Curve {
    let (id, label) = id_label(e);
    let entries: Vec<(Option<SyntaxToken>, f64, SyntaxNode)> = e
        .children()
        .filter(|v| v.kind() == VALUE)
        .map(|v| {
            let n = token(&v, NUMBER).map_or(0.0, |t| t.text().parse().unwrap_or(0.0));
            (token(&v, IDENT), n, v)
        })
        .collect();
    let mut error = |node: &SyntaxNode, message: String| {
        diagnostics.push(Diagnostic::new(node.text_range(), message));
    };
    let n = r.axes.len();
    let mut values = vec![None; n];
    if entries.iter().any(|(key, ..)| key.is_some()) {
        for (key, v, node) in &entries {
            let Some(key) = key else {
                error(
                    node,
                    format!("expected `axis: value` in `{id}`, as its other values are"),
                );
                continue;
            };
            let key = key.text();
            match r.axes.iter().position(|a| a.id == key) {
                None => error(
                    node,
                    format!("`{key}` in `{id}` is not an axis; ignoring it"),
                ),
                Some(i) if values[i].is_some() => {
                    error(
                        node,
                        format!("`{id}` has two values for `{key}`; using the first"),
                    );
                }
                Some(i) => values[i] = Some(*v),
            }
        }
        let missing: Vec<&str> = r
            .axes
            .iter()
            .zip(&values)
            .filter(|(_, v)| v.is_none())
            .map(|(a, _)| a.id.as_str())
            .collect();
        if !missing.is_empty() {
            let list = missing
                .iter()
                .map(|m| format!("`{m}`"))
                .collect::<Vec<_>>()
                .join(", ");
            error(
                e,
                format!("`{id}` has no value for {list}; drawing it at the minimum"),
            );
        }
    } else {
        for (slot, (_, v, _)) in values.iter_mut().zip(&entries) {
            *slot = Some(*v);
        }
        if entries.len() != n {
            let what = if entries.len() < n {
                "drawing the missing ones at the minimum"
            } else {
                "ignoring the extra ones"
            };
            error(
                e,
                format!(
                    "`{id}` has {} for {}; {what}",
                    count(entries.len(), "value"),
                    count(n, "axis")
                ),
            );
        }
    }
    Curve {
        id,
        label,
        values: values.into_iter().map(|v| v.unwrap_or(r.min)).collect(),
    }
}

fn count(n: usize, what: &str) -> String {
    match (n, what) {
        (1, _) => format!("1 {what}"),
        (_, "axis") => format!("{n} axes"),
        _ => format!("{n} {what}s"),
    }
}

/// Applies one option, or returns why it cannot.
fn option(r: &mut Radar, stmt: &SyntaxNode) -> Option<Diagnostic> {
    let kw = token(stmt, SETTING_KW)?;
    let value = token(stmt, TEXT)?;
    let v = value.text();
    let number = v.parse::<f64>().ok().filter(|n| n.is_finite());
    let ok = match kw.text() {
        "showLegend" => v.parse::<bool>().ok().map(|b| r.show_legend = b),
        "max" => number.map(|n| r.max = Some(n)),
        "min" => number.map(|n| r.min = n),
        "graticule" => match v {
            "circle" => Some(Graticule::Circle),
            "polygon" => Some(Graticule::Polygon),
            _ => None,
        }
        .map(|g| r.graticule = g),
        "ticks" => v
            .parse::<u32>()
            .ok()
            .filter(|n| (1..=100).contains(n))
            .map(|n| r.ticks = n),
        _ => Some(()),
    };
    if ok.is_some() {
        return None;
    }
    let expected = match kw.text() {
        "showLegend" => "`true` or `false`",
        "graticule" => "`circle` or `polygon`",
        "ticks" => "a whole number from 1 to 100",
        _ => "a number",
    };
    Some(Diagnostic::new(
        value.text_range(),
        format!("expected {expected} after `{}`, not `{v}`", kw.text()),
    ))
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
