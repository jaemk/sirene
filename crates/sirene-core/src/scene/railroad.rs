//! Railroad diagram to scene (MRAIL-3), as Mermaid draws it.
//!
//! The layout follows Mermaid 12.3.0's `railroadRenderer.ts` (MIT License, Copyright (c) 2014
//! - 2022 Knut Sveidqvist); see THIRD-PARTY-NOTICES.

use super::shapes::{self, BoxF};
use super::{
    Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle, Theme,
};
use crate::railroad::{Expr, Railroad};

const FONT: f32 = 16.0;
/// The height of a line of text, as a browser measures 16px text.
const TEXT_H: f32 = FONT * 1.15;
const PADDING: f32 = 10.0;
const V_SEP: f32 = 8.0;
const H_SEP: f32 = 10.0;
const ARC: f32 = 10.0;
const STROKE: f32 = 2.0;
const MARKER_R: f32 = 5.0;
const CORNER: f32 = 10.0;

/// An expression's size: its width, and its height above and below the line through it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Dim {
    w: f32,
    up: f32,
    down: f32,
}

impl Dim {
    fn h(self) -> f32 {
        self.up + self.down
    }
}

struct Draw<'a, M: TextMetrics> {
    metrics: &'a M,
    theme: &'a Theme,
    lines: Vec<Element>,
    boxes: Vec<Element>,
}

fn el(primitive: Primitive) -> Element {
    Element {
        owner: Owner::Diagram,
        primitive,
    }
}

/// A box's look: terminals are rounded, specials dashed.
#[derive(Clone, Copy, PartialEq)]
enum Look {
    Terminal,
    NonTerminal,
    Special,
}

fn atom(e: &Expr) -> Option<(Look, String)> {
    Some(match e {
        Expr::Terminal(t) => (Look::Terminal, t.clone()),
        Expr::NonTerminal(n) => (Look::NonTerminal, n.clone()),
        Expr::Special(s) => (Look::Special, format!("? {s} ?")),
        _ => return None,
    })
}

impl<M: TextMetrics> Draw<'_, M> {
    fn stroke(&self) -> Stroke {
        Stroke {
            width: STROKE,
            color: self.theme.edge,
            dash: None,
        }
    }

    fn path(&mut self, segments: Vec<Segment>) {
        let stroke = self.stroke();
        self.lines.push(el(Primitive::Path {
            segments,
            fill: None,
            stroke: Some(stroke),
        }));
    }

    fn line(&mut self, a: Pt, b: Pt) {
        self.path(vec![Segment::Move(a), Segment::Line(b)]);
    }

    fn measure(&self, e: &Expr) -> Dim {
        match e {
            Expr::Terminal(_) | Expr::NonTerminal(_) | Expr::Special(_) => {
                let (_, text) = atom(e).unwrap_or((Look::Terminal, String::new()));
                let w = self.metrics.width(&text, FONT) + PADDING * 2.0;
                let h = TEXT_H + PADDING * 2.0;
                Dim {
                    w,
                    up: h / 2.0,
                    down: h / 2.0,
                }
            }
            Expr::Sequence(items) => {
                let dims: Vec<Dim> = items.iter().map(|i| self.measure(i)).collect();
                Dim {
                    w: dims.iter().map(|d| d.w).sum::<f32>()
                        + (dims.len().saturating_sub(1)) as f32 * H_SEP,
                    up: dims.iter().map(|d| d.up).fold(0.0, f32::max),
                    down: dims.iter().map(|d| d.down).fold(0.0, f32::max),
                }
            }
            Expr::Choice(alts) => {
                let dims: Vec<Dim> = alts.iter().map(|a| self.measure(a)).collect();
                let max_w = dims.iter().map(|d| d.w).fold(0.0, f32::max);
                let h = dims.iter().map(|d| d.h()).sum::<f32>()
                    + (dims.len().saturating_sub(1)) as f32 * V_SEP;
                Dim {
                    w: max_w + ARC * 4.0,
                    up: h / 2.0,
                    down: h / 2.0,
                }
            }
            Expr::Optional(inner) => {
                let d = self.measure(inner);
                Dim {
                    w: d.w + ARC * 4.0,
                    up: ARC * 2.0 + d.up,
                    down: d.down,
                }
            }
            Expr::Repeat { expr, min, .. } => {
                let d = self.measure(expr);
                let bypass = if *min == 0 { ARC * 2.0 } else { 0.0 };
                Dim {
                    w: d.w + ARC * 4.0,
                    up: bypass + d.up,
                    down: d.down + ARC * 2.0,
                }
            }
        }
    }

    /// Draws `e` with its top left at `(x, y)`; returns its size.
    fn draw(&mut self, e: &Expr, x: f32, y: f32) -> Dim {
        let d = self.measure(e);
        match e {
            Expr::Terminal(_) | Expr::NonTerminal(_) | Expr::Special(_) => {
                if let Some((look, text)) = atom(e) {
                    self.atom(
                        look,
                        &text,
                        BoxF {
                            x,
                            y,
                            w: d.w,
                            h: d.h(),
                        },
                    );
                }
            }
            Expr::Sequence(items) => {
                let mut cx = x;
                let n = items.len();
                for (i, item) in items.iter().enumerate() {
                    let id = self.measure(item);
                    self.draw(item, cx, y + d.up - id.up);
                    if i + 1 < n {
                        let ly = y + d.up;
                        self.line(Pt::new(cx + id.w, ly), Pt::new(cx + id.w + H_SEP, ly));
                    }
                    cx += id.w + H_SEP;
                }
            }
            Expr::Choice(alts) => self.choice(alts, x, y, d),
            Expr::Optional(inner) => {
                let id = self.measure(inner);
                let center = y + d.up;
                let ex = x + ARC * 2.0;
                self.draw(inner, ex, y + ARC * 2.0);
                self.line(Pt::new(x, center), Pt::new(ex, center));
                self.line(Pt::new(ex + id.w, center), Pt::new(x + d.w, center));
                self.bypass(x, y, d.w, center);
            }
            Expr::Repeat { expr, min, .. } => {
                let id = self.measure(expr);
                let bypass = *min == 0;
                let ey = if bypass { y + ARC * 2.0 } else { y };
                let ex = x + ARC * 2.0;
                let center = y + d.up;
                self.draw(expr, ex, ey);
                self.line(Pt::new(x, center), Pt::new(ex, center));
                self.line(Pt::new(ex + id.w, center), Pt::new(x + d.w, center));
                // The loop back below.
                let right = ex + id.w;
                let loop_y = ey + id.h() + ARC;
                let r = Pt::new(ARC, ARC);
                let arc = |sweep: bool, to: Pt| Segment::Arc {
                    radii: r,
                    large: false,
                    sweep,
                    to,
                };
                self.path(vec![
                    Segment::Move(Pt::new(right, center)),
                    arc(true, Pt::new(right + ARC, center + ARC)),
                    Segment::Line(Pt::new(right + ARC, loop_y)),
                    arc(true, Pt::new(right, loop_y + ARC)),
                    Segment::Line(Pt::new(x + ARC * 2.0, loop_y + ARC)),
                    arc(true, Pt::new(x + ARC, loop_y)),
                    Segment::Line(Pt::new(x + ARC, center + ARC)),
                    arc(true, Pt::new(x + ARC * 2.0, center)),
                ]);
                if bypass {
                    self.bypass(x, y, d.w, center);
                }
            }
        }
        d
    }

    /// The path over the top of an optional or zero-or-more `w` wide, from and back to the
    /// line at `center`.
    fn bypass(&mut self, x: f32, y: f32, w: f32, center: f32) {
        let r = Pt::new(ARC, ARC);
        let arc = |sweep: bool, to: Pt| Segment::Arc {
            radii: r,
            large: false,
            sweep,
            to,
        };
        self.path(vec![
            Segment::Move(Pt::new(x, center)),
            arc(false, Pt::new(x + ARC, center - ARC)),
            Segment::Line(Pt::new(x + ARC, y + ARC)),
            arc(true, Pt::new(x + ARC * 2.0, y)),
            Segment::Line(Pt::new(x + w - ARC * 2.0, y)),
            arc(true, Pt::new(x + w - ARC, y + ARC)),
            Segment::Line(Pt::new(x + w - ARC, center - ARC)),
            arc(false, Pt::new(x + w, center)),
        ]);
    }

    /// Alternatives stacked and centered, joined to the line at the middle of the stack.
    fn choice(&mut self, alts: &[Expr], x: f32, y: f32, d: Dim) {
        let max_w = d.w - ARC * 4.0;
        let center = y + d.up;
        let r = Pt::new(ARC, ARC);
        let arc = |sweep: bool, to: Pt| Segment::Arc {
            radii: r,
            large: false,
            sweep,
            to,
        };
        let mut ay = y;
        for alt in alts {
            let ad = self.measure(alt);
            let ac = ay + ad.up;
            let ax = x + ARC * 2.0 + (max_w - ad.w) / 2.0;
            self.draw(alt, ax, ay);
            let right = ax + ad.w;
            let end = x + d.w;
            if (ac - center).abs() < 0.01 {
                self.line(Pt::new(x, center), Pt::new(ax, ac));
                self.line(Pt::new(right, ac), Pt::new(end, center));
            } else {
                let below = ac > center;
                let s = if below { 1.0 } else { -1.0 };
                self.path(vec![
                    Segment::Move(Pt::new(x, center)),
                    arc(below, Pt::new(x + ARC, center + s * ARC)),
                    Segment::Line(Pt::new(x + ARC, ac - s * ARC)),
                    arc(!below, Pt::new(x + ARC * 2.0, ac)),
                    Segment::Line(Pt::new(ax, ac)),
                ]);
                self.path(vec![
                    Segment::Move(Pt::new(right, ac)),
                    Segment::Line(Pt::new(end - ARC * 2.0, ac)),
                    arc(!below, Pt::new(end - ARC, ac - s * ARC)),
                    Segment::Line(Pt::new(end - ARC, center + s * ARC)),
                    arc(below, Pt::new(end, center)),
                ]);
            }
            ay += ad.h() + V_SEP;
        }
    }

    fn atom(&mut self, look: Look, text: &str, b: BoxF) {
        let t = self.theme;
        let (fill, stroke, dash, segments) = match look {
            Look::Terminal => (
                t.cluster_fill,
                t.cluster_stroke,
                None,
                shapes::rounded(b, CORNER),
            ),
            Look::NonTerminal => (t.node_fill, t.node_stroke, None, rect(b)),
            Look::Special => (t.edge_label_fill, t.node_stroke, Some((5.0, 3.0)), rect(b)),
        };
        self.boxes.push(el(Primitive::Path {
            segments,
            fill: Some(fill),
            stroke: Some(Stroke {
                width: STROKE,
                color: stroke,
                dash,
            }),
        }));
        self.boxes.push(el(Primitive::Text {
            text: text.to_owned(),
            center: b.center(),
            size: FONT,
            color: t.text,
            style: TextStyle::default(),
        }));
    }

    fn marker(&mut self, c: Pt) {
        let color: Color = self.theme.edge;
        self.boxes.push(el(Primitive::Path {
            segments: shapes::ellipse(c, MARKER_R, MARKER_R),
            fill: Some(color),
            stroke: None,
        }));
    }
}

fn rect(b: BoxF) -> Vec<Segment> {
    shapes::polygon(&[
        (b.x, b.y),
        (b.right(), b.y),
        (b.right(), b.bottom()),
        (b.x, b.bottom()),
    ])
}

/// Draws `chart` (MRAIL-3): each rule's name, then its definition between a start and an end
/// marker, the rules stacked down.
pub fn railroad(chart: &Railroad, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let mut d = Draw {
        metrics,
        theme,
        lines: Vec::new(),
        boxes: Vec::new(),
    };
    let mut names = Vec::new();
    let mut y = PADDING;
    let mut max_w: f32 = 0.0;
    for rule in &chart.rules {
        // An unnamed rule, such as a PlantUML regex, starts at the left.
        let label = if rule.name.is_empty() {
            String::new()
        } else {
            format!("{} =", rule.name)
        };
        let name_w = if label.is_empty() {
            PADDING
        } else {
            metrics.width(&label, FONT) + 20.0
        };
        let def_x = name_w + 20.0;
        let dim = d.measure(&rule.definition);
        let baseline = y + dim.up.max(20.0);
        let def_y = baseline - dim.up;
        d.draw(&rule.definition, def_x, def_y);
        // Mermaid sets the name on the line as a text baseline.
        let bold = TextStyle {
            bold: true,
            italic: false,
        };
        let w = metrics.styled_width(&label, FONT, bold);
        if !label.is_empty() {
            names.push(el(Primitive::Text {
                text: label,
                center: Pt::new(w / 2.0, baseline - FONT * 0.35),
                size: FONT,
                color: theme.text,
                style: bold,
            }));
        }
        let end = def_x + dim.w + 10.0;
        d.marker(Pt::new(name_w, baseline));
        d.marker(Pt::new(end, baseline));
        d.line(
            Pt::new(name_w + MARKER_R, baseline),
            Pt::new(def_x, baseline),
        );
        d.line(
            Pt::new(def_x + dim.w, baseline),
            Pt::new(end - MARKER_R, baseline),
        );
        let h = (def_y - y + dim.h() + PADDING * 2.0).max(40.0);
        y += h + V_SEP;
        max_w = max_w.max(end + MARKER_R);
    }
    let (width, height) = if chart.rules.is_empty() {
        (200.0, 100.0)
    } else {
        (max_w + PADDING * 2.0, y + PADDING)
    };
    let mut elements = d.lines;
    elements.append(&mut d.boxes);
    elements.append(&mut names);
    Scene {
        width,
        height,
        background: theme.background,
        elements,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Approx;

    fn draw(src: &str) -> Scene {
        let (chart, d) = crate::railroad::parse(src).lower();
        assert!(d.is_empty(), "{d:?}");
        railroad(&chart, &Approx, &Theme::LIGHT)
    }

    fn texts(s: &Scene) -> Vec<(String, Pt)> {
        s.elements
            .iter()
            .filter_map(|e| match &e.primitive {
                Primitive::Text { text, center, .. } => Some((text.clone(), *center)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn rules_stack() {
        let s = draw("railroad-ebnf-beta\na = \"x\" ;\nb = c ;\n");
        let t = texts(&s);
        let names: Vec<&str> = t.iter().map(|(s, _)| s.as_str()).collect();
        assert_eq!(names, ["x", "c", "a =", "b ="]);
        // A box's line is 20 down, so each rule is the box from there plus padding, 8 apart.
        let box_h = TEXT_H + 20.0;
        let rule_h = 20.0 - box_h / 2.0 + box_h + 20.0;
        assert!((t[1].1.y - t[0].1.y - (rule_h + V_SEP)).abs() < 0.01);
        assert!((s.height - (PADDING + 2.0 * (rule_h + V_SEP) + PADDING)).abs() < 0.01);
        let empty = draw("railroad-beta\n");
        assert_eq!((empty.width, empty.height), (200.0, 100.0));
    }

    #[test]
    fn sizes() {
        let d = Draw {
            metrics: &Approx,
            theme: &Theme::LIGHT,
            lines: Vec::new(),
            boxes: Vec::new(),
        };
        let x = Expr::Terminal("x".to_owned());
        let a = d.measure(&x);
        let h = TEXT_H + 20.0;
        assert!((a.h() - h).abs() < 0.01);
        // Two alternatives stack with 8 between, centered on the middle.
        let c = d.measure(&Expr::Choice(vec![x.clone(), x.clone()]));
        assert!((c.w - (a.w + 40.0)).abs() < 0.01);
        assert!((c.up - (h + 4.0)).abs() < 0.01);
        let o = d.measure(&Expr::Optional(Box::new(x.clone())));
        assert!((o.up - (20.0 + h / 2.0)).abs() < 0.01 && (o.down - h / 2.0).abs() < 0.01);
        let star = d.measure(&Expr::Repeat {
            expr: Box::new(x.clone()),
            min: 0,
            max: None,
        });
        assert!((star.h() - (h + 40.0)).abs() < 0.01);
        let plus = d.measure(&Expr::Repeat {
            expr: Box::new(x.clone()),
            min: 1,
            max: None,
        });
        assert!((plus.up - h / 2.0).abs() < 0.01 && (plus.down - (h / 2.0 + 20.0)).abs() < 0.01);
        let seq = d.measure(&Expr::Sequence(vec![x.clone(), x]));
        assert!((seq.w - (2.0 * a.w + 10.0)).abs() < 0.01);
    }

    #[test]
    fn looks() {
        let s = draw(
            "railroad-beta\nr = sequence(terminal(\"a\"), nonterminal(\"b\"), special(\"c\")) ;\n",
        );
        let fills: Vec<_> = s
            .elements
            .iter()
            .filter_map(|e| match &e.primitive {
                Primitive::Path {
                    fill: Some(f),
                    stroke: Some(st),
                    ..
                } => Some((*f, st.dash)),
                _ => None,
            })
            .collect();
        assert_eq!(
            fills,
            [
                (Theme::LIGHT.cluster_fill, None),
                (Theme::LIGHT.node_fill, None),
                (Theme::LIGHT.edge_label_fill, Some((5.0, 3.0))),
            ]
        );
        assert!(texts(&s).iter().any(|(t, _)| t == "? c ?"));
    }
}
