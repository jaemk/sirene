//! Timing diagram to scene (TIMING-4).

use super::git::ink;
use super::shapes::{self, BoxF};
use super::style::parse_color;
use super::{
    Color, Element, Owner, Primitive, Pt, Scene, Segment as Seg, Stroke, TextMetrics, TextStyle,
    Theme,
};
use crate::timing::{SignalKind, Timing, Value};

const MARGIN: f32 = 16.0;
const PAD: f32 = 8.0;
/// Width of the time axis.
const CHART_W: f32 = 720.0;
/// Height of one robust level, and of a concise band.
const LEVEL: f32 = 20.0;
const BAND: f32 = 26.0;
const ANALOG_H: f32 = 60.0;
const LABEL_H: f32 = 22.0;
/// How far a concise band's ends slant.
const SLANT: f32 = 4.0;
const HEAD: f32 = 7.0;

struct Painter<'a, M> {
    metrics: &'a M,
    theme: &'a Theme,
    elements: Vec<Element>,
    right: f32,
}

impl<M: TextMetrics> Painter<'_, M> {
    fn width(&self, text: &str, size: f32) -> f32 {
        text.lines()
            .map(|l| self.metrics.width(l, size))
            .fold(0.0, f32::max)
    }

    fn path(&mut self, segments: Vec<Seg>, fill: Option<Color>, stroke: Option<Stroke>) {
        self.elements.push(Element {
            owner: Owner::Diagram,
            primitive: Primitive::Path {
                segments,
                fill,
                stroke,
            },
        });
    }

    fn line(&mut self, from: Pt, to: Pt, stroke: Stroke) {
        self.path(vec![Seg::Move(from), Seg::Line(to)], None, Some(stroke));
    }

    /// One line of text centered on `(x, y)`.
    fn text_c(&mut self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        let w = self.width(text, size);
        self.right = self.right.max(x + w / 2.0);
        self.elements.push(Element {
            owner: Owner::Diagram,
            primitive: Primitive::Text {
                text: text.to_owned(),
                center: Pt::new(x, y),
                size,
                color,
                style: TextStyle::default(),
            },
        });
    }

    /// Text starting at `x`, its first line centered on `y`.
    fn text_at(&mut self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        for (i, l) in text.lines().enumerate() {
            let w = self.width(l, size);
            self.text_c(
                l,
                x + w / 2.0,
                y + i as f32 * self.theme.line_height,
                size,
                color,
            );
        }
    }

    /// An arrowhead with its tip at `tip`, pointing along `dir`.
    fn head(&mut self, tip: Pt, dir: (f32, f32), color: Color) {
        let len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt().max(1e-3);
        let (ux, uy) = (dir.0 / len, dir.1 / len);
        let (bx, by) = (tip.x - ux * HEAD, tip.y - uy * HEAD);
        let (px, py) = (-uy * HEAD / 2.0, ux * HEAD / 2.0);
        let pts = [(tip.x, tip.y), (bx + px, by + py), (bx - px, by - py)];
        self.path(shapes::polygon(&pts), Some(color), None);
    }
}

/// What a row holds, top to bottom: notes above, the signal, its constraints, notes below.
struct Row {
    top: f32,
    /// The signal's band.
    y: f32,
    h: f32,
    constraint_y: Option<f32>,
}

/// Draws `timing` (TIMING-4).
pub fn timing(timing: &Timing, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let mut p = Painter {
        metrics,
        theme,
        elements: Vec::new(),
        right: 0.0,
    };
    let fs = theme.font_size;
    let small = fs * 0.85;
    let lh = theme.line_height;
    let title = timing
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_h = if title.is_some() { lh + 16.0 } else { 0.0 };
    let Some(range) = timing.range() else {
        return finish(p, title, MARGIN * 2.0 + title_h);
    };
    let (t0, t1) = range;
    // A column of signal names, then one of robust state labels.
    let mut name_w: f32 = 60.0;
    let mut state_w: f32 = 0.0;
    for s in &timing.signals {
        name_w = name_w.max(p.width(&s.label, fs) + 2.0 * PAD);
        if s.kind == SignalKind::Robust {
            for l in s.levels() {
                state_w = state_w.max(p.width(s.state_label(&l), small) + PAD);
            }
        }
    }
    let x0 = MARGIN + name_w + state_w;
    let x1 = x0 + CHART_W;
    let span = (t1 - t0).max(1e-9);
    let x = |t: f64| x0 + ((t - t0) / span * f64::from(CHART_W)) as f32;
    p.right = x1;

    // Rows.
    let mut y = MARGIN + title_h;
    let highlight_lane = timing.highlights.iter().any(|h| !h.label.is_empty());
    let highlight_y = y;
    if highlight_lane {
        y += LABEL_H;
    }
    let rows_top = y;
    let mut rows = Vec::new();
    for s in &timing.signals {
        let top = y;
        for n in s.notes.iter().filter(|n| n.above) {
            y += n.text.lines().count() as f32 * lh + PAD;
        }
        y += PAD;
        let h = match s.kind {
            SignalKind::Robust => s.levels().len().max(2) as f32 * LEVEL,
            SignalKind::Binary | SignalKind::Clock => 2.0 * LEVEL,
            SignalKind::Concise | SignalKind::Rectangle => BAND,
            SignalKind::Analog => ANALOG_H,
        };
        // Change notes sit above the band.
        let change_notes = s.changes.iter().any(|c| c.note.is_some());
        if change_notes {
            y += lh;
        }
        let band_y = y;
        y += h + PAD;
        let constraint_y = timing
            .constraints
            .iter()
            .any(|c| c.signal.as_deref() == Some(s.id.as_str()))
            .then(|| {
                let cy = y + lh;
                y += lh + PAD;
                cy
            });
        for n in s.notes.iter().filter(|n| !n.above) {
            y += n.text.lines().count() as f32 * lh + PAD;
        }
        rows.push(Row {
            top,
            y: band_y,
            h,
            constraint_y,
        });
    }
    let rows_bottom = y;
    let axis_y = rows_bottom + PAD;
    let mut height = axis_y;
    if !timing.hide_axis {
        height += LABEL_H + PAD;
    }
    let free_constraints: Vec<_> = timing
        .constraints
        .iter()
        .filter(|c| c.signal.is_none())
        .collect();
    let free_y = height + lh;
    if !free_constraints.is_empty() {
        height += free_constraints.len() as f32 * (lh + PAD) + PAD;
    }
    height += MARGIN;

    // Highlights.
    for h in &timing.highlights {
        let (a, b) = (x(h.from.min(h.to)), x(h.from.max(h.to)));
        let base = h
            .color
            .as_deref()
            .and_then(parse_color)
            .unwrap_or(theme.cluster_fill);
        let fill = base.with_alpha(0x80).over(theme.background);
        let r = BoxF {
            x: a,
            y: rows_top,
            w: (b - a).max(1.0),
            h: rows_bottom - rows_top,
        };
        p.path(shapes::polygon(&corners(r)), Some(fill), None);
        if !h.label.is_empty() {
            p.text_c(
                &h.label,
                (a + b) / 2.0,
                highlight_y + LABEL_H / 2.0,
                small,
                theme.text,
            );
        }
    }
    // Grid lines at the ticks.
    let ticks = timing.ticks(range);
    let grid = Stroke {
        width: 1.0,
        color: theme.edge.with_alpha(0x30).over(theme.background),
        dash: Some((3.0, 3.0)),
    };
    for &(t, _) in &ticks {
        p.line(Pt::new(x(t), rows_top), Pt::new(x(t), rows_bottom), grid);
    }
    let rule = Stroke {
        width: 1.0,
        color: theme.edge.with_alpha(0x50).over(theme.background),
        dash: None,
    };
    let wave = Stroke {
        width: 2.0,
        color: theme.edge,
        dash: None,
    };

    for (s, row) in timing.signals.iter().zip(&rows) {
        // Name, and a rule between signals.
        p.text_at(&s.label, MARGIN, row.y + small / 2.0, fs, theme.text);
        if row.top > rows_top {
            p.line(Pt::new(MARGIN, row.top), Pt::new(x1, row.top), rule);
        }
        let mut ny = row.top + PAD + lh / 2.0;
        for n in s.notes.iter().filter(|n| n.above) {
            note(&mut p, &n.text, x0, ny - lh / 2.0);
            ny += n.text.lines().count() as f32 * lh + PAD;
        }
        let segs = timing.segments(s, range);
        match s.kind {
            SignalKind::Robust | SignalKind::Binary | SignalKind::Clock | SignalKind::Analog => {
                let levels = s.levels();
                let level_y = |v: &str| -> Option<f32> {
                    if s.kind == SignalKind::Analog {
                        let (lo, hi) = s.analog_scale();
                        let f = ((v.parse::<f64>().ok()? - lo) / (hi - lo)).clamp(0.0, 1.0);
                        return Some(row.y + row.h - f as f32 * row.h);
                    }
                    let n = levels.len().max(2) as f32;
                    let h = row.h / n;
                    s.level(&levels, v).map(|i| row.y + h * (i as f32 + 0.5))
                };
                if s.kind == SignalKind::Robust {
                    for l in &levels {
                        if let Some(ly) = level_y(l) {
                            let label = s.state_label(l);
                            let w = p.width(label, small);
                            p.text_at(label, x0 - PAD - w, ly, small, theme.text);
                        }
                    }
                }
                let mut path: Vec<Seg> = Vec::new();
                let mut last: Option<Pt> = None;
                for g in &segs {
                    let (a, b) = (x(g.start), x(g.end));
                    match &g.value {
                        Value::State(v) => {
                            let Some(ly) = level_y(v) else { continue };
                            match last {
                                Some(l)
                                    if s.kind == SignalKind::Analog || (l.x - a).abs() < 0.5 =>
                                {
                                    path.push(Seg::Line(Pt::new(a, ly)));
                                }
                                _ => path.push(Seg::Move(Pt::new(a, ly))),
                            }
                            if s.kind == SignalKind::Analog {
                                // Values meet in straight lines; the last one holds.
                                last = Some(Pt::new(a, ly));
                                continue;
                            }
                            path.push(Seg::Line(Pt::new(b, ly)));
                            last = Some(Pt::new(b, ly));
                        }
                        Value::Hidden => last = None,
                        Value::Undefined => {
                            let r = BoxF {
                                x: a,
                                y: row.y,
                                w: (b - a).max(1.0),
                                h: row.h,
                            };
                            let fill = theme.edge.with_alpha(0x30).over(theme.background);
                            p.path(shapes::polygon(&corners(r)), Some(fill), None);
                            last = None;
                        }
                    }
                }
                if s.kind == SignalKind::Analog
                    && let Some(l) = last
                {
                    path.push(Seg::Line(Pt::new(x(t1), l.y)));
                }
                if !path.is_empty() {
                    p.path(path, None, Some(wave));
                }
            }
            SignalKind::Concise | SignalKind::Rectangle => {
                if s.kind == SignalKind::Rectangle {
                    let r = BoxF {
                        x: x0,
                        y: row.y - 2.0,
                        w: CHART_W,
                        h: row.h + 4.0,
                    };
                    p.path(
                        shapes::polygon(&corners(r)),
                        None,
                        Some(Stroke {
                            width: 1.0,
                            color: theme.node_stroke,
                            dash: None,
                        }),
                    );
                }
                for g in &segs {
                    let (a, b) = (x(g.start), x(g.end));
                    let d = SLANT.min((b - a) / 2.0);
                    let (top, mid, bot) = (row.y, row.y + row.h / 2.0, row.y + row.h);
                    let hex = [
                        (a, mid),
                        (a + d, top),
                        (b - d, top),
                        (b, mid),
                        (b - d, bot),
                        (a + d, bot),
                    ];
                    let stroke = Stroke {
                        width: 1.5,
                        color: theme.node_stroke,
                        dash: None,
                    };
                    match &g.value {
                        Value::Hidden => {}
                        Value::Undefined => {
                            let fill = theme.edge.with_alpha(0x30).over(theme.background);
                            p.path(shapes::polygon(&hex), Some(fill), Some(stroke));
                        }
                        Value::State(v) => {
                            let fill = g.color.and_then(parse_color).unwrap_or(theme.node_fill);
                            p.path(shapes::polygon(&hex), Some(fill), Some(stroke));
                            let label = s.state_label(v);
                            if p.width(label, small) + 2.0 * d + 2.0 <= b - a {
                                p.text_c(label, (a + b) / 2.0, mid, small, ink(fill));
                            }
                        }
                    }
                }
            }
        }
        // Change notes, above the band at their moment.
        for c in &s.changes {
            if let Some(n) = &c.note {
                p.text_at(n, x(c.at) + 3.0, row.y - lh / 2.0, small, theme.text);
            }
        }
        if let Some(cy) = row.constraint_y {
            for c in timing
                .constraints
                .iter()
                .filter(|c| c.signal.as_deref() == Some(s.id.as_str()))
            {
                constraint(&mut p, x(c.from), x(c.to), cy, &c.label, small);
            }
        }
        let mut ny = row.y + row.h + PAD + row.constraint_y.map_or(0.0, |_| lh + PAD);
        for n in s.notes.iter().filter(|n| !n.above) {
            note(&mut p, &n.text, x0, ny);
            ny += n.text.lines().count() as f32 * lh + PAD;
        }
    }

    // Messages.
    let center = |id: &str| -> Option<f32> {
        let i = timing.signals.iter().position(|s| s.id == id)?;
        Some(rows[i].y + rows[i].h / 2.0)
    };
    let arrow = Stroke {
        width: 1.5,
        color: theme.edge,
        dash: None,
    };
    for m in &timing.messages {
        let (Some(fy), Some(ty)) = (center(&m.from), center(&m.to)) else {
            continue;
        };
        let (a, b) = (Pt::new(x(m.from_at), fy), Pt::new(x(m.to_at), ty));
        let dir = (b.x - a.x, b.y - a.y);
        let len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt();
        if len < 1.0 {
            continue;
        }
        let end = Pt::new(
            b.x - dir.0 / len * HEAD * 0.8,
            b.y - dir.1 / len * HEAD * 0.8,
        );
        p.line(a, end, arrow);
        p.head(b, dir, theme.edge);
        if !m.label.is_empty() {
            let mx = (a.x + b.x) / 2.0;
            let my = (a.y + b.y) / 2.0;
            p.text_at(&m.label, mx + 4.0, my - lh / 2.0, small, theme.text);
        }
    }

    // The axis.
    if !timing.hide_axis {
        p.line(Pt::new(x0, axis_y), Pt::new(x1, axis_y), wave);
        let mut last_label = f32::NEG_INFINITY;
        for (t, label) in &ticks {
            let tx = x(*t);
            p.line(Pt::new(tx, axis_y), Pt::new(tx, axis_y + 5.0), wave);
            let w = p.width(label, small);
            if tx - w / 2.0 < last_label + 4.0 {
                continue;
            }
            last_label = tx + w / 2.0;
            p.text_c(label, tx, axis_y + 5.0 + LABEL_H / 2.0, small, theme.text);
        }
    }
    for (i, c) in free_constraints.iter().enumerate() {
        let cy = free_y + i as f32 * (lh + PAD);
        constraint(&mut p, x(c.from), x(c.to), cy, &c.label, small);
    }
    finish(p, title, height)
}

/// A two-headed span from `a` to `b` at `y`, its label above.
fn constraint<M: TextMetrics>(
    p: &mut Painter<'_, M>,
    a: f32,
    b: f32,
    y: f32,
    label: &str,
    size: f32,
) {
    let color = p.theme.edge;
    let stroke = Stroke {
        width: 1.0,
        color,
        dash: None,
    };
    let (a, b) = (a.min(b), a.max(b));
    if b - a > 2.0 * HEAD {
        p.line(Pt::new(a + HEAD, y), Pt::new(b - HEAD, y), stroke);
        p.head(Pt::new(a, y), (-1.0, 0.0), color);
        p.head(Pt::new(b, y), (1.0, 0.0), color);
    } else {
        p.line(Pt::new(a, y), Pt::new(b.max(a + 1.0), y), stroke);
    }
    if !label.is_empty() {
        p.text_c(
            label,
            (a + b) / 2.0,
            y - p.theme.line_height / 2.0,
            size,
            p.theme.text,
        );
    }
}

/// A note box with its top left at `(x, y)`.
fn note<M: TextMetrics>(p: &mut Painter<'_, M>, text: &str, x: f32, y: f32) {
    let size = p.theme.font_size * 0.85;
    let lh = p.theme.line_height;
    let w = p.width(text, size) + 2.0 * PAD;
    let h = text.lines().count().max(1) as f32 * lh;
    let r = BoxF { x, y, w, h };
    p.path(
        shapes::polygon(&corners(r)),
        Some(p.theme.cluster_fill),
        Some(Stroke {
            width: 1.0,
            color: p.theme.cluster_stroke,
            dash: None,
        }),
    );
    p.text_at(text, x + PAD, y + lh / 2.0, size, p.theme.text);
}

fn corners(b: BoxF) -> [(f32, f32); 4] {
    [
        (b.x, b.y),
        (b.x + b.w, b.y),
        (b.x + b.w, b.y + b.h),
        (b.x, b.y + b.h),
    ]
}

/// The scene, as wide as what was drawn, with the title centered above.
fn finish<M: TextMetrics>(p: Painter<'_, M>, title: Option<&str>, height: f32) -> Scene {
    let size = p.theme.font_size * 1.2;
    let width = (p.right + MARGIN).max(title.map_or(0.0, |t| p.width(t, size)) + 2.0 * MARGIN);
    let mut elements = p.elements;
    if let Some(t) = title {
        elements.insert(
            0,
            Element {
                owner: Owner::Diagram,
                primitive: Primitive::Text {
                    text: t.to_owned(),
                    center: Pt::new(width / 2.0, MARGIN + p.theme.line_height / 2.0),
                    size,
                    color: p.theme.text,
                    style: TextStyle::default(),
                },
            },
        );
    }
    Scene {
        width,
        height,
        background: p.theme.background,
        elements,
    }
}
