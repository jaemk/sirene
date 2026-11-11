//! Mermaid's expanded node shapes, the ones written only with `@{ shape }` (DISPLA-7):
//! documents, stacked and tagged shapes, cylinders on their side, stored data, braces, and the
//! window and figure shapes. The geometry is ported from Mermaid 12.3.0's
//! `rendering-util/rendering-elements/shapes/` (Copyright (c) 2014-2022 Knut Sveidqvist; MIT);
//! see THIRD-PARTY-NOTICES.

use std::f32::consts::PI;

use super::shapes::{BoxF, Outline, PAD_X, PAD_Y, ellipse, flatten, polygon, rounded};
use super::{Pt, Segment};
use crate::flowchart::Shape;

/// Whether `shape` is drawn here.
pub(super) fn is_expanded(shape: Shape) -> bool {
    matches!(
        shape,
        Shape::Document
            | Shape::LinedDocument
            | Shape::Documents
            | Shape::TaggedDocument
            | Shape::Flag
            | Shape::Delay
            | Shape::HorizontalCylinder
            | Shape::CurvedTrapezoid
            | Shape::DividedRect
            | Shape::WindowPane
            | Shape::NotchedPentagon
            | Shape::StackedRect
            | Shape::BowTieRect
            | Shape::TaggedRect
            | Shape::LinedRect
            | Shape::Hourglass
            | Shape::Bolt
            | Shape::Brace
            | Shape::BraceRight
            | Shape::Braces
            | Shape::DataStore
            | Shape::Folder
            | Shape::Bucket
            | Shape::Console
            | Shape::Browser
            | Shape::Person
            | Shape::Bang
            | Shape::Cloud
    )
}

/// Whether the shape shows its label: Mermaid draws an hourglass and a bolt without one.
pub(super) fn has_label(shape: Shape) -> bool {
    !matches!(shape, Shape::Hourglass | Shape::Bolt)
}

/// The room a stacked document's and a stacked rectangle's back sheets take.
const STACK: f32 = 10.0;
const STACK_RECT: f32 = 5.0;
/// A window pane's and a lined rectangle's frame.
const PANE: f32 = 10.0;
const LINED: f32 = 8.0;
/// The bars above a console's and a browser's label.
const CONSOLE_BAND: f32 = 20.0;
const BROWSER_BAR: f32 = 18.0;
const WINDOW_RADIUS: f32 = 12.0;

/// A side cylinder's and stored data's end radius for a height of `h`.
fn end_rx(h: f32) -> f32 {
    (h / 2.0) / (2.5 + h / 50.0)
}

/// How far a label's corners reach into a cap of radius `r` on a body of label height `th`.
fn cap_clearance(r: f32, th: f32) -> f32 {
    r - (r * r - (th / 2.0).powi(2)).max(0.0).sqrt()
}

fn brace_radius(h: f32) -> f32 {
    (0.1 * h).max(5.0)
}

fn folder_tab(h: f32) -> f32 {
    (h * 0.16).clamp(8.0, 14.0)
}

fn bucket_rim(w: f32) -> f32 {
    (w * 0.08).clamp(5.0, 12.0)
}

/// A stacked document's label height before its waves, from its box height.
fn documents_label_h(h: f32) -> f32 {
    (h - 2.0 * STACK) / 1.1875
}

/// The size of a node whose label measures `tw` by `th`.
pub(super) fn size(shape: Shape, tw: f32, th: f32) -> (f32, f32) {
    let (w0, h0) = (tw + 2.0 * PAD_X, th + 2.0 * PAD_Y);
    match shape {
        Shape::Document => (w0, h0 * 9.0 / 8.0),
        Shape::LinedDocument | Shape::TaggedDocument => (w0 * 1.1, h0 * 9.0 / 8.0),
        Shape::Documents => (w0 + 2.0 * STACK, (th + 3.0 * PAD_Y) * 1.1875 + 2.0 * STACK),
        Shape::Flag => (w0, h0 * 1.25),
        Shape::Delay => (w0 + 2.0 * cap_clearance(h0 / 2.0, th), h0),
        Shape::HorizontalCylinder => (w0 + 3.0 * end_rx(h0), h0),
        Shape::CurvedTrapezoid => {
            let side = (h0 / 4.0).max(cap_clearance(h0 / 2.0, th));
            ((w0 + 2.0 * side).max(w0 * 1.25), h0)
        }
        Shape::DividedRect => (w0, h0 * 1.2),
        Shape::WindowPane => (w0 + PANE, h0 + PANE),
        Shape::StackedRect => (w0 + 2.0 * STACK_RECT, h0 + 2.0 * STACK_RECT),
        Shape::BowTieRect => (w0 + 2.0 * end_rx(h0), h0),
        Shape::TaggedRect => (w0 + 0.2 * h0, h0),
        Shape::LinedRect => (w0 + 2.0 * LINED, h0),
        Shape::Hourglass => (30.0, 30.0),
        Shape::Bolt => (35.0, 70.0),
        Shape::Brace | Shape::BraceRight => (w0 + 2.0 * brace_radius(h0), h0),
        Shape::Braces => (w0 + 4.0 * brace_radius(h0), h0),
        Shape::Folder => (w0.max(90.0), h0 + folder_tab(h0)),
        Shape::Bucket => {
            let w = w0.max(80.0);
            (w, h0 + 2.0 * bucket_rim(w))
        }
        Shape::Console => (w0.max(90.0), h0 + CONSOLE_BAND),
        Shape::Browser => (w0.max(90.0), h0 + BROWSER_BAR),
        Shape::Person => {
            let w = w0.max(100.0);
            let (head, _) = person_head(w);
            (w, h0 + 2.0 * head * 0.73)
        }
        Shape::Bang => extent(&bang(tw + 40.0, th + 32.0)),
        Shape::Cloud => extent(&cloud(tw + PAD_Y, th + PAD_Y)),
        // Rectangles: the datastore, the notched pentagon.
        _ => (w0, h0),
    }
}

/// Where the label is centered within a node's box.
pub(super) fn text_box(shape: Shape, b: BoxF) -> BoxF {
    match shape {
        Shape::Document | Shape::TaggedDocument => BoxF {
            h: b.h * 8.0 / 9.0,
            ..b
        },
        Shape::LinedDocument => {
            let inset = b.w / 22.0;
            BoxF {
                x: b.x + inset,
                w: b.w - inset,
                h: b.h * 8.0 / 9.0,
                ..b
            }
        }
        Shape::Documents => {
            let amp = documents_label_h(b.h) / 8.0;
            let top = b.y + 2.0 * STACK;
            BoxF {
                y: top,
                w: b.w - 2.0 * STACK,
                h: b.bottom() - amp - top,
                ..b
            }
        }
        Shape::HorizontalCylinder => {
            let rx = end_rx(b.h);
            BoxF {
                x: b.x + rx,
                w: b.w - 3.0 * rx,
                ..b
            }
        }
        Shape::BowTieRect => {
            let rx = end_rx(b.h);
            BoxF {
                x: b.x + rx,
                w: b.w - 2.0 * rx,
                ..b
            }
        }
        Shape::DividedRect => BoxF {
            y: b.y + b.h / 6.0,
            h: b.h * 5.0 / 6.0,
            ..b
        },
        Shape::WindowPane => BoxF {
            x: b.x + PANE,
            y: b.y + PANE,
            w: b.w - PANE,
            h: b.h - PANE,
        },
        Shape::StackedRect => BoxF {
            y: b.y + 2.0 * STACK_RECT,
            w: b.w - 2.0 * STACK_RECT,
            h: b.h - 2.0 * STACK_RECT,
            ..b
        },
        Shape::LinedRect => BoxF {
            x: b.x + LINED,
            w: b.w - LINED,
            ..b
        },
        Shape::TaggedRect => BoxF {
            w: b.w - 0.2 * b.h,
            ..b
        },
        Shape::Brace => {
            let r = 2.0 * brace_radius(b.h);
            BoxF {
                x: b.x + r,
                w: b.w - r,
                ..b
            }
        }
        Shape::BraceRight => BoxF {
            w: b.w - 2.0 * brace_radius(b.h),
            ..b
        },
        Shape::Braces => {
            let r = 2.0 * brace_radius(b.h);
            BoxF {
                x: b.x + r,
                w: b.w - 2.0 * r,
                ..b
            }
        }
        Shape::Folder => {
            let tab = folder_tab(b.h / 1.16);
            BoxF {
                y: b.y + tab,
                h: b.h - tab,
                ..b
            }
        }
        Shape::Bucket => {
            let rim = bucket_rim(b.w);
            BoxF {
                y: b.y + rim,
                h: b.h - 2.0 * rim,
                ..b
            }
        }
        Shape::Console => BoxF {
            y: b.y + CONSOLE_BAND,
            h: b.h - CONSOLE_BAND,
            ..b
        },
        Shape::Browser => BoxF {
            y: b.y + BROWSER_BAR,
            h: b.h - BROWSER_BAR,
            ..b
        },
        Shape::Person => {
            let (head, overlap) = person_head(b.w);
            let top = b.y + 2.0 * head - overlap;
            BoxF {
                y: top,
                h: b.bottom() - top,
                ..b
            }
        }
        Shape::Bang => fit(bang, b).1,
        Shape::Cloud => fit(cloud, b).1,
        _ => b,
    }
}

/// The head radius and the head's overlap with the body of a person `w` wide.
fn person_head(w: f32) -> (f32, f32) {
    let head = (w * 0.23).clamp(16.0, 56.0);
    (head, head * 0.27)
}

fn arc(r: (f32, f32), large: bool, sweep: bool, to: (f32, f32)) -> Segment {
    Segment::Arc {
        radii: Pt::new(r.0, r.1),
        large,
        sweep,
        to: Pt::new(to.0, to.1),
    }
}

fn mv(p: (f32, f32)) -> Segment {
    Segment::Move(Pt::new(p.0, p.1))
}

fn ln(p: (f32, f32)) -> Segment {
    Segment::Line(Pt::new(p.0, p.1))
}

fn open(points: &[(f32, f32)]) -> Vec<Segment> {
    let mut out = polygon(points);
    out.pop();
    out
}

/// Mermaid's `generateFullSineWavePoints`: 51 points from `x1` to `x2` around `y`.
fn wave(x1: f32, x2: f32, y: f32, amp: f32, cycles: f32) -> Vec<(f32, f32)> {
    let dx = x2 - x1;
    let freq = 2.0 * PI * cycles / dx;
    (0..=50)
        .map(|i| {
            let x = x1 + dx * i as f32 / 50.0;
            (x, y + amp * (freq * (x - x1)).sin())
        })
        .collect()
}

/// The y of a wave at `x`, as `wave` draws it.
fn wave_at(x1: f32, x2: f32, y: f32, amp: f32, cycles: f32, x: f32) -> f32 {
    y + amp * (2.0 * PI * cycles / (x2 - x1) * (x - x1)).sin()
}

/// A path drawn with relative arcs, as Mermaid writes the cloud and the bang.
struct Rel {
    at: (f32, f32),
    out: Vec<Segment>,
}

impl Rel {
    fn new() -> Rel {
        Rel {
            at: (0.0, 0.0),
            out: vec![mv((0.0, 0.0))],
        }
    }

    fn arc(&mut self, rx: f32, ry: f32, sweep: bool, dx: f32, dy: f32) {
        self.at = (self.at.0 + dx, self.at.1 + dy);
        self.out.push(arc((rx, ry), false, sweep, self.at));
    }

    fn close(mut self) -> Vec<Segment> {
        self.out.push(Segment::Close);
        self.out
    }
}

/// Mermaid's `cloud` for a label box `w` by `h`.
fn cloud(w: f32, h: f32) -> Vec<Segment> {
    let (r1, r2, r3, r4) = (0.15 * w, 0.25 * w, 0.35 * w, 0.2 * w);
    let mut p = Rel::new();
    p.arc(r1, r1, true, w * 0.25, -w * 0.1);
    p.arc(r3, r3, true, w * 0.4, -w * 0.1);
    p.arc(r2, r2, true, w * 0.35, w * 0.2);
    p.arc(r1, r1, true, w * 0.15, h * 0.35);
    p.arc(r4, r4, true, -w * 0.15, h * 0.65);
    p.arc(r2, r1, true, -w * 0.25, w * 0.15);
    p.arc(r3, r3, true, -w * 0.5, 0.0);
    p.arc(r1, r1, true, -w * 0.25, -w * 0.15);
    p.arc(r1, r1, true, -w * 0.1, -h * 0.35);
    p.arc(r4, r4, true, w * 0.1, -h * 0.65);
    p.close()
}

/// Mermaid's `bang` for a box `w` by `h`.
fn bang(w: f32, h: f32) -> Vec<Segment> {
    let r = 0.15 * w;
    let mut p = Rel::new();
    p.arc(r, r, false, w * 0.25, -h * 0.1);
    p.arc(r, r, false, w * 0.25, 0.0);
    p.arc(r, r, false, w * 0.25, 0.0);
    p.arc(r, r, false, w * 0.25, h * 0.1);
    p.arc(r, r, false, w * 0.15, h * 0.33);
    p.arc(r * 0.8, r * 0.8, false, 0.0, h * 0.34);
    p.arc(r, r, false, -w * 0.15, h * 0.33);
    p.arc(r, r, false, -w * 0.25, h * 0.15);
    p.arc(r, r, false, -w * 0.25, 0.0);
    p.arc(r, r, false, -w * 0.25, 0.0);
    p.arc(r, r, false, -w * 0.25, -h * 0.15);
    p.arc(r, r, false, -w * 0.1, -h * 0.33);
    p.arc(r * 0.8, r * 0.8, false, 0.0, -h * 0.34);
    p.arc(r, r, false, w * 0.1, -h * 0.33);
    p.close()
}

fn bounds(segments: &[Segment]) -> (f32, f32, f32, f32) {
    flatten(segments).iter().fold(
        (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
        |(x0, y0, x1, y1), p| (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)),
    )
}

fn extent(segments: &[Segment]) -> (f32, f32) {
    let (x0, y0, x1, y1) = bounds(segments);
    (x1 - x0, y1 - y0)
}

/// `build`'s path sized to fill `b`, and where its `w` by `h` label box lands.
fn fit(build: fn(f32, f32) -> Vec<Segment>, b: BoxF) -> (Vec<Segment>, BoxF) {
    // The path's height depends on both `w` and `h`, so step each by its own slope.
    let (mut w, mut h) = (b.w.max(1.0), b.h.max(1.0));
    for _ in 0..8 {
        let (ew, eh) = extent(&build(w, h));
        let (dw, _) = extent(&build(w + 1.0, h));
        let (_, dh) = extent(&build(w, h + 1.0));
        if dw - ew > 1e-3 {
            w = (w + (b.w - ew) / (dw - ew)).max(1.0);
        }
        if dh - eh > 1e-3 {
            h = (h + (b.h - eh) / (dh - eh)).max(1.0);
        }
    }
    let path = build(w, h);
    let (x0, y0, x1, y1) = bounds(&path);
    let sx = if x1 > x0 { b.w / (x1 - x0) } else { 1.0 };
    let sy = if y1 > y0 { b.h / (y1 - y0) } else { 1.0 };
    let map = |p: Pt| Pt::new(b.x + (p.x - x0) * sx, b.y + (p.y - y0) * sy);
    let path = path
        .into_iter()
        .map(|s| match s {
            Segment::Move(p) => Segment::Move(map(p)),
            Segment::Line(p) => Segment::Line(map(p)),
            Segment::Arc {
                radii,
                large,
                sweep,
                to,
            } => Segment::Arc {
                radii: Pt::new(radii.x * sx, radii.y * sy),
                large,
                sweep,
                to: map(to),
            },
            Segment::Cubic { c1, c2, to } => Segment::Cubic {
                c1: map(c1),
                c2: map(c2),
                to: map(to),
            },
            Segment::Close => Segment::Close,
        })
        .collect();
    let origin = map(Pt::new(0.0, 0.0));
    let label = BoxF {
        x: origin.x,
        y: origin.y,
        w: w * sx,
        h: h * sy,
    };
    (path, label)
}

/// A left brace down the left side of `b` (or a right one down its right side).
fn brace(b: BoxF, left: bool) -> Vec<Segment> {
    let r = brace_radius(b.h);
    let cy = b.center().y;
    let (edge, s) = if left { (b.x, 1.0) } else { (b.right(), -1.0) };
    let x = |k: f32| edge + s * k * r;
    // Mirroring flips the turns.
    let cw = |t: bool| t == left;
    vec![
        mv((x(2.0), b.y)),
        arc((r, r), false, cw(false), (x(1.0), b.y + r)),
        ln((x(1.0), cy - r)),
        arc((r, r), false, cw(true), (x(0.0), cy)),
        arc((r, r), false, cw(true), (x(1.0), cy + r)),
        ln((x(1.0), b.bottom() - r)),
        arc((r, r), false, cw(false), (x(2.0), b.bottom())),
    ]
}

/// The drawing of a node of `shape` filling `b`: the body, with extra strokes in `out`.
pub(super) fn outline(shape: Shape, b: BoxF, out: &mut Outline) -> Vec<Segment> {
    let (x, y, r, bottom) = (b.x, b.y, b.right(), b.bottom());
    let Pt { x: cx, y: cy } = b.center();
    let rect = [(x, y), (r, y), (r, bottom), (x, bottom)];
    match shape {
        Shape::Document | Shape::LinedDocument | Shape::TaggedDocument => {
            let amp = b.h / 9.0;
            let base = bottom - amp;
            let mut pts = vec![(x, base)];
            pts.extend(wave(x, r, base, amp, 0.8));
            pts.extend([(r, y), (x, y)]);
            if shape == Shape::LinedDocument {
                let lx = x + b.w / 22.0;
                out.lines
                    .push(open(&[(lx, y), (lx, wave_at(x, r, base, amp, 0.8, lx))]));
            }
            if shape == Shape::TaggedDocument {
                let (tw, th) = (0.2 * b.w / 1.1, 0.2 * b.h * 8.0 / 9.0);
                let end = wave_at(x, r, base, amp, 0.8, r);
                out.lines.push(polygon(&[
                    (r, end - th),
                    (r, end),
                    (r - tw, wave_at(x, r, base, amp, 0.8, r - tw)),
                ]));
            }
            polygon(&pts)
        }
        Shape::Documents => {
            let o = STACK;
            let amp = documents_label_h(b.h) / 8.0;
            let (fx, fy, w) = (x + o, y + o, b.w - 2.0 * o);
            let base = bottom - amp;
            let front = wave(x, fx + w - o, base, amp, 0.8);
            let last = front[front.len() - 1].1;
            let mut pts = vec![(x, fy + o)];
            pts.extend(front);
            pts.extend([
                (fx + w - o, last - o),
                (fx + w, last - o),
                (fx + w, last - 2.0 * o),
                (r, last - 2.0 * o),
                (r, y),
                (fx + o, y),
                (fx + o, fy),
                (fx, fy),
                (fx, fy + o),
            ]);
            out.lines.push(polygon(&[
                (fx, fy + o),
                (fx + w - o, fy + o),
                (fx + w - o, last - o),
                (fx + w, last - o),
                (fx + w, fy),
                (fx, fy),
            ]));
            polygon(&pts)
        }
        Shape::Flag => {
            let amp = b.h / 10.0;
            let mut pts = vec![(x, bottom - amp)];
            pts.extend(wave(x, r, bottom - amp, amp, 1.0));
            pts.push((r, y + amp));
            pts.extend(wave(r, x, y + amp, amp, -1.0));
            polygon(&pts)
        }
        Shape::Delay => {
            let rad = b.h / 2.0;
            let k = (r - rad).max(x);
            vec![
                mv((x, y)),
                ln((k, y)),
                arc((rad, rad), false, true, (k, bottom)),
                ln((x, bottom)),
                Segment::Close,
            ]
        }
        Shape::HorizontalCylinder => {
            let (rx, ry) = (end_rx(b.h), b.h / 2.0);
            let (x0, x1) = (x + rx, r - rx);
            out.lines
                .push(vec![mv((x1, y)), arc((rx, ry), false, false, (x1, bottom))]);
            vec![
                mv((x0, bottom)),
                arc((rx, ry), false, true, (x0, y)),
                ln((x1, y)),
                arc((rx, ry), false, true, (x1, bottom)),
                Segment::Close,
            ]
        }
        Shape::CurvedTrapezoid => {
            let rad = b.h / 2.0;
            let k = (r - rad).max(x + b.h / 4.0);
            vec![
                mv((k, y)),
                ln((x + b.h / 4.0, y)),
                ln((x, cy)),
                ln((x + b.h / 4.0, bottom)),
                ln((k, bottom)),
                arc((rad, rad), false, false, (k, y)),
                Segment::Close,
            ]
        }
        Shape::DividedRect => {
            out.lines
                .push(open(&[(x, y + b.h / 6.0), (r, y + b.h / 6.0)]));
            polygon(&rect)
        }
        Shape::WindowPane => {
            out.lines.push(open(&[(x, y + PANE), (r, y + PANE)]));
            out.lines.push(open(&[(x + PANE, y), (x + PANE, bottom)]));
            polygon(&rect)
        }
        Shape::NotchedPentagon => polygon(&[
            (x + 0.1 * b.w, y),
            (r - 0.1 * b.w, y),
            (r, y + 0.2 * b.h),
            (r, bottom),
            (x, bottom),
            (x, y + 0.2 * b.h),
        ]),
        Shape::StackedRect => {
            let o = STACK_RECT;
            let (fx, fy, w, h) = (x + o, y + o, b.w - 2.0 * o, b.h - 2.0 * o);
            out.lines.push(polygon(&[
                (fx, fy + o),
                (fx + w - o, fy + o),
                (fx + w - o, fy + h),
                (fx + w, fy + h),
                (fx + w, fy),
                (fx, fy),
            ]));
            polygon(&[
                (x, fy + o),
                (x, bottom),
                (fx + w - o, bottom),
                (fx + w - o, fy + h),
                (fx + w, fy + h),
                (fx + w, fy + h - o),
                (r, fy + h - o),
                (r, y),
                (fx + o, y),
                (fx + o, fy),
                (fx, fy),
                (fx, fy + o),
            ])
        }
        Shape::BowTieRect => {
            let (rx, ry) = (end_rx(b.h), b.h / 2.0);
            let xl = x + rx;
            vec![
                mv((r, y)),
                ln((xl, y)),
                arc((rx, ry), false, false, (xl, bottom)),
                ln((r, bottom)),
                arc((rx, ry), false, true, (r, y)),
                Segment::Close,
            ]
        }
        Shape::TaggedRect => {
            let t = 0.2 * b.h;
            out.lines
                .push(polygon(&[(r - t, bottom), (r, bottom), (r, bottom - t)]));
            polygon(&rect)
        }
        Shape::LinedRect => {
            out.lines.push(open(&[(x + LINED, y), (x + LINED, bottom)]));
            polygon(&rect)
        }
        Shape::Hourglass => {
            // Edges stop at the box, not at the waist where the outline crosses itself.
            out.boundary = rect.iter().map(|&(x, y)| Pt::new(x, y)).collect();
            polygon(&[(x, y), (r, y), (x, bottom), (r, bottom)])
        }
        Shape::Bolt => {
            let (w, h) = (b.w, b.h / 2.0);
            let g = 7.0 * b.w / 35.0;
            polygon(&[
                (x + w, y),
                (x, y + h + g / 2.0),
                (x + w - 2.0 * g, y + h + g / 2.0),
                (x, y + 2.0 * h),
                (x + w, y + h - g / 2.0),
                (x + 2.0 * g, y + h - g / 2.0),
            ])
        }
        Shape::Brace | Shape::BraceRight | Shape::Braces => {
            if shape != Shape::BraceRight {
                out.lines.push(brace(b, true));
            }
            if shape != Shape::Brace {
                out.lines.push(brace(b, false));
            }
            out.ghost = true;
            polygon(&rect)
        }
        Shape::DataStore => {
            out.bare = true;
            out.lines.push(open(&[(x, y), (r, y)]));
            out.lines.push(open(&[(x, bottom), (r, bottom)]));
            polygon(&rect)
        }
        Shape::Folder => {
            let tab = folder_tab(b.h / 1.16);
            let tw = (b.w * 0.38).max(28.0);
            polygon(&[
                (x, y),
                (x + tw, y),
                (x + tw, y + tab),
                (r, y + tab),
                (r, bottom),
                (x, bottom),
            ])
        }
        Shape::Bucket => {
            let rim = bucket_rim(b.w);
            let (top, base) = (y + rim, bottom - rim);
            let half = b.w * 0.36;
            out.lines.push(ellipse(Pt::new(cx, top), b.w / 2.0, rim));
            vec![
                mv((x, top)),
                ln((cx - half, base)),
                arc((half, rim), false, false, (cx + half, base)),
                ln((r, top)),
                arc((b.w / 2.0, rim), false, false, (x, top)),
                Segment::Close,
            ]
        }
        Shape::Console => {
            // The `>_` prompt in the top-left corner.
            out.lines.push(open(&[
                (x + 12.0, y + 6.0),
                (x + 18.0, y + 10.5),
                (x + 12.0, y + 15.0),
            ]));
            out.lines
                .push(open(&[(x + 20.0, y + 15.0), (x + 28.0, y + 15.0)]));
            rounded(b, WINDOW_RADIUS)
        }
        Shape::Browser => {
            let bar = y + BROWSER_BAR;
            out.lines.push(open(&[(x, bar), (r, bar)]));
            for i in 0..3 {
                out.dots.push(ellipse(
                    Pt::new(x + 12.0 + 9.0 * i as f32, y + BROWSER_BAR / 2.0),
                    2.5,
                    2.5,
                ));
            }
            out.lines.push(rounded(
                BoxF {
                    x: x + 44.0,
                    y: y + 4.0,
                    w: (b.w - 56.0).max(10.0),
                    h: BROWSER_BAR - 8.0,
                },
                3.0,
            ));
            rounded(b, WINDOW_RADIUS)
        }
        Shape::Person => {
            let (head, overlap) = person_head(b.w);
            let top = y + 2.0 * head - overlap;
            let body = BoxF {
                y: top,
                h: bottom - top,
                ..b
            };
            let radius = (b.w * 0.177).min(body.h * 0.45);
            let center = Pt::new(cx, y + head);
            out.parts.push(ellipse(center, head, head));
            // Edges meet the head's exposed arc and the body below it.
            let half = (head * head - (top - center.y).powi(2)).max(0.0).sqrt();
            let from = (top - center.y).atan2(-half);
            let to = (top - center.y).atan2(half) + 2.0 * PI;
            let mut boundary: Vec<Pt> = (0..=24)
                .map(|i| {
                    let t = from + (to - from) * i as f32 / 24.0;
                    Pt::new(center.x + head * t.cos(), center.y + head * t.sin())
                })
                .collect();
            let body_outline = rounded(body, radius);
            boundary.extend(flatten(&body_outline).into_iter().skip(1));
            out.boundary = boundary;
            body_outline
        }
        Shape::Bang => fit(bang, b).0,
        Shape::Cloud => fit(cloud, b).0,
        _ => polygon(&rect),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Shape; 28] = [
        Shape::Document,
        Shape::LinedDocument,
        Shape::Documents,
        Shape::TaggedDocument,
        Shape::Flag,
        Shape::Delay,
        Shape::HorizontalCylinder,
        Shape::CurvedTrapezoid,
        Shape::DividedRect,
        Shape::WindowPane,
        Shape::NotchedPentagon,
        Shape::StackedRect,
        Shape::BowTieRect,
        Shape::TaggedRect,
        Shape::LinedRect,
        Shape::Hourglass,
        Shape::Bolt,
        Shape::Brace,
        Shape::BraceRight,
        Shape::Braces,
        Shape::DataStore,
        Shape::Folder,
        Shape::Bucket,
        Shape::Console,
        Shape::Browser,
        Shape::Person,
        Shape::Bang,
        Shape::Cloud,
    ];

    /// Every shape stays in its box, and its label box sits inside it.
    #[test]
    fn shapes_fit_their_boxes() {
        for shape in ALL {
            assert!(is_expanded(shape), "{shape:?}");
            let (w, h) = size(shape, 60.0, 20.0);
            let b = BoxF {
                x: 10.0,
                y: 20.0,
                w,
                h,
            };
            let mut out = Outline::default();
            let body = outline(shape, b, &mut out);
            let mut all: Vec<Segment> = body.clone();
            for l in out.lines.iter().chain(&out.parts).chain(&out.dots) {
                all.extend(l.iter().copied());
            }
            let (x0, y0, x1, y1) = bounds(&all);
            let slack = 0.5;
            assert!(
                x0 >= b.x - slack && y0 >= b.y - slack,
                "{shape:?}: {x0},{y0} in {b:?}"
            );
            assert!(
                x1 <= b.right() + slack && y1 <= b.bottom() + slack,
                "{shape:?}: {x1},{y1} in {b:?}"
            );
            let t = text_box(shape, b);
            assert!(
                t.x >= b.x - slack && t.right() <= b.right() + slack,
                "{shape:?}"
            );
            assert!(
                t.y >= b.y - slack && t.bottom() <= b.bottom() + slack,
                "{shape:?}"
            );
            if has_label(shape) {
                assert!(
                    t.w >= 60.0 - slack && t.h >= 20.0 - slack,
                    "{shape:?}: {t:?}"
                );
            }
        }
    }
}
