//! Converts a laid out display list to scene paths (LMATH-4, LRES-T4).
//!
//! A glyph becomes the outline of its character in the KaTeX font, a rule a filled rectangle,
//! and an SVG path (a radical, a brace, a stretchy arrow) the parsed path data mapped through its
//! view box. Everything is filled; there are no strokes. Positions in the display list are in em
//! of the root font, with y growing down from the baseline; `size` is the pixel size of one em.

use ttf_parser::OutlineBuilder;

use super::{
    boxes::{Align, Fit, Glyph, Item, Path},
    fonts,
    metrics_data::Face,
};
use crate::scene::{Color, Primitive, Pt, Rect, Segment, parse_color};

/// A point in an SVG path's own coordinates.
type P = (f64, f64);

/// One step of a parsed SVG path, in absolute coordinates. Quadratic curves and arcs are
/// already cubics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Seg {
    Move(P),
    Line(P),
    Cubic(P, P, P),
    Close,
}

/// Draws the display list with its origin (the root's left edge, on its baseline) at `origin`,
/// one em being `size` pixels. An item without a color of its own is drawn in `color`.
pub(crate) fn draw(items: &[Item<'_>], origin: Pt, size: f32, color: Color) -> Vec<Primitive> {
    let frame = Frame {
        ox: f64::from(origin.x),
        oy: f64::from(origin.y),
        size: f64::from(size),
    };
    let mut out = Vec::new();
    for item in items {
        let primitive = match item {
            Item::Glyph {
                x,
                y,
                scale,
                glyph,
                mirrored,
            } => glyph_primitive(&frame, (*x, *y), *scale, glyph, *mirrored, color),
            Item::Rule {
                x,
                y,
                width,
                height,
                color: own,
            } => rule_primitive(&frame, (*x, *y, *width, *height), pick(*own, color)),
            Item::Path {
                x,
                y,
                width,
                height,
                path,
                mirrored,
            } => path_primitive(&frame, (*x, *y, *width, *height), path, *mirrored, color),
        };
        out.extend(primitive);
    }
    out
}

/// The item's own color if it names one that parses, else the default.
fn pick(own: Option<&str>, default: Color) -> Color {
    own.and_then(parse_color).unwrap_or(default)
}

/// Where the formula sits on the scene and how big an em is, in pixels.
struct Frame {
    ox: f64,
    oy: f64,
    size: f64,
}

impl Frame {
    /// A point given in em of the root font, to scene pixels.
    fn pt(&self, x: f64, y: f64) -> Pt {
        Pt::new(
            (self.ox + x * self.size) as f32,
            (self.oy + y * self.size) as f32,
        )
    }

    fn finite(&self) -> bool {
        self.ox.is_finite() && self.oy.is_finite() && self.size.is_finite() && self.size > 0.0
    }
}

fn fill(segments: Vec<Segment>, color: Color, clip: Option<Rect>) -> Option<Primitive> {
    let drawn = segments
        .iter()
        .any(|s| !matches!(s, Segment::Move(_) | Segment::Close));
    drawn.then_some(Primitive::Path {
        segments,
        fill: Some(color),
        stroke: None,
        clip,
    })
}

fn rule_primitive(
    frame: &Frame,
    (x, y, width, height): (f64, f64, f64, f64),
    color: Color,
) -> Option<Primitive> {
    let sane = [x, y, width, height].iter().all(|v| v.is_finite());
    if !frame.finite() || !sane || width <= 0.0 || height <= 0.0 {
        return None;
    }
    let segments = vec![
        Segment::Move(frame.pt(x, y)),
        Segment::Line(frame.pt(x + width, y)),
        Segment::Line(frame.pt(x + width, y + height)),
        Segment::Line(frame.pt(x, y + height)),
        Segment::Close,
    ];
    fill(segments, color, None)
}

// Glyphs.

/// The font and glyph of a character: its own face, or Main-Regular where the face has no glyph
/// for it (KaTeX falls back the same way, e.g. for accents in `\texttt`).
fn lookup(face: Face, ch: char) -> Option<(&'static fonts::Font, ttf_parser::GlyphId)> {
    [face, Face::MainRegular].into_iter().find_map(|face| {
        let font = fonts::font(face)?;
        Some((font, font.glyph_index(ch)?))
    })
}

fn glyph_primitive(
    frame: &Frame,
    (x, y): (f64, f64),
    scale: f64,
    glyph: &Glyph<'_>,
    mirrored: bool,
    default: Color,
) -> Option<Primitive> {
    if !frame.finite() || !x.is_finite() || !y.is_finite() || !scale.is_finite() {
        return None;
    }
    // Em of the root per font unit, and which way the run goes.
    let dir = if mirrored { -1.0 } else { 1.0 };
    let mut pen = x;
    let mut segments = Vec::new();
    for ch in glyph.text.chars() {
        let Some((font, id)) = lookup(glyph.face, ch) else {
            continue;
        };
        let units = f64::from(font.units_per_em());
        if units <= 0.0 {
            continue;
        }
        let per_unit = scale / units * frame.size;
        let mut outline = Outline {
            segments: Vec::new(),
            at: (0.0, 0.0),
            // Font units to pixels: y flips because the scene's y grows down.
            map: |ux: f32, uy: f32| {
                Pt::new(
                    (frame.ox + pen * frame.size + dir * f64::from(ux) * per_unit) as f32,
                    (frame.oy + y * frame.size - f64::from(uy) * per_unit) as f32,
                )
            },
        };
        let _ = font.outline_glyph(id, &mut outline);
        segments.append(&mut outline.segments);
        let advance = font.glyph_hor_advance(id).map_or(0.0, f64::from);
        pen += dir * advance / units * scale;
    }
    fill(segments, pick(glyph.color, default), None)
}

/// Collects an outline as scene segments, quadratics raised to cubics.
struct Outline<F> {
    segments: Vec<Segment>,
    at: (f32, f32),
    map: F,
}

impl<F: Fn(f32, f32) -> Pt> OutlineBuilder for Outline<F> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.at = (x, y);
        self.segments.push(Segment::Move((self.map)(x, y)));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.at = (x, y);
        self.segments.push(Segment::Line((self.map)(x, y)));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x0, y0) = self.at;
        let third = 2.0 / 3.0;
        self.segments.push(Segment::Cubic {
            c1: (self.map)(x0 + third * (x1 - x0), y0 + third * (y1 - y0)),
            c2: (self.map)(x + third * (x1 - x), y + third * (y1 - y)),
            to: (self.map)(x, y),
        });
        self.at = (x, y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.at = (x, y);
        self.segments.push(Segment::Cubic {
            c1: (self.map)(x1, y1),
            c2: (self.map)(x2, y2),
            to: (self.map)(x, y),
        });
    }

    fn close(&mut self) {
        self.segments.push(Segment::Close);
    }
}

// SVG paths.

fn path_primitive(
    frame: &Frame,
    (x, y, width, height): (f64, f64, f64, f64),
    path: &Path<'_>,
    mirrored: bool,
    default: Color,
) -> Option<Primitive> {
    let [min_x, min_y, vw, vh] = path.view_box;
    let sane = [x, y, width, height, min_x, min_y, vw, vh]
        .iter()
        .all(|v| v.is_finite());
    if !frame.finite() || !sane || width <= 0.0 || height <= 0.0 || vw <= 0.0 || vh <= 0.0 {
        return None;
    }
    // View box units to em: a scale on each axis and where the box's corner lands.
    let (sx, sy, dx, crops) = match path.fit {
        Fit::Stretch => (width / vw, height / vh, 0.0, false),
        Fit::Crop(align) => {
            // `slice`: the larger scale, so the view box covers the box, and `YMin`.
            let s = (width / vw).max(height / vh);
            let spare = width - vw * s;
            let dx = match align {
                Align::Min => 0.0,
                Align::Mid => spare / 2.0,
                Align::Max => spare,
            };
            let slack = 1e-9 * (width + height);
            (s, s, dx, vw * s > width + slack || vh * s > height + slack)
        }
    };
    let map = |(px, py): P| {
        let ex = x + dx + (px - min_x) * sx;
        let ey = y + (py - min_y) * sy;
        // Mirrored about the box's vertical center line.
        let ex = if mirrored { 2.0 * x + width - ex } else { ex };
        frame.pt(ex, ey)
    };
    let segments = parse_svg_path(&path.data)
        .into_iter()
        .map(|seg| match seg {
            Seg::Move(p) => Segment::Move(map(p)),
            Seg::Line(p) => Segment::Line(map(p)),
            Seg::Cubic(c1, c2, to) => Segment::Cubic {
                c1: map(c1),
                c2: map(c2),
                to: map(to),
            },
            Seg::Close => Segment::Close,
        })
        .collect();
    let clip = crops.then(|| {
        let at = frame.pt(x, y);
        Rect::new(
            at.x,
            at.y,
            (width * frame.size) as f32,
            (height * frame.size) as f32,
        )
    });
    fill(segments, pick(path.color, default), clip)
}

/// Reads numbers, flags, and commands from SVG path data.
struct Scan<'s> {
    bytes: &'s [u8],
    at: usize,
}

impl Scan<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn skip_separators(&mut self) {
        while matches!(
            self.peek(),
            Some(b' ' | b'\t' | b'\n' | b'\r' | b'\x0c' | b',')
        ) {
            self.at += 1;
        }
    }

    fn digits(&mut self) -> usize {
        let start = self.at;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.at += 1;
        }
        self.at - start
    }

    /// A number as SVG writes it: `-.5`, `1.`, `2e-3`; `1.5.5` is two numbers.
    fn number(&mut self) -> Option<f64> {
        self.skip_separators();
        let start = self.at;
        if matches!(self.peek(), Some(b'+' | b'-')) {
            self.at += 1;
        }
        let mut count = self.digits();
        if self.peek() == Some(b'.') {
            self.at += 1;
            count += self.digits();
        }
        if count == 0 {
            self.at = start;
            return None;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            let mark = self.at;
            self.at += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.at += 1;
            }
            if self.digits() == 0 {
                self.at = mark;
            }
        }
        let text = std::str::from_utf8(self.bytes.get(start..self.at)?).ok()?;
        text.parse::<f64>().ok().filter(|v| v.is_finite())
    }

    /// An arc flag: one `0` or `1`, which needs no separator after it.
    fn flag(&mut self) -> Option<bool> {
        self.skip_separators();
        let flag = match self.peek()? {
            b'0' => false,
            b'1' => true,
            _ => return None,
        };
        self.at += 1;
        Some(flag)
    }

    fn pair(&mut self) -> Option<P> {
        Some((self.number()?, self.number()?))
    }
}

/// What the previous command left for a smooth curve (`S`, `T`) to reflect.
#[derive(Clone, Copy)]
enum Ctrl {
    None,
    Cubic(P),
    Quad(P),
}

/// Parses SVG path data into absolute segments. All of `M L H V C S Q T A Z` are read, absolute
/// and relative; quadratics and arcs come out as cubics. Data that goes wrong ends the path
/// where it did, keeping what was read.
pub(crate) fn parse_svg_path(data: &str) -> Vec<Seg> {
    let mut scan = Scan {
        bytes: data.as_bytes(),
        at: 0,
    };
    let mut out = Vec::new();
    let mut cur: P = (0.0, 0.0);
    let mut start: P = (0.0, 0.0);
    let mut open = false;
    let mut ctrl = Ctrl::None;
    let mut cmd: Option<u8> = None;
    loop {
        scan.skip_separators();
        let Some(c) = scan.peek() else { break };
        if c.is_ascii_alphabetic() {
            scan.at += 1;
            cmd = Some(c);
        }
        // Numbers with no command repeat the last one; after `Z` or at the start there is none.
        let Some(c) = cmd else { break };
        let rel = c.is_ascii_lowercase();
        let upper = c.to_ascii_uppercase();
        if out.is_empty() && upper != b'M' {
            break;
        }
        let base = if rel { cur } else { (0.0, 0.0) };
        let abs = |(x, y): P| (base.0 + x, base.1 + y);
        // Each arm gives the new current point and the control point it leaves, or ends the path
        // when its arguments are missing.
        let step = (|| -> Option<(P, Ctrl)> {
            Some(match upper {
                b'M' => {
                    let to = abs(scan.pair()?);
                    out.push(Seg::Move(to));
                    start = to;
                    open = true;
                    // More pairs after `M` are `L`.
                    cmd = Some(if rel { b'l' } else { b'L' });
                    (to, Ctrl::None)
                }
                b'Z' => {
                    out.push(Seg::Close);
                    open = false;
                    cmd = None;
                    (start, Ctrl::None)
                }
                b'L' | b'H' | b'V' => {
                    let to = match upper {
                        b'L' => abs(scan.pair()?),
                        b'H' => (base.0 + scan.number()?, cur.1),
                        _ => (cur.0, base.1 + scan.number()?),
                    };
                    reopen(&mut out, &mut open, cur);
                    out.push(Seg::Line(to));
                    (to, Ctrl::None)
                }
                b'C' | b'S' => {
                    let c1 = if upper == b'C' {
                        abs(scan.pair()?)
                    } else if let Ctrl::Cubic(prev) = ctrl {
                        (2.0 * cur.0 - prev.0, 2.0 * cur.1 - prev.1)
                    } else {
                        cur
                    };
                    let c2 = abs(scan.pair()?);
                    let to = abs(scan.pair()?);
                    reopen(&mut out, &mut open, cur);
                    out.push(Seg::Cubic(c1, c2, to));
                    (to, Ctrl::Cubic(c2))
                }
                b'Q' | b'T' => {
                    let q = if upper == b'Q' {
                        abs(scan.pair()?)
                    } else if let Ctrl::Quad(prev) = ctrl {
                        (2.0 * cur.0 - prev.0, 2.0 * cur.1 - prev.1)
                    } else {
                        cur
                    };
                    let to = abs(scan.pair()?);
                    reopen(&mut out, &mut open, cur);
                    out.push(raise(cur, q, to));
                    (to, Ctrl::Quad(q))
                }
                b'A' => {
                    let (rx, ry) = scan.pair()?;
                    let turn = scan.number()?;
                    let large = scan.flag()?;
                    let sweep = scan.flag()?;
                    let to = abs(scan.pair()?);
                    reopen(&mut out, &mut open, cur);
                    arc(&mut out, cur, (rx, ry), turn, (large, sweep), to);
                    (to, Ctrl::None)
                }
                _ => return None,
            })
        })();
        let Some((to, next)) = step else { break };
        cur = to;
        ctrl = next;
    }
    out
}

/// A drawing command after `Z` continues from the current point, in a new subpath.
fn reopen(out: &mut Vec<Seg>, open: &mut bool, at: P) {
    if !*open {
        out.push(Seg::Move(at));
        *open = true;
    }
}

/// The cubic with the same curve as the quadratic from `from` by `ctrl` to `to`.
fn raise(from: P, ctrl: P, to: P) -> Seg {
    let third = 2.0 / 3.0;
    Seg::Cubic(
        (
            from.0 + third * (ctrl.0 - from.0),
            from.1 + third * (ctrl.1 - from.1),
        ),
        (
            to.0 + third * (ctrl.0 - to.0),
            to.1 + third * (ctrl.1 - to.1),
        ),
        to,
    )
}

/// An elliptical arc from `from` to `to` as cubics, one per quarter turn at most (SVG's
/// implementation notes, F.6.5 and F.6.6). `turn` is the ellipse's rotation in degrees.
fn arc(out: &mut Vec<Seg>, from: P, (rx, ry): P, turn: f64, (large, sweep): (bool, bool), to: P) {
    if from == to {
        return;
    }
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 {
        out.push(Seg::Line(to));
        return;
    }
    let (sin, cos) = turn.to_radians().sin_cos();
    let (hx, hy) = ((from.0 - to.0) / 2.0, (from.1 - to.1) / 2.0);
    let (x1, y1) = (cos * hx + sin * hy, -sin * hx + cos * hy);
    // Radii too small to reach `to` grow until they just do.
    let spread = (x1 / rx).powi(2) + (y1 / ry).powi(2);
    if spread > 1.0 {
        let grow = spread.sqrt();
        rx *= grow;
        ry *= grow;
    }
    let numerator = (rx * ry).powi(2) - (rx * y1).powi(2) - (ry * x1).powi(2);
    let denominator = (rx * y1).powi(2) + (ry * x1).powi(2);
    let mut coef = if denominator > 0.0 {
        (numerator / denominator).max(0.0).sqrt()
    } else {
        0.0
    };
    if large == sweep {
        coef = -coef;
    }
    let (cx1, cy1) = (coef * rx * y1 / ry, -coef * ry * x1 / rx);
    let center = (
        cos * cx1 - sin * cy1 + (from.0 + to.0) / 2.0,
        sin * cx1 + cos * cy1 + (from.1 + to.1) / 2.0,
    );
    let start = ((y1 - cy1) / ry).atan2((x1 - cx1) / rx);
    let end = ((-y1 - cy1) / ry).atan2((-x1 - cx1) / rx);
    let mut sweep_angle = end - start;
    let tau = std::f64::consts::TAU;
    if !sweep && sweep_angle > 0.0 {
        sweep_angle -= tau;
    } else if sweep && sweep_angle < 0.0 {
        sweep_angle += tau;
    }
    let parts = (sweep_angle.abs() / std::f64::consts::FRAC_PI_2 - 1e-9)
        .ceil()
        .max(1.0);
    let delta = sweep_angle / parts;
    let k = 4.0 / 3.0 * (delta / 4.0).tan();
    let point = |a: f64| {
        let (s, c) = a.sin_cos();
        (
            center.0 + cos * rx * c - sin * ry * s,
            center.1 + sin * rx * c + cos * ry * s,
        )
    };
    let tangent = |a: f64| {
        let (s, c) = a.sin_cos();
        (-cos * rx * s - sin * ry * c, -sin * rx * s + cos * ry * c)
    };
    let steps = parts as u32;
    for i in 0..steps {
        let a1 = start + f64::from(i) * delta;
        let a2 = a1 + delta;
        let (p1, d1) = (point(a1), tangent(a1));
        let (p2, d2) = (point(a2), tangent(a2));
        let end = if i + 1 == steps { to } else { p2 };
        out.push(Seg::Cubic(
            (p1.0 + k * d1.0, p1.1 + k * d1.1),
            (p2.0 - k * d2.0, p2.1 - k * d2.1),
            end,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{
        boxes::Bx,
        layout::layout,
        parser::{Settings, parse_with},
    };

    const BLACK: Color = Color::rgb(0);
    const ORIGIN: Pt = Pt::new(10.0, 50.0);
    const SIZE: f32 = 100.0;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn near(a: P, b: P) -> bool {
        close(a.0, b.0) && close(a.1, b.1)
    }

    // The SVG path parser.

    #[test]
    fn absolute_lines_and_close() {
        assert_eq!(
            parse_svg_path("M1 2 L3,4 H5 V6 Z"),
            vec![
                Seg::Move((1.0, 2.0)),
                Seg::Line((3.0, 4.0)),
                Seg::Line((5.0, 4.0)),
                Seg::Line((5.0, 6.0)),
                Seg::Close
            ]
        );
    }

    #[test]
    fn relative_commands_start_at_the_current_point() {
        let segs = parse_svg_path("m1 1 l2 3 h-1 v2 c1 1 2 2 3 3 z m1 1 l1 0");
        assert_eq!(
            segs,
            vec![
                Seg::Move((1.0, 1.0)),
                Seg::Line((3.0, 4.0)),
                Seg::Line((2.0, 4.0)),
                Seg::Line((2.0, 6.0)),
                Seg::Cubic((3.0, 7.0), (4.0, 8.0), (5.0, 9.0)),
                Seg::Close,
                // `z` goes back to (1, 1), where the next `m` is relative to.
                Seg::Move((2.0, 2.0)),
                Seg::Line((3.0, 2.0)),
            ]
        );
    }

    #[test]
    fn extra_pairs_after_move_are_lines_and_numbers_repeat() {
        assert_eq!(
            parse_svg_path("M0 0 1 1 l1 0 1 0"),
            vec![
                Seg::Move((0.0, 0.0)),
                Seg::Line((1.0, 1.0)),
                Seg::Line((2.0, 1.0)),
                Seg::Line((3.0, 1.0)),
            ]
        );
    }

    #[test]
    fn numbers_may_run_together() {
        // KaTeX writes `-.667.7` for (-0.667, 0.7) and `c-4-3.333` with no separators.
        assert_eq!(
            parse_svg_path("M0 0c-.667.7-2 1.5-4-3.333-1 1e1"),
            vec![
                Seg::Move((0.0, 0.0)),
                Seg::Cubic((-0.667, 0.7), (-2.0, 1.5), (-4.0, -3.333)),
            ]
        );
        assert_eq!(
            parse_svg_path("M0 0l1e1 2E-1"),
            vec![Seg::Move((0.0, 0.0)), Seg::Line((10.0, 0.2))]
        );
    }

    #[test]
    fn smooth_curves_reflect_the_last_control_point() {
        let segs = parse_svg_path("M0 0C1 2 3 2 4 0S7 -2 8 0");
        assert_eq!(
            segs.get(2),
            Some(&Seg::Cubic((5.0, -2.0), (7.0, -2.0), (8.0, 0.0)))
        );
        // With no curve before it `S` has its first control point at the current point.
        let lone = parse_svg_path("M0 0S1 1 2 0");
        assert_eq!(
            lone.get(1),
            Some(&Seg::Cubic((0.0, 0.0), (1.0, 1.0), (2.0, 0.0)))
        );
    }

    #[test]
    fn quadratics_are_raised_to_cubics() {
        let segs = parse_svg_path("M0 0Q3 3 6 0T12 0");
        let Some(Seg::Cubic(c1, c2, to)) = segs.get(1).copied() else {
            unreachable!("a cubic: {segs:?}")
        };
        assert!(near(c1, (2.0, 2.0)) && near(c2, (4.0, 2.0)) && near(to, (6.0, 0.0)));
        // `T` reflects (3, 3) about (6, 0) to (9, -3).
        let Some(Seg::Cubic(c1, c2, to)) = segs.get(2).copied() else {
            unreachable!("a cubic: {segs:?}")
        };
        assert!(near(c1, (8.0, -2.0)) && near(c2, (10.0, -2.0)) && near(to, (12.0, 0.0)));
        // The raised curve passes through the quadratic's midpoint (3, 1.5).
        let Some(Seg::Cubic(a, b, c)) = segs.get(1).copied() else {
            unreachable!("a cubic")
        };
        let mid = (
            (0.0 + 3.0 * a.0 + 3.0 * b.0 + c.0) / 8.0,
            (0.0 + 3.0 * a.1 + 3.0 * b.1 + c.1) / 8.0,
        );
        assert!(near(mid, (3.0, 1.5)));
    }

    /// The point of a cubic at `t`.
    fn at(from: P, seg: Seg, t: f64) -> P {
        let Seg::Cubic(a, b, c) = seg else {
            unreachable!("a cubic")
        };
        let u = 1.0 - t;
        let f = |p: fn(P) -> f64| {
            u.powi(3) * p(from) + 3.0 * u * u * t * p(a) + 3.0 * u * t * t * p(b) + t.powi(3) * p(c)
        };
        (f(|p| p.0), f(|p| p.1))
    }

    #[test]
    fn arcs_become_cubics_on_the_ellipse() {
        // A half circle of radius 5 about (5, 0), over the top on screen (sweep 1 is clockwise).
        let segs = parse_svg_path("M0 0A5 5 0 0 1 10 0");
        assert_eq!(segs.len(), 3, "{segs:?}");
        let mut from = (0.0, 0.0);
        for seg in &segs[1..] {
            for step in 0..=8 {
                let p = at(from, *seg, f64::from(step) / 8.0);
                let r = ((p.0 - 5.0).powi(2) + p.1.powi(2)).sqrt();
                assert!((r - 5.0).abs() < 0.01, "{p:?} is {r} from the center");
                assert!(p.1 <= 1e-9, "{p:?} is under the chord");
            }
            let Seg::Cubic(_, _, to) = seg else {
                unreachable!("cubic")
            };
            from = *to;
        }
        assert_eq!(from, (10.0, 0.0));
        // The other sweep goes under, and relative arcs with packed flags are read.
        let under = parse_svg_path("m0 0a5 5 0 0010 0");
        let Some(first) = under.get(1).copied() else {
            unreachable!("a cubic: {under:?}")
        };
        assert!(at((0.0, 0.0), first, 0.5).1 > 1.0);
    }

    #[test]
    fn arcs_with_large_flag_and_small_radii() {
        // Radii too small to reach grow to a half circle; the large flag then makes no difference.
        let segs = parse_svg_path("M0 0a1 1 0 1 1 10 0");
        let Some(Seg::Cubic(_, _, end)) = segs.last().copied() else {
            unreachable!("a cubic: {segs:?}")
        };
        assert_eq!(end, (10.0, 0.0));
        // A quarter circle of radius 5 from (0, 5) to (5, 0), the long way is three quarters.
        let short = parse_svg_path("M0 5A5 5 0 0 1 5 0");
        let long = parse_svg_path("M0 5A5 5 0 1 1 5 0");
        assert_eq!((short.len(), long.len()), (2, 4));
        // A zero radius is a line.
        assert_eq!(
            parse_svg_path("M0 0A0 5 0 0 1 3 4"),
            vec![Seg::Move((0.0, 0.0)), Seg::Line((3.0, 4.0))]
        );
    }

    #[test]
    fn bad_data_keeps_what_was_read_and_does_not_panic() {
        assert_eq!(parse_svg_path(""), Vec::new());
        assert_eq!(parse_svg_path("L1 1"), Vec::new());
        assert_eq!(parse_svg_path("1 2"), Vec::new());
        assert_eq!(
            parse_svg_path("M0 0L1 1L2"),
            vec![Seg::Move((0.0, 0.0)), Seg::Line((1.0, 1.0))]
        );
        assert_eq!(parse_svg_path("M0 0 X 1 1"), vec![Seg::Move((0.0, 0.0))]);
        assert_eq!(parse_svg_path("M0 0 A1 1 0 2 0 1 1").len(), 1);
        assert_eq!(parse_svg_path("M1e999 0").len(), 0);
        // Numbers after `Z` need a command.
        assert_eq!(
            parse_svg_path("M0 0Z 1 1"),
            vec![Seg::Move((0.0, 0.0)), Seg::Close]
        );
        // Drawing after `Z` starts a new subpath at the start point.
        assert_eq!(
            parse_svg_path("M2 2h1zL5 5"),
            vec![
                Seg::Move((2.0, 2.0)),
                Seg::Line((3.0, 2.0)),
                Seg::Close,
                Seg::Move((2.0, 2.0)),
                Seg::Line((5.0, 5.0)),
            ]
        );
    }

    // Drawing.

    /// The extent of a path's drawn shape: all points on lines and sampled along cubics.
    fn bounds(segments: &[Segment]) -> Option<(f64, f64, f64, f64)> {
        let mut b: Option<(f64, f64, f64, f64)> = None;
        let mut add = |p: P| {
            b = Some(match b {
                None => (p.0, p.1, p.0, p.1),
                Some((x0, y0, x1, y1)) => (x0.min(p.0), y0.min(p.1), x1.max(p.0), y1.max(p.1)),
            });
        };
        let to_p = |p: Pt| (f64::from(p.x), f64::from(p.y));
        let mut cur: P = (0.0, 0.0);
        for seg in segments {
            match *seg {
                Segment::Move(p) | Segment::Line(p) => {
                    cur = to_p(p);
                    add(cur);
                }
                Segment::Cubic { c1, c2, to } => {
                    let cubic = Seg::Cubic(to_p(c1), to_p(c2), to_p(to));
                    for step in 0..=16 {
                        add(at(cur, cubic, f64::from(step) / 16.0));
                    }
                    cur = to_p(to);
                }
                Segment::Arc { to, .. } => {
                    cur = to_p(to);
                    add(cur);
                }
                Segment::Close => {}
            }
        }
        b
    }

    fn laid_out(src: &str) -> (f64, f64, f64, Vec<Primitive>) {
        let display = false;
        let nodes = parse_with(
            src,
            Settings {
                display_mode: display,
            },
        )
        .expect("parses");
        let bx: Bx<'_> = layout(&nodes, display).expect("lays out");
        let items = bx.items();
        let drawn = draw(&items, ORIGIN, SIZE, BLACK);
        (bx.width.0, bx.height.0, bx.depth.0, drawn)
    }

    /// Every path lies in the formula's box (or its clip does), to five hundredths of an em. A
    /// glyph's ink can pass its TeX height (the `1` of `\frac12` does), and the radical sign of
    /// `\sqrt2` reaches 0.04em above the box layout gives it, as KaTeX's svg does.
    fn check_inside(src: &str) -> Vec<Primitive> {
        let (w, h, d, drawn) = laid_out(src);
        assert!(!drawn.is_empty(), "{src}: nothing drawn");
        let slack = 0.05 * f64::from(SIZE);
        let left = f64::from(ORIGIN.x) - slack;
        let right = f64::from(ORIGIN.x) + w * f64::from(SIZE) + slack;
        let top = f64::from(ORIGIN.y) - h * f64::from(SIZE) - slack;
        let bottom = f64::from(ORIGIN.y) + d * f64::from(SIZE) + slack;
        for primitive in &drawn {
            let Primitive::Path {
                segments,
                fill,
                stroke,
                clip,
            } = primitive
            else {
                unreachable!("only paths: {primitive:?}")
            };
            assert_eq!((*fill, *stroke), (Some(BLACK), None), "{src}");
            let shape = match clip {
                // A clipped path shows only its clip rectangle.
                Some(r) => (
                    f64::from(r.x),
                    f64::from(r.y),
                    f64::from(r.x + r.w),
                    f64::from(r.y + r.h),
                ),
                None => bounds(segments).expect("a shape"),
            };
            assert!(
                shape.0 >= left && shape.1 >= top && shape.2 <= right && shape.3 <= bottom,
                "{src}: {shape:?} outside ({left}, {top}, {right}, {bottom})"
            );
        }
        drawn
    }

    #[test]
    fn a_letter_is_one_path_inside_its_box() {
        let drawn = check_inside("x");
        assert_eq!(drawn.len(), 1);
    }

    #[test]
    fn formulas_draw_inside_their_boxes() {
        for src in [
            "\\frac12",
            "\\sqrt2",
            "\\sqrt{x+1}",
            "\\overbrace{a+b}",
            "x^2_i",
            "\\sqrt[3]{\\frac{a}{b}}",
            "\\underbrace{abc}_n",
            "\\xrightarrow{abc}",
            "\\text{ab}\\texttt{\\~{a}}",
        ] {
            check_inside(src);
        }
    }

    #[test]
    fn a_fraction_has_a_bar_and_a_radical_has_a_path() {
        let (_, _, _, frac) = laid_out("\\frac12");
        // Numerator, denominator, and the bar.
        assert_eq!(frac.len(), 3);
        let (_, _, _, root) = laid_out("\\sqrt2");
        assert_eq!(root.len(), 2);
    }

    #[test]
    fn wide_paths_are_clipped_to_their_box() {
        let drawn = check_inside("\\overbrace{a+b}");
        let clipped = drawn
            .iter()
            .filter(|p| matches!(p, Primitive::Path { clip: Some(_), .. }))
            .count();
        assert!(clipped >= 1, "{drawn:?}");
        // The radical's bar is 400000 units wide and cropped; a glyph is never clipped.
        let (_, _, _, x) = laid_out("x");
        assert!(matches!(
            x.first(),
            Some(Primitive::Path { clip: None, .. })
        ));
    }

    #[test]
    fn a_glyph_outline_matches_the_fonts_box() {
        let (_, _, _, drawn) = laid_out("x");
        let Some(Primitive::Path { segments, .. }) = drawn.first() else {
            unreachable!("a path: {drawn:?}")
        };
        let (x0, y0, x1, y1) = bounds(segments).expect("a shape");
        let font = fonts::font(Face::MathItalic).expect("font");
        let id = font.glyph_index('x').expect("glyph");
        let rect = font.glyph_bounding_box(id).expect("bbox");
        let k = f64::from(SIZE) / f64::from(font.units_per_em());
        let want = (
            f64::from(ORIGIN.x) + f64::from(rect.x_min) * k,
            f64::from(ORIGIN.y) - f64::from(rect.y_max) * k,
            f64::from(ORIGIN.x) + f64::from(rect.x_max) * k,
            f64::from(ORIGIN.y) - f64::from(rect.y_min) * k,
        );
        let tolerance = 2.0 * k;
        for (got, want) in [(x0, want.0), (y0, want.1), (x1, want.2), (y1, want.3)] {
            assert!((got - want).abs() < tolerance, "{got} vs {want}");
        }
    }

    #[test]
    fn rules_are_filled_rectangles() {
        let items = [Item::Rule {
            x: 1.0,
            y: -0.5,
            width: 2.0,
            height: 0.25,
            color: Some("#ff0000"),
        }];
        let drawn = draw(&items, Pt::new(0.0, 0.0), 10.0, BLACK);
        assert_eq!(
            drawn,
            vec![Primitive::Path {
                segments: vec![
                    Segment::Move(Pt::new(10.0, -5.0)),
                    Segment::Line(Pt::new(30.0, -5.0)),
                    Segment::Line(Pt::new(30.0, -2.5)),
                    Segment::Line(Pt::new(10.0, -2.5)),
                    Segment::Close,
                ],
                fill: Some(Color::rgb(0xff0000)),
                stroke: None,
                clip: None,
            }]
        );
        // Empty or unreadable rules draw nothing, and a color that does not parse is the default.
        let none = [
            Item::Rule {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 1.0,
                color: None,
            },
            Item::Rule {
                x: f64::NAN,
                y: 0.0,
                width: 1.0,
                height: 1.0,
                color: None,
            },
        ];
        assert!(draw(&none, Pt::new(0.0, 0.0), 10.0, BLACK).is_empty());
        assert!(draw(&items, Pt::new(0.0, 0.0), f32::NAN, BLACK).is_empty());
        let odd = [Item::Rule {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
            color: Some("nonsense"),
        }];
        let Some(Primitive::Path { fill, .. }) =
            draw(&odd, Pt::new(0.0, 0.0), 10.0, Color::rgb(0x123456)).pop()
        else {
            unreachable!("a path")
        };
        assert_eq!(fill, Some(Color::rgb(0x123456)));
    }

    fn view_path<'a>(fit: Fit, view_box: [f64; 4]) -> Path<'a> {
        Path {
            data: "M0 0H100V100H0z".into(),
            view_box,
            fit,
            color: None,
        }
    }

    fn path_item(path: Path<'static>, mirrored: bool) -> Item<'static> {
        Item::Path {
            x: 1.0,
            y: 2.0,
            width: 4.0,
            height: 1.0,
            path,
            mirrored,
        }
    }

    fn only(items: &[Item<'_>]) -> (Vec<Segment>, Option<Rect>) {
        let Some(Primitive::Path { segments, clip, .. }) =
            draw(items, Pt::new(0.0, 0.0), 1.0, BLACK).pop()
        else {
            unreachable!("a path")
        };
        (segments, clip)
    }

    #[test]
    fn views_map_per_fit() {
        // Stretched: the 100 x 100 square fills the 4 x 1 box, with no clip.
        let stretch = path_item(view_path(Fit::Stretch, [0.0, 0.0, 100.0, 100.0]), false);
        let (segs, clip) = only(&[stretch]);
        assert_eq!(bounds(&segs), Some((1.0, 2.0, 5.0, 3.0)), "{segs:?}");
        assert_eq!(clip, None);
        // Cropped, 400 x 100 view box on a 4 x 1 box: scale 0.01, shows all of the view box.
        // Here the path covers 1 em of it at the left, right, or middle of a wider box.
        let wide = [0.0, 0.0, 400.0, 100.0];
        let left = only(&[path_item(view_path(Fit::Crop(Align::Min), wide), false)]);
        assert_eq!(bounds(&left.0), Some((1.0, 2.0, 2.0, 3.0)));
        assert_eq!(left.1, None);
        // A narrower view box than the box is cropped in y: scale is the larger, 4 / 100.
        let tall = [0.0, 0.0, 100.0, 100.0];
        let mid = only(&[path_item(view_path(Fit::Crop(Align::Mid), tall), false)]);
        assert_eq!(bounds(&mid.0), Some((1.0, 2.0, 5.0, 6.0)));
        assert_eq!(mid.1, Some(Rect::new(1.0, 2.0, 4.0, 1.0)));
        // A view box much wider than the box is cropped in x and aligned by `Align`.
        let huge = [0.0, 0.0, 800.0, 100.0];
        let min = only(&[path_item(view_path(Fit::Crop(Align::Min), huge), false)]);
        assert_eq!(min.1, Some(Rect::new(1.0, 2.0, 4.0, 1.0)));
        assert_eq!(bounds(&min.0), Some((1.0, 2.0, 2.0, 3.0)));
        let max = only(&[path_item(view_path(Fit::Crop(Align::Max), huge), false)]);
        assert_eq!(bounds(&max.0), Some((-3.0, 2.0, -2.0, 3.0)));
    }

    #[test]
    fn mirrored_paths_flip_inside_their_box() {
        let wide = [0.0, 0.0, 400.0, 100.0];
        let flipped = only(&[path_item(view_path(Fit::Crop(Align::Min), wide), true)]);
        // The 1 em shape was at the left of the box (1 to 2); it is now at the right (4 to 5).
        assert_eq!(bounds(&flipped.0), Some((4.0, 2.0, 5.0, 3.0)));
    }

    #[test]
    fn bad_view_boxes_draw_nothing() {
        for view_box in [
            [0.0, 0.0, 0.0, 100.0],
            [0.0, 0.0, 100.0, -1.0],
            [f64::NAN, 0.0, 100.0, 100.0],
        ] {
            let item = path_item(view_path(Fit::Stretch, view_box), false);
            assert!(draw(&[item], Pt::new(0.0, 0.0), 1.0, BLACK).is_empty());
        }
    }

    #[test]
    fn mirrored_glyphs_run_leftward() {
        let nodes = parse_with("x", Settings::default()).expect("parses");
        let items = layout(&nodes, false).expect("lays out").items();
        let [
            Item::Glyph {
                x, y, scale, glyph, ..
            },
        ] = items.as_slice()
        else {
            unreachable!("a glyph: {items:?}")
        };
        let item = |mirrored| Item::Glyph {
            x: *x + 1.0,
            y: *y,
            scale: *scale,
            glyph: glyph.clone(),
            mirrored,
        };
        let straight = only(&[item(false)]).0;
        let flipped = only(&[item(true)]).0;
        let (a, _, b, _) = bounds(&straight).expect("a shape");
        let (c, _, d, _) = bounds(&flipped).expect("a shape");
        // Reflected about x = 1 (em) = 1 pixel at size 1: [a, b] becomes [2 - b, 2 - a].
        assert!(((2.0 - b) - c).abs() < 1e-4 && ((2.0 - a) - d).abs() < 1e-4);
    }

    #[test]
    fn a_missing_glyph_falls_back_to_main_regular() {
        // A character Math-Italic has no glyph for and Main-Regular does.
        let math = fonts::font(Face::MathItalic).expect("font");
        let main = fonts::font(Face::MainRegular).expect("font");
        let missing = ('!'..='~')
            .find(|c| math.glyph_index(*c).is_none() && main.glyph_index(*c).is_some())
            .expect("such a character");
        let typewriter = Glyph {
            text: missing.to_string().into(),
            face: Face::MathItalic,
            italic: super::super::options::Em(0.0),
            skew: super::super::options::Em(0.0),
            color: None,
        };
        let drawn = draw(
            &[Item::Glyph {
                x: 0.0,
                y: 0.0,
                scale: 1.0,
                glyph: typewriter,
                mirrored: false,
            }],
            Pt::new(0.0, 0.0),
            100.0,
            BLACK,
        );
        assert_eq!(drawn.len(), 1);
        // No face has a glyph for a private-use character: nothing is drawn.
        let nothing = Glyph {
            text: "\u{10ffff}".into(),
            face: Face::MainRegular,
            italic: super::super::options::Em(0.0),
            skew: super::super::options::Em(0.0),
            color: None,
        };
        let drawn = draw(
            &[Item::Glyph {
                x: 0.0,
                y: 0.0,
                scale: 1.0,
                glyph: nothing,
                mirrored: false,
            }],
            Pt::new(0.0, 0.0),
            100.0,
            BLACK,
        );
        assert!(drawn.is_empty());
    }
}
