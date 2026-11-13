//! Collects a quadrant chart's axes, quadrant labels, and styled points (MQUAD-2).

use std::collections::HashMap;

use super::{Axis, Point, PointStyle, QuadrantChart};
use crate::diagnostic::Diagnostic;
use crate::flowchart::{Style, text};
use crate::scene::parse_color;
use crate::syntax::SyntaxKind::{self, *};
use crate::syntax::{SyntaxNode, SyntaxToken};
use rowan::TextRange;

pub(crate) fn lower(root: &SyntaxNode) -> (QuadrantChart, Vec<Diagnostic>) {
    let mut q = QuadrantChart::default();
    let mut diagnostics = Vec::new();
    let mut title = None;
    for el in root.children_with_tokens() {
        if el.kind() == FRONTMATTER
            && let Some(tok) = el.as_token()
        {
            title = text::frontmatter_title(tok.text());
        }
    }
    // Points with the statement they came from, styled once every `classDef` is known.
    let mut points: Vec<(Point, SyntaxNode)> = Vec::new();
    let mut classes: HashMap<String, Vec<Style>> = HashMap::new();
    for stmt in root.children() {
        match stmt.kind() {
            TITLE_STMT => title = text_of(&stmt, TEXT).filter(|s| !s.is_empty()),
            AXIS_STMT => {
                let texts: Vec<String> = stmt
                    .children_with_tokens()
                    .filter_map(|e| e.into_token())
                    .filter(|t| matches!(t.kind(), TEXT | STRING))
                    .map(|t| text::unquote(t.text()).trim().to_owned())
                    .collect();
                let label = |i: usize| texts.get(i).filter(|s| !s.is_empty()).cloned();
                let mut axis = Axis {
                    low: label(0),
                    high: label(1),
                };
                // `x-axis Low -->` with no high label: Mermaid appends the arrow to the low one.
                if axis.high.is_none() && token(&stmt, ARROW).is_some() {
                    axis.low = axis.low.map(|low| format!("{low} ⟶"));
                }
                match token(&stmt, AXIS_KW)
                    .map(|t| t.text().to_owned())
                    .as_deref()
                {
                    Some("x-axis") => q.x_axis = axis,
                    _ => q.y_axis = axis,
                }
            }
            QUADRANT_STMT => {
                let n = token(&stmt, QUADRANT_KW)
                    .and_then(|t| t.text().strip_prefix("quadrant-")?.parse::<usize>().ok());
                if let Some(n) = n.filter(|n| (1..=4).contains(n)) {
                    q.quadrants[n - 1] = label(&stmt);
                }
            }
            CLASSDEF_STMT => {
                let (styles, _) = text::styles(&text_of(&stmt, STYLE_TEXT).unwrap_or_default());
                for id in stmt
                    .children()
                    .filter(|n| n.kind() == ID_LIST)
                    .flat_map(|l| l.children_with_tokens())
                    .filter_map(|e| e.into_token())
                    .filter(|t| t.kind() == IDENT)
                {
                    classes
                        .entry(id.text().to_owned())
                        .or_default()
                        .extend(styles.iter().cloned());
                }
            }
            POINT_STMT => {
                if let Some(p) = point(&stmt, &mut diagnostics) {
                    points.push((p, stmt));
                }
            }
            ACC_TITLE_STMT => q.acc_title = text_of(&stmt, ACC_TEXT),
            ACC_DESCR_STMT => q.acc_descr = text_of(&stmt, ACC_TEXT),
            _ => {}
        }
    }
    for (mut p, stmt) in points {
        if let Some(c) = &p.class {
            match classes.get(c) {
                Some(styles) => apply(&mut p.style, styles, stmt.text_range(), &mut diagnostics),
                None => diagnostics.push(Diagnostic::warning(
                    stmt.text_range(),
                    format!(
                        "no `classDef {c}`; drawing `{}` with the default style",
                        p.name
                    ),
                )),
            }
        }
        if let Some(t) = token(&stmt, STYLE_TEXT) {
            let (styles, invalid) = text::styles(t.text());
            for s in invalid {
                diagnostics.push(Diagnostic::warning(
                    t.text_range(),
                    format!("ignoring `{s}`: expected `name: value`"),
                ));
            }
            apply(&mut p.style, &styles, t.text_range(), &mut diagnostics);
        }
        q.points.push(p);
    }
    q.title = title;
    (q, diagnostics)
}

/// A point from its name, class, and coordinates, without its styles.
fn point(stmt: &SyntaxNode, diagnostics: &mut Vec<Diagnostic>) -> Option<Point> {
    token(stmt, R_BRACKET)?;
    let name = label(stmt).unwrap_or_default();
    let class = stmt
        .children()
        .find(|n| n.kind() == CLASS_SHORTHAND)
        .and_then(|n| token(&n, IDENT))
        .map(|t| t.text().to_owned());
    let mut at = [0.0; 2];
    let numbers = stmt
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind() == NUMBER);
    for (v, tok) in at.iter_mut().zip(numbers) {
        let raw = tok.text();
        let Some(n) = raw.parse::<f64>().ok().filter(|n| n.is_finite()) else {
            diagnostics.push(Diagnostic::new(
                tok.text_range(),
                format!("`{raw}` is not a number; expected a value from 0 to 1"),
            ));
            return None;
        };
        *v = n;
        if !(0.0..=1.0).contains(&n) {
            *v = n.clamp(0.0, 1.0);
            diagnostics.push(Diagnostic::warning(
                tok.text_range(),
                format!("`{raw}` is outside 0 to 1; drawing it at {v}"),
            ));
        }
    }
    Some(Point {
        name,
        x: at[0],
        y: at[1],
        class,
        style: PointStyle::default(),
    })
}

/// Applies `styles` to `style` in order; unknown names and bad values are warnings.
fn apply(
    style: &mut PointStyle,
    styles: &[Style],
    range: TextRange,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for s in styles {
        let v = s.value.trim();
        let ok = match s.property.as_str() {
            "radius" => length(v).map(|n| style.radius = Some(n)),
            "color" => parse_color(v).map(|c| style.color = Some(c)),
            "stroke-color" => parse_color(v).map(|c| style.stroke_color = Some(c)),
            "stroke-width" => length(v).map(|n| style.stroke_width = Some(n)),
            other => {
                diagnostics.push(Diagnostic::warning(
                    range,
                    format!(
                        "ignoring `{other}`: a point's styles are `radius`, `color`, \
                         `stroke-color`, and `stroke-width`"
                    ),
                ));
                continue;
            }
        };
        if ok.is_none() {
            diagnostics.push(Diagnostic::warning(
                range,
                format!("ignoring `{}: {v}`: not a valid value", s.property),
            ));
        }
    }
}

/// A non-negative length in pixels, with an optional `px` unit.
fn length(v: &str) -> Option<f32> {
    let n = v.strip_suffix("px").unwrap_or(v).trim();
    n.parse::<f32>().ok().filter(|n| n.is_finite() && *n >= 0.0)
}

/// The first `TEXT` or `STRING`, unquoted and trimmed; `None` when empty.
fn label(node: &SyntaxNode) -> Option<String> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| matches!(t.kind(), TEXT | STRING))
        .map(|t| text::unquote(t.text()).trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == kind)
}

fn text_of(node: &SyntaxNode, kind: SyntaxKind) -> Option<String> {
    token(node, kind).map(|t| t.text().trim().to_owned())
}
