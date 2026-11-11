//! Cynefin diagram to scene (MCYN-3): four tinted quadrants split by dashed wavy boundaries,
//! the cliff between Clear and Chaotic, the confusion ellipse in the middle, item badges, and
//! curved transition arrows.

use super::shapes;
use super::{
    Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle, Theme,
};
use crate::cynefin::{Cynefin, Domain};

/// The least size of the area holding the domains, as Mermaid's default.
const WIDTH: f32 = 800.0;
const HEIGHT: f32 = 600.0;
/// Space around the domains; a title sits in the top padding.
const PADDING: f32 = 40.0;
const DOMAIN_FONT: f32 = 16.0;
const ITEM_FONT: f32 = 12.0;
const SMALL_FONT: f32 = 11.0;
const TITLE_FONT: f32 = 18.0;
const BADGE_H: f32 = 26.0;
const BADGE_GAP: f32 = 4.0;
const BADGE_PAD: f32 = 10.0;
/// Space between a domain's text block and its quadrant's edges or the confusion ellipse.
const MARGIN: f32 = 12.0;
/// Waviness of the boundaries.
const AMPLITUDE: f32 = 8.0;
const ARROW: f32 = 10.0;

/// The color each domain is tinted with.
fn hue(d: Domain) -> Color {
    Color::rgb(match d {
        Domain::Complex => 0x4caf50,
        Domain::Complicated => 0x2196f3,
        Domain::Chaotic => 0xff5722,
        Domain::Clear => 0xffc107,
        Domain::Confusion => 0x9c27b0,
    })
}

fn is_dark(c: Color) -> bool {
    u32::from(c.r) * 299 + u32::from(c.g) * 587 + u32::from(c.b) * 114 < 128_000
}

/// One line of a domain's heading: its name, or a description line.
struct Heading {
    text: String,
    size: f32,
    style: TextStyle,
    height: f32,
}

/// A domain's text: its heading lines, then its item badges.
struct Block {
    headings: Vec<Heading>,
    /// Badge texts and widths; the last is dashed when it counts items not shown.
    badges: Vec<(String, f32)>,
    overflow: bool,
    w: f32,
    h: f32,
}

impl Block {
    fn new(chart: &Cynefin, d: Domain, metrics: &impl TextMetrics) -> Self {
        let bold = TextStyle {
            bold: true,
            italic: false,
        };
        let italic = TextStyle {
            bold: false,
            italic: true,
        };
        let mut headings = vec![Heading {
            text: d.title().to_owned(),
            size: DOMAIN_FONT,
            style: bold,
            height: 22.0,
        }];
        if chart.descriptions {
            // The bundled italic font has no arrow glyph.
            let model = d.model().join(" -> ");
            for text in [model, d.practice().to_owned()] {
                if !text.is_empty() {
                    headings.push(Heading {
                        text,
                        size: SMALL_FONT,
                        style: italic,
                        height: 15.0,
                    });
                }
            }
        }
        let (items, more) = chart.shown(d);
        let mut badges: Vec<String> = items.to_vec();
        if more > 0 {
            badges.push(format!("+{more} more"));
        }
        let badges: Vec<(String, f32)> = badges
            .into_iter()
            .map(|t| {
                let w = metrics.width(&t, ITEM_FONT) + 2.0 * BADGE_PAD;
                (t, w)
            })
            .collect();
        let mut w = badges.iter().map(|b| b.1).fold(0.0f32, f32::max);
        for h in &headings {
            w = w.max(metrics.styled_width(&h.text, h.size, h.style));
        }
        let mut h: f32 = headings.iter().map(|h| h.height).sum();
        if !badges.is_empty() {
            h += 8.0 + badges.len() as f32 * (BADGE_H + BADGE_GAP) - BADGE_GAP;
        }
        Block {
            headings,
            badges,
            overflow: more > 0,
            w,
            h,
        }
    }
}

/// The layout: the domain area's size, the confusion ellipse's radii, and each domain's
/// center.
struct Frame {
    w: f32,
    h: f32,
    rx: f32,
    ry: f32,
}

impl Frame {
    fn center(&self, d: Domain) -> Pt {
        let (qx, qy) = (self.w / 4.0, self.h / 4.0);
        match d {
            Domain::Complex => Pt::new(qx, qy),
            Domain::Complicated => Pt::new(3.0 * qx, qy),
            Domain::Chaotic => Pt::new(qx, 3.0 * qy),
            Domain::Clear => Pt::new(3.0 * qx, 3.0 * qy),
            Domain::Confusion => Pt::new(2.0 * qx, 2.0 * qy),
        }
    }

    /// Whether `b`, centered in quadrant `d`, fits it clear of the confusion ellipse.
    fn fits(&self, d: Domain, b: &Block) -> (bool, bool) {
        let c = self.center(d);
        let fit_w = b.w + 2.0 * MARGIN <= self.w / 2.0;
        let fit_h = b.h + 2.0 * MARGIN <= self.h / 2.0;
        // The block's corner nearest the middle must be outside the ellipse.
        let (mx, my) = (self.w / 2.0, self.h / 2.0);
        let px = if c.x < mx {
            c.x + b.w / 2.0
        } else {
            c.x - b.w / 2.0
        };
        let py = if c.y < my {
            c.y + b.h / 2.0
        } else {
            c.y - b.h / 2.0
        };
        let ex = (px - mx) / (self.rx + MARGIN);
        let ey = (py - my) / (self.ry + MARGIN);
        let clear = ex * ex + ey * ey >= 1.0;
        (fit_w && clear, fit_h && clear)
    }
}

/// The size of the domain area and the ellipse that holds every block.
fn frame(blocks: &[Block; 5], title_w: f32) -> Frame {
    let conf = &blocks[4];
    let (mut w, mut h) = (WIDTH.max(title_w), HEIGHT);
    for _ in 0..80 {
        let f = Frame {
            w,
            h,
            rx: (0.15 * w).max(conf.w / 2.0 * std::f32::consts::SQRT_2 + 8.0),
            ry: (0.15 * h).max(conf.h / 2.0 * std::f32::consts::SQRT_2 + 8.0),
        };
        let mut grow = (false, false);
        let mut ok = f.rx * 2.0 + 2.0 * MARGIN <= w && f.ry * 2.0 + 2.0 * MARGIN <= h;
        if !ok {
            grow = (f.rx * 2.0 + 2.0 * MARGIN > w, f.ry * 2.0 + 2.0 * MARGIN > h);
        }
        for (d, b) in Domain::QUADRANTS.iter().zip(blocks) {
            let (fw, fh) = f.fits(*d, b);
            if !fw || !fh {
                ok = false;
                grow.0 |= !fw;
                grow.1 |= !fh;
            }
        }
        if ok {
            return f;
        }
        if grow.0 {
            w *= 1.08;
        }
        if grow.1 {
            h *= 1.08;
        }
    }
    Frame {
        w,
        h,
        rx: 0.15 * w,
        ry: 0.15 * h,
    }
}

struct Draw {
    out: Vec<Element>,
}

impl Draw {
    fn push(&mut self, primitive: Primitive) {
        self.out.push(Element {
            owner: Owner::Diagram,
            primitive,
        });
    }

    fn path(&mut self, segments: Vec<Segment>, fill: Option<Color>, stroke: Option<Stroke>) {
        self.push(Primitive::Path {
            segments,
            fill,
            stroke,
        });
    }

    fn text(&mut self, text: &str, center: Pt, size: f32, color: Color, style: TextStyle) {
        if text.is_empty() {
            return;
        }
        self.push(Primitive::Text {
            text: text.to_owned(),
            center,
            size,
            color,
            style,
        });
    }
}

fn stroke(width: f32, color: Color, dash: Option<(f32, f32)>) -> Option<Stroke> {
    Some(Stroke { width, color, dash })
}

/// A wavy line from `a` to `b` in `n` cubic arcs bulging to alternate sides.
fn wave(a: Pt, b: Pt, n: usize) -> Vec<Segment> {
    let (dx, dy) = ((b.x - a.x) / n as f32, (b.y - a.y) / n as f32);
    let len = (dx * dx + dy * dy).sqrt().max(1e-3);
    let (nx, ny) = (-dy / len * AMPLITUDE, dx / len * AMPLITUDE);
    let mut out = vec![Segment::Move(a)];
    for i in 0..n {
        let s = if i % 2 == 0 { 1.0 } else { -1.0 };
        let p0 = Pt::new(a.x + dx * i as f32, a.y + dy * i as f32);
        let p1 = Pt::new(p0.x + dx, p0.y + dy);
        out.push(Segment::Cubic {
            c1: Pt::new(p0.x + dx / 3.0 + s * nx, p0.y + dy / 3.0 + s * ny),
            c2: Pt::new(p1.x - dx / 3.0 + s * nx, p1.y - dy / 3.0 + s * ny),
            to: p1,
        });
    }
    out
}

/// The point at `t` on the quadratic curve `p0`, `c`, `p2`, and the direction there.
fn quad(p0: Pt, c: Pt, p2: Pt, t: f32) -> (Pt, Pt) {
    let u = 1.0 - t;
    let p = Pt::new(
        u * u * p0.x + 2.0 * u * t * c.x + t * t * p2.x,
        u * u * p0.y + 2.0 * u * t * c.y + t * t * p2.y,
    );
    let d = Pt::new(
        2.0 * u * (c.x - p0.x) + 2.0 * t * (p2.x - c.x),
        2.0 * u * (c.y - p0.y) + 2.0 * t * (p2.y - c.y),
    );
    (p, d)
}

/// Whether `p` lies in the box of half size `half` around `c`.
fn inside(p: Pt, c: Pt, half: (f32, f32)) -> bool {
    (p.x - c.x).abs() <= half.0 && (p.y - c.y).abs() <= half.1
}

/// Draws `chart` (MCYN-3).
pub fn cynefin(chart: &Cynefin, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let dark = is_dark(theme.background);
    let blocks: [Block; 5] = Domain::ALL.map(|d| Block::new(chart, d, metrics));
    let title = chart
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let bold = TextStyle {
        bold: true,
        italic: false,
    };
    let title_w = title.map_or(0.0, |t| metrics.styled_width(t, TITLE_FONT, bold));
    let f = frame(&blocks, title_w);
    let (width, height) = (f.w + 2.0 * PADDING, f.h + 2.0 * PADDING);
    let at = |p: Pt| Pt::new(p.x + PADDING, p.y + PADDING);
    let tint = |d: Domain| {
        hue(d)
            .with_alpha(if dark { 0x2a } else { 0x18 })
            .over(theme.background)
    };
    let line = theme.edge;
    let mut dr = Draw { out: Vec::new() };

    // Quadrants.
    let (hw, hh) = (f.w / 2.0, f.h / 2.0);
    for d in Domain::QUADRANTS {
        let c = f.center(d);
        let (x, y) = (at(c).x - hw / 2.0, at(c).y - hh / 2.0);
        let pts = [(x, y), (x + hw, y), (x + hw, y + hh), (x, y + hh)];
        dr.path(shapes::polygon(&pts), Some(tint(d)), None);
    }
    // Boundaries: the fold between ordered and unordered across the top half, the line between
    // the halves, and the cliff under Clear's left edge.
    let mid = at(Pt::new(hw, hh));
    let dashed = stroke(2.0, line, Some((6.0, 3.0)));
    dr.path(wave(at(Pt::new(hw, 0.0)), mid, 4), None, dashed);
    dr.path(
        wave(at(Pt::new(0.0, hh)), at(Pt::new(f.w, hh)), 8),
        None,
        dashed,
    );
    let cliff = Color::rgb(if dark { 0xe06666 } else { 0x8b0000 });
    let bottom = at(Pt::new(hw, f.h));
    let a = f.w * 0.03;
    dr.path(
        vec![
            Segment::Move(mid),
            Segment::Cubic {
                c1: Pt::new(mid.x + a, mid.y + hh * 0.3),
                c2: Pt::new(mid.x - a * 1.5, mid.y + hh * 0.55),
                to: Pt::new(mid.x + a * 0.4, mid.y + hh * 0.75),
            },
            Segment::Cubic {
                c1: Pt::new(mid.x + a * 1.2, mid.y + hh * 0.85),
                c2: Pt::new(mid.x - a * 0.4, mid.y + hh * 0.95),
                to: bottom,
            },
        ],
        None,
        stroke(4.0, cliff, None),
    );
    // Confusion, over the middle.
    dr.path(
        shapes::ellipse(mid, f.rx, f.ry),
        Some(tint(Domain::Confusion)),
        stroke(1.5, line, Some((4.0, 2.0))),
    );

    // Each domain's heading and items, centered on its center.
    for (d, b) in Domain::ALL.iter().zip(&blocks) {
        let c = at(f.center(*d));
        let mut y = c.y - b.h / 2.0;
        for h in &b.headings {
            dr.text(
                &h.text,
                Pt::new(c.x, y + h.height / 2.0),
                h.size,
                theme.text,
                h.style,
            );
            y += h.height;
        }
        y += 8.0;
        let fill = theme.background.with_alpha(0xc8).over(tint(*d));
        for (i, (t, w)) in b.badges.iter().enumerate() {
            let last = b.overflow && i + 1 == b.badges.len();
            let r = shapes::BoxF {
                x: c.x - w / 2.0,
                y,
                w: *w,
                h: BADGE_H,
            };
            let dash = last.then_some((3.0, 2.0));
            dr.path(shapes::rounded(r, 4.0), Some(fill), stroke(1.0, line, dash));
            dr.text(t, r.center(), ITEM_FONT, theme.text, TextStyle::default());
            y += BADGE_H + BADGE_GAP;
        }
    }

    // Transitions: curves between domain centers, bowed to the left of their direction and
    // trimmed to start and end outside the domains' text.
    for tr in &chart.transitions {
        let (p0, p2) = (at(f.center(tr.from)), at(f.center(tr.to)));
        let (dx, dy) = (p2.x - p0.x, p2.y - p0.y);
        let len = (dx * dx + dy * dy).sqrt();
        if len == 0.0 {
            continue;
        }
        let n = Pt::new(-dy / len, dx / len);
        let c = Pt::new(
            (p0.x + p2.x) / 2.0 + n.x * len * 0.15,
            (p0.y + p2.y) / 2.0 + n.y * len * 0.15,
        );
        let half = |d: Domain| {
            let b = &blocks[Domain::ALL.iter().position(|x| *x == d).unwrap_or(0)];
            (b.w / 2.0 + 6.0, b.h / 2.0 + 6.0)
        };
        let (hs, ht) = (half(tr.from), half(tr.to));
        const STEPS: usize = 400;
        let ts = (0..=STEPS).map(|i| i as f32 / STEPS as f32);
        let t0 = ts
            .clone()
            .find(|&t| !inside(quad(p0, c, p2, t).0, p0, hs))
            .unwrap_or(0.0);
        let t1 = ts
            .rev()
            .find(|&t| !inside(quad(p0, c, p2, t).0, p2, ht))
            .unwrap_or(1.0);
        let (t0, t1) = if t0 < t1 { (t0, t1) } else { (0.0, 1.0) };
        let (s, ds) = quad(p0, c, p2, t0);
        let (e, de) = quad(p0, c, p2, t1);
        // The control point of the curve between t0 and t1.
        let k = (t1 - t0) / 2.0;
        let sc = Pt::new(s.x + ds.x * k, s.y + ds.y * k);
        let dl = (de.x * de.x + de.y * de.y).sqrt().max(1e-3);
        let dir = Pt::new(de.x / dl, de.y / dl);
        let base = Pt::new(e.x - dir.x * ARROW * 0.8, e.y - dir.y * ARROW * 0.8);
        dr.path(
            vec![
                Segment::Move(s),
                Segment::Cubic {
                    c1: Pt::new(
                        s.x + 2.0 / 3.0 * (sc.x - s.x),
                        s.y + 2.0 / 3.0 * (sc.y - s.y),
                    ),
                    c2: Pt::new(
                        base.x + 2.0 / 3.0 * (sc.x - base.x),
                        base.y + 2.0 / 3.0 * (sc.y - base.y),
                    ),
                    to: base,
                },
            ],
            None,
            stroke(2.0, line, None),
        );
        let back = Pt::new(e.x - dir.x * ARROW, e.y - dir.y * ARROW);
        let (nx, ny) = (-dir.y * ARROW / 2.0, dir.x * ARROW / 2.0);
        dr.path(
            shapes::polygon(&[
                (e.x, e.y),
                (back.x + nx, back.y + ny),
                (back.x - nx, back.y - ny),
            ]),
            Some(line),
            None,
        );
        if let Some(label) = &tr.label {
            let (m, _) = quad(p0, c, p2, 0.5);
            let lw = metrics.width(label, SMALL_FONT);
            let off = n.x.abs() * lw / 2.0 + n.y.abs() * 8.0 + 4.0;
            let at = Pt::new(m.x + n.x * off, m.y + n.y * off);
            let back = shapes::BoxF {
                x: at.x - lw / 2.0 - 4.0,
                y: at.y - 9.0,
                w: lw + 8.0,
                h: 18.0,
            };
            dr.path(
                shapes::rounded(back, 4.0),
                Some(theme.background.with_alpha(0xd0)),
                None,
            );
            dr.text(label, at, SMALL_FONT, theme.text, TextStyle::default());
        }
    }

    if let Some(t) = title {
        dr.text(
            t,
            Pt::new(width / 2.0, PADDING / 2.0),
            TITLE_FONT,
            theme.text,
            bold,
        );
    }
    Scene {
        width,
        height,
        background: theme.background,
        elements: dr.out,
    }
}
