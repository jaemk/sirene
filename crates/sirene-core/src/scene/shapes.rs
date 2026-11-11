//! Node shape geometry in pixels (DISPLA-7).

use std::f32::consts::{FRAC_1_SQRT_2, PI, SQRT_2};

use super::{Pt, Segment};
use crate::flowchart::Shape;

/// Horizontal padding between a label and its node's sides.
pub(super) const PAD_X: f32 = 12.0;
/// Vertical padding between a label and its node's top and bottom.
pub(super) const PAD_Y: f32 = 8.0;

/// An axis-aligned box.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct BoxF {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl BoxF {
    pub fn right(self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(self) -> f32 {
        self.y + self.h
    }

    pub fn center(self) -> Pt {
        Pt::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

/// Shapes that are drawn alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Geo {
    Rect,
    Rounded,
    Stadium,
    Framed,
    Cylinder,
    Circle,
    Crossed,
    Double,
    /// A small solid dot.
    Dot,
    /// A ring around a solid dot.
    Stop,
    Ellipse,
    Diamond,
    Hexagon,
    LeanRight,
    LeanLeft,
    TrapBottom,
    TrapTop,
    Odd,
    Text,
    Triangle,
    Flipped,
    Fork,
    Notched,
    Sloped,
    /// A figure with its label below.
    Actor,
    UseCase,
    /// A collapsed subgraph: its title over a dashed rule and three dots.
    Collapsed,
}

fn geo(shape: Shape) -> Geo {
    match shape {
        Shape::Actor | Shape::HollowActor | Shape::AwesomeActor => Geo::Actor,
        Shape::UseCase => Geo::UseCase,
        Shape::Collapsed => Geo::Collapsed,
        Shape::Rounded => Geo::Rounded,
        Shape::Stadium => Geo::Stadium,
        Shape::FramedRect => Geo::Framed,
        Shape::Cylinder | Shape::LinedCylinder | Shape::HorizontalCylinder => Geo::Cylinder,
        Shape::Circle => Geo::Circle,
        Shape::CrossedCircle => Geo::Crossed,
        Shape::DoubleCircle => Geo::Double,
        Shape::SmallCircle | Shape::FilledCircle => Geo::Dot,
        Shape::FramedCircle => Geo::Stop,
        Shape::Ellipse => Geo::Ellipse,
        Shape::Diamond => Geo::Diamond,
        Shape::Hexagon => Geo::Hexagon,
        Shape::LeanRight => Geo::LeanRight,
        Shape::LeanLeft => Geo::LeanLeft,
        Shape::TrapezoidBottom => Geo::TrapBottom,
        Shape::TrapezoidTop => Geo::TrapTop,
        Shape::Odd => Geo::Odd,
        Shape::Text => Geo::Text,
        Shape::Triangle => Geo::Triangle,
        Shape::FlippedTriangle => Geo::Flipped,
        Shape::Fork => Geo::Fork,
        Shape::NotchedRect => Geo::Notched,
        Shape::SlopedRect => Geo::Sloped,
        _ => Geo::Rect,
    }
}

/// Whether the shape shows its label.
pub(super) fn has_label(shape: Shape) -> bool {
    !matches!(geo(shape), Geo::Dot | Geo::Stop | Geo::Fork)
}

const SLOPE: f32 = 10.0;
const NOTCH: f32 = 12.0;
const FRAME: f32 = 8.0;
/// An actor's figure, the gap below it, and the padding around figure and label, as Mermaid
/// draws them.
const FIGURE_W: f32 = 56.0;
const FIGURE_H: f32 = 72.0;
const FIGURE_GAP: f32 = 8.0;
const ACTOR_PAD: f32 = 10.0;
/// The room between a use case's label and its ellipse on every side.
const USECASE_PAD: f32 = 20.0;
/// A collapsed subgraph's padding, minimum width, and the gap and row under its title, as
/// Mermaid's `collapsedGroup` draws them.
const COLLAPSED_PAD: f32 = 8.0;
const COLLAPSED_MIN_W: f32 = 80.0;
const COLLAPSED_GAP: f32 = 8.0;
const COLLAPSED_ROW: f32 = 20.0;

/// Radius of a cylinder's end ellipses.
fn cylinder_ry(w: f32) -> f32 {
    (w * 0.1).clamp(5.0, 12.0)
}

/// Shapes whose size doubles the label's in both directions, so a long one-line label makes a
/// wide flat shape and wrapping it makes a smaller one.
pub(super) fn grows_square(shape: Shape) -> bool {
    matches!(geo(shape), Geo::Diamond | Geo::Triangle | Geo::Flipped)
}

/// The size of a node whose label measures `tw` by `th`.
pub(super) fn size(shape: Shape, tw: f32, th: f32) -> (f32, f32) {
    let (w0, h0) = (tw + 2.0 * PAD_X, th + 2.0 * PAD_Y);
    let circle = (tw.hypot(th) + 2.0 * PAD_Y).max(h0);
    match geo(shape) {
        Geo::Rect | Geo::Rounded | Geo::Notched => (w0, h0),
        Geo::Stadium | Geo::Hexagon | Geo::LeanRight | Geo::LeanLeft => (w0 + h0 / 2.0, h0),
        Geo::TrapBottom | Geo::TrapTop => (w0 + h0 / 2.0, h0),
        Geo::Odd => (w0 + h0 / 4.0, h0),
        Geo::Framed => (w0 + 2.0 * FRAME, h0),
        Geo::Cylinder => (w0, h0 + 2.0 * cylinder_ry(w0)),
        Geo::Circle | Geo::Crossed => (circle, circle),
        Geo::Double => (circle + 10.0, circle + 10.0),
        Geo::Dot => (14.0, 14.0),
        Geo::Stop => (18.0, 18.0),
        Geo::Fork => (70.0, 10.0),
        Geo::Ellipse => ((tw + PAD_X) * SQRT_2, (th + PAD_Y) * SQRT_2),
        Geo::Diamond | Geo::Triangle | Geo::Flipped => (2.0 * w0 - 2.0 * PAD_X, 2.0 * h0),
        Geo::Text => (tw + 4.0, th + 4.0),
        Geo::Sloped => (w0, h0 + SLOPE),
        Geo::Actor => (
            tw.max(FIGURE_W) + 2.0 * ACTOR_PAD,
            FIGURE_H + FIGURE_GAP + th + 2.0 * ACTOR_PAD,
        ),
        Geo::UseCase => (tw + 2.0 * USECASE_PAD, th + 2.0 * USECASE_PAD),
        Geo::Collapsed => (
            (tw + 2.0 * COLLAPSED_PAD).max(COLLAPSED_MIN_W),
            th + COLLAPSED_GAP + COLLAPSED_ROW + 2.0 * COLLAPSED_PAD,
        ),
    }
}

/// Where the label is centered within a node's box.
pub(super) fn text_box(shape: Shape, b: BoxF) -> BoxF {
    match geo(shape) {
        Geo::Cylinder => {
            let ry = cylinder_ry(b.w);
            BoxF {
                y: b.y + 2.0 * ry,
                h: b.h - 3.0 * ry,
                ..b
            }
        }
        Geo::Triangle => BoxF {
            y: b.y + b.h / 2.0,
            h: b.h / 2.0,
            ..b
        },
        Geo::Flipped => BoxF { h: b.h / 2.0, ..b },
        Geo::Collapsed => BoxF {
            h: (b.h - COLLAPSED_GAP - COLLAPSED_ROW).max(0.0),
            ..b
        },
        Geo::Sloped => BoxF {
            y: b.y + SLOPE,
            h: b.h - SLOPE,
            ..b
        },
        Geo::Actor => {
            let top = ACTOR_PAD + FIGURE_H + FIGURE_GAP;
            BoxF {
                y: b.y + top,
                h: (b.h - top - ACTOR_PAD).max(0.0),
                ..b
            }
        }
        _ => b,
    }
}

/// The business slash across a use case or an actor's head, for the `business` metadata.
pub(super) fn business_marker(shape: Shape, b: BoxF) -> Option<Vec<Segment>> {
    match shape {
        Shape::UseCase => {
            let (c, rx, ry) = (b.center(), b.w / 2.0, b.h / 2.0);
            let inset = 2.0;
            let (sx, ex) = ((rx - USECASE_PAD + inset).max(0.0), rx - inset);
            let y_at = |x: f32| ry * (1.0 - (x / rx).powi(2)).max(0.0).sqrt();
            Some(line((c.x + sx, c.y + y_at(sx)), (c.x + ex, c.y - y_at(ex))))
        }
        Shape::Actor | Shape::HollowActor => {
            let (cy, r) = if shape == Shape::Actor {
                (-24.0, 12.0)
            } else {
                (-23.0, 9.0)
            };
            let (dx, dy) = (0.5_f32, -(3.0_f32.sqrt()) / 2.0);
            let (off, half) = (r * 0.6, r * 0.8);
            let (mx, my) = (off * -dy, cy + off * dx);
            let o = figure_origin(b);
            let p = |x: f32, y: f32| (o.x + x, o.y + y);
            Some(line(
                p(mx - half * dx, my - half * dy),
                p(mx + half * dx, my + half * dy),
            ))
        }
        _ => None,
    }
}

/// Where an actor's figure is drawn from: its coordinates run from y = -36 to 30.
fn figure_origin(b: BoxF) -> Pt {
    Pt::new(b.center().x, b.y + ACTOR_PAD + FIGURE_H / 2.0)
}

/// An actor's figure, as Mermaid's `usecaseActor`, `usecaseActorHollow`, and
/// `usecaseActorAwesome` draw it. The figure paths and the business slash are ported from Mermaid
/// (Copyright (c) 2014-2022 Knut Sveidqvist; MIT); see THIRD-PARTY-NOTICES.
fn figure(shape: Shape, b: BoxF, out: &mut Outline) {
    let o = figure_origin(b);
    let p = |x: f32, y: f32| (o.x + x, o.y + y);
    let pt = |x: f32, y: f32| Pt::new(o.x + x, o.y + y);
    match shape {
        Shape::HollowActor => {
            out.lines.push(ellipse(pt(0.0, -23.0), 9.0, 9.0));
            out.lines.push(polygon(&[
                p(-22.0, -10.0),
                p(22.0, -10.0),
                p(22.0, 0.0),
                p(6.0, 0.0),
                p(22.0, 17.0),
                p(13.0, 28.0),
                p(0.0, 13.0),
                p(-13.0, 28.0),
                p(-22.0, 17.0),
                p(-6.0, 0.0),
                p(-22.0, 0.0),
            ]));
        }
        Shape::AwesomeActor => {
            out.dots.push(ellipse(pt(0.0, -21.0), 13.0, 13.0));
            let c = |x1, y1, x2, y2, x, y| Segment::Cubic {
                c1: pt(x1, y1),
                c2: pt(x2, y2),
                to: pt(x, y),
            };
            out.dots.push(vec![
                Segment::Move(pt(-24.0, 25.0)),
                c(-24.0, 7.0, -14.0, -3.0, 0.0, -3.0),
                c(14.0, -3.0, 24.0, 7.0, 24.0, 25.0),
                c(24.0, 28.0, 21.0, 30.0, 18.0, 30.0),
                Segment::Line(pt(-18.0, 30.0)),
                c(-21.0, 30.0, -24.0, 28.0, -24.0, 25.0),
                Segment::Close,
            ]);
        }
        _ => {
            out.lines.push(ellipse(pt(0.0, -24.0), 12.0, 12.0));
            out.lines.push(line(p(0.0, -12.0), p(0.0, 8.0)));
            out.lines.push(line(p(-17.0, -5.0), p(17.0, -5.0)));
            out.lines.push(line(p(0.0, 8.0), p(-15.0, 28.0)));
            out.lines.push(line(p(0.0, 8.0), p(15.0, 28.0)));
        }
    }
}

/// A node's drawing.
#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct Outline {
    /// The filled and stroked body; empty for shapes drawn as bare text.
    pub body: Vec<Segment>,
    /// The body is filled with the stroke color.
    pub solid: bool,
    /// Extra strokes drawn over the body.
    pub lines: Vec<Vec<Segment>>,
    /// Extra shapes filled with the stroke color.
    pub dots: Vec<Vec<Segment>>,
    /// The body's boundary as a polygon, for clipping edges.
    pub boundary: Vec<Pt>,
}

pub(super) fn polygon(points: &[(f32, f32)]) -> Vec<Segment> {
    let mut out: Vec<Segment> = points
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| {
            if i == 0 {
                Segment::Move(Pt::new(x, y))
            } else {
                Segment::Line(Pt::new(x, y))
            }
        })
        .collect();
    out.push(Segment::Close);
    out
}

fn line(a: (f32, f32), b: (f32, f32)) -> Vec<Segment> {
    vec![
        Segment::Move(Pt::new(a.0, a.1)),
        Segment::Line(Pt::new(b.0, b.1)),
    ]
}

fn arc(rx: f32, ry: f32, sweep: bool, x: f32, y: f32) -> Segment {
    Segment::Arc {
        radii: Pt::new(rx, ry),
        large: false,
        sweep,
        to: Pt::new(x, y),
    }
}

/// A full ellipse as two half arcs.
pub(super) fn ellipse(c: Pt, rx: f32, ry: f32) -> Vec<Segment> {
    vec![
        Segment::Move(Pt::new(c.x - rx, c.y)),
        arc(rx, ry, true, c.x + rx, c.y),
        arc(rx, ry, true, c.x - rx, c.y),
        Segment::Close,
    ]
}

/// A rectangle with corners rounded by `r`.
pub(super) fn rounded(b: BoxF, r: f32) -> Vec<Segment> {
    let r = r.min(b.w / 2.0).min(b.h / 2.0);
    let (x, y, right, bottom) = (b.x, b.y, b.right(), b.bottom());
    vec![
        Segment::Move(Pt::new(x + r, y)),
        Segment::Line(Pt::new(right - r, y)),
        arc(r, r, true, right, y + r),
        Segment::Line(Pt::new(right, bottom - r)),
        arc(r, r, true, right - r, bottom),
        Segment::Line(Pt::new(x + r, bottom)),
        arc(r, r, true, x, bottom - r),
        Segment::Line(Pt::new(x, y + r)),
        arc(r, r, true, x + r, y),
        Segment::Close,
    ]
}

/// The drawing of a node of `shape` filling `b`.
pub(super) fn outline(shape: Shape, b: BoxF) -> Outline {
    let (x, y, r, bottom) = (b.x, b.y, b.right(), b.bottom());
    let Pt { x: cx, y: cy } = b.center();
    let (rx, ry) = (b.w / 2.0, b.h / 2.0);
    let mut out = Outline::default();
    let rect = [(x, y), (r, y), (r, bottom), (x, bottom)];
    out.body = match geo(shape) {
        Geo::Rect => polygon(&rect),
        Geo::Rounded => rounded(b, 6.0),
        Geo::Stadium => rounded(b, b.h / 2.0),
        Geo::Framed => {
            out.lines.push(line((x + FRAME, y), (x + FRAME, bottom)));
            out.lines.push(line((r - FRAME, y), (r - FRAME, bottom)));
            polygon(&rect)
        }
        Geo::Cylinder => {
            let e = cylinder_ry(b.w);
            out.lines.push(vec![
                Segment::Move(Pt::new(x, y + e)),
                arc(rx, e, false, r, y + e),
            ]);
            vec![
                Segment::Move(Pt::new(x, y + e)),
                arc(rx, e, true, r, y + e),
                Segment::Line(Pt::new(r, bottom - e)),
                arc(rx, e, true, x, bottom - e),
                Segment::Close,
            ]
        }
        Geo::Circle | Geo::Ellipse | Geo::UseCase => ellipse(b.center(), rx, ry),
        Geo::Actor => {
            figure(shape, b, &mut out);
            Vec::new()
        }
        Geo::Crossed => {
            let (dx, dy) = (rx * FRAC_1_SQRT_2, ry * FRAC_1_SQRT_2);
            out.lines.push(line((cx - dx, cy - dy), (cx + dx, cy + dy)));
            out.lines.push(line((cx - dx, cy + dy), (cx + dx, cy - dy)));
            ellipse(b.center(), rx, ry)
        }
        Geo::Double => {
            out.lines
                .push(ellipse(b.center(), rx - 5.0, ry - 5.0).to_vec());
            ellipse(b.center(), rx, ry)
        }
        // Round whatever room the layout gave them.
        Geo::Dot => {
            out.solid = true;
            let r = rx.min(ry);
            ellipse(b.center(), r, r)
        }
        Geo::Stop => {
            let r = rx.min(ry);
            out.dots.push(ellipse(b.center(), r - 4.0, r - 4.0));
            ellipse(b.center(), r, r)
        }
        Geo::Diamond => polygon(&[(cx, y), (r, cy), (cx, bottom), (x, cy)]),
        Geo::Hexagon => {
            let s = b.h / 4.0;
            polygon(&[
                (x + s, y),
                (r - s, y),
                (r, cy),
                (r - s, bottom),
                (x + s, bottom),
                (x, cy),
            ])
        }
        Geo::LeanRight => {
            let s = b.h / 2.0;
            polygon(&[(x + s, y), (r, y), (r - s, bottom), (x, bottom)])
        }
        Geo::LeanLeft => {
            let s = b.h / 2.0;
            polygon(&[(x, y), (r - s, y), (r, bottom), (x + s, bottom)])
        }
        Geo::TrapBottom => {
            let s = b.h / 2.0;
            polygon(&[(x + s, y), (r - s, y), (r, bottom), (x, bottom)])
        }
        Geo::TrapTop => {
            let s = b.h / 2.0;
            polygon(&[(x, y), (r, y), (r - s, bottom), (x + s, bottom)])
        }
        Geo::Odd => polygon(&[
            (x, y),
            (r, y),
            (r, bottom),
            (x, bottom),
            (x + b.h / 4.0, cy),
        ]),
        Geo::Text => Vec::new(),
        Geo::Triangle => polygon(&[(cx, y), (r, bottom), (x, bottom)]),
        Geo::Flipped => polygon(&[(x, y), (r, y), (cx, bottom)]),
        Geo::Fork => {
            out.solid = true;
            polygon(&rect)
        }
        Geo::Notched => polygon(&[
            (x + NOTCH, y),
            (r, y),
            (r, bottom),
            (x, bottom),
            (x, y + NOTCH),
        ]),
        Geo::Sloped => polygon(&[(x, y + SLOPE), (r, y), (r, bottom), (x, bottom)]),
        Geo::Collapsed => {
            // A dashed rule above the indicator row, then three dots centered in it.
            let rule = bottom - COLLAPSED_PAD - COLLAPSED_ROW;
            let mut at = x + 8.0;
            while at < r - 8.0 {
                out.lines
                    .push(line((at, rule), ((at + 3.0).min(r - 8.0), rule)));
                at += 6.0;
            }
            for k in [-1.0, 0.0, 1.0] {
                out.dots.push(ellipse(
                    Pt::new(cx + 10.0 * k, rule + COLLAPSED_ROW / 2.0),
                    2.5,
                    2.5,
                ));
            }
            rounded(b, 8.0)
        }
    };
    out.boundary = if geo(shape) == Geo::Actor {
        // Edges meet the figure, not the label below it.
        let o = figure_origin(b);
        let (l, t) = (o.x - FIGURE_W / 2.0, b.y + ACTOR_PAD);
        let (r, bottom) = (o.x + FIGURE_W / 2.0, t + FIGURE_H);
        vec![
            Pt::new(l, t),
            Pt::new(r, t),
            Pt::new(r, bottom),
            Pt::new(l, bottom),
        ]
    } else if out.body.is_empty() {
        rect.iter().map(|&(x, y)| Pt::new(x, y)).collect()
    } else {
        flatten(&out.body)
    };
    out
}

/// The points of a path, with arcs sampled. Arcs follow SVG's endpoint parameterization
/// without rotation.
pub(super) fn flatten(segments: &[Segment]) -> Vec<Pt> {
    let mut out = Vec::new();
    let mut at = Pt::default();
    for s in segments {
        match *s {
            Segment::Move(p) | Segment::Line(p) => {
                out.push(p);
                at = p;
            }
            Segment::Arc {
                radii,
                large,
                sweep,
                to,
            } => {
                out.extend(arc_points(at, radii, large, sweep, to));
                at = to;
            }
            Segment::Cubic { c1, c2, to } => {
                for i in 1..=8 {
                    let t = i as f32 / 8.0;
                    let u = 1.0 - t;
                    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    out.push(Pt::new(
                        a * at.x + b * c1.x + c * c2.x + d * to.x,
                        a * at.y + b * c1.y + c * c2.y + d * to.y,
                    ));
                }
                at = to;
            }
            Segment::Close => {}
        }
    }
    out
}

/// Points along an arc from `from` to `to`, excluding `from`.
fn arc_points(from: Pt, radii: Pt, large: bool, sweep: bool, to: Pt) -> Vec<Pt> {
    let (mut rx, mut ry) = (radii.x.abs(), radii.y.abs());
    if rx == 0.0 || ry == 0.0 || from == to {
        return vec![to];
    }
    // SVG 1.1 F.6.5, with no rotation.
    let x1 = (from.x - to.x) / 2.0;
    let y1 = (from.y - to.y) / 2.0;
    let lambda = (x1 / rx).powi(2) + (y1 / ry).powi(2);
    if lambda > 1.0 {
        rx *= lambda.sqrt();
        ry *= lambda.sqrt();
    }
    let num = (rx * ry).powi(2) - (rx * y1).powi(2) - (ry * x1).powi(2);
    let den = (rx * y1).powi(2) + (ry * x1).powi(2);
    let mut k = (num / den).max(0.0).sqrt();
    if large == sweep {
        k = -k;
    }
    let cx1 = k * rx * y1 / ry;
    let cy1 = -k * ry * x1 / rx;
    let c = Pt::new(cx1 + (from.x + to.x) / 2.0, cy1 + (from.y + to.y) / 2.0);
    let angle = |ux: f32, uy: f32| uy.atan2(ux);
    let start = angle((x1 - cx1) / rx, (y1 - cy1) / ry);
    let end = angle((-x1 - cx1) / rx, (-y1 - cy1) / ry);
    let mut delta = end - start;
    if sweep && delta < 0.0 {
        delta += 2.0 * PI;
    } else if !sweep && delta > 0.0 {
        delta -= 2.0 * PI;
    }
    // The tolerance keeps a sweep within rounding of a whole number of steps (a half circle
    // is 16) at that number, as `atan2` rounds differently across platforms.
    let steps = ((delta.abs() / (PI / 16.0) - 1e-3).ceil() as usize).max(1);
    (1..=steps)
        .map(|i| {
            if i == steps {
                return to;
            }
            let t = start + delta * i as f32 / steps as f32;
            Pt::new(c.x + rx * t.cos(), c.y + ry * t.sin())
        })
        .collect()
}

/// The first point where the ray from `origin` along `dir` meets `polygon`'s outline, at a
/// distance of at least `min`.
pub(super) fn ray_hit(polygon: &[Pt], origin: Pt, dir: Pt, min: f32) -> Option<Pt> {
    let n = polygon.len();
    let mut best: Option<f32> = None;
    for i in 0..n {
        let (a, b) = (polygon[i], polygon[(i + 1) % n]);
        let e = Pt::new(b.x - a.x, b.y - a.y);
        let den = dir.x * e.y - dir.y * e.x;
        if den.abs() < 1e-6 {
            continue;
        }
        let w = Pt::new(a.x - origin.x, a.y - origin.y);
        let t = (w.x * e.y - w.y * e.x) / den;
        let u = (w.x * dir.y - w.y * dir.x) / den;
        if (-1e-4..=1.0 + 1e-4).contains(&u) && t >= min && best.is_none_or(|bt| t < bt) {
            best = Some(t);
        }
    }
    best.map(|t| Pt::new(origin.x + dir.x * t, origin.y + dir.y * t))
}
