//! Timing diagram to scene (TIMING-4).

use std::collections::HashMap;

use super::git::ink;
use super::mathtext::{
    Formulas, block_size, draw_lines, sized, tag_runs, title_elements, title_extent,
};
use super::shapes::{self, BoxF};
use super::style::parse_color;
use super::{
    Color, Element, Owner, Primitive, Pt, Scene, Segment as Seg, Stroke, TextMetrics, TextStyle,
    Theme,
};
use crate::timing::{Constraint, Levels, SignalKind, Timing, Value};

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
    /// Whether `<latex>` and `<math>` tags in text are formulas (PlantUML).
    tags: bool,
    /// The formulas of this render, typeset once each.
    formulas: Formulas,
}

impl<M: TextMetrics> Painter<'_, M> {
    /// The size of the block of `text` at `size` when it holds a formula, else `None` (the text
    /// is drawn as it always has been).
    fn block(&self, text: &str, size: f32) -> Option<(f32, f32)> {
        let runs = tag_runs(text, self.tags)?;
        Some(block_size(
            self.metrics,
            &self.formulas,
            &sized(self.theme, size),
            &runs,
        ))
    }

    /// The height of the formula in `text` at `size`, with a pixel above and below; `None` for
    /// plain text.
    fn math_h(&self, text: &str, size: f32) -> Option<f32> {
        self.block(text, size).map(|b| b.1 + 2.0)
    }

    /// The first line's center for text whose first line is centered on `y` when plain: the same
    /// top, and the formula's block centered in its own height.
    fn lead(&self, text: &str, y: f32, size: f32) -> f32 {
        let lh = self.theme.line_height;
        self.math_h(text, size)
            .map_or(y, |h| y - lh / 2.0 + h / 2.0)
    }

    /// As [`Self::lead`] for text whose line sits on a bottom edge: the formula's block rests on
    /// the same edge.
    fn trail(&self, text: &str, y: f32, size: f32) -> f32 {
        let lh = self.theme.line_height;
        self.math_h(text, size)
            .map_or(y, |h| y + lh / 2.0 - h / 2.0)
    }

    /// The tallest formula among `texts` at `size`, and at least `least`.
    fn tallest<'t>(&self, texts: impl Iterator<Item = &'t str>, size: f32, least: f32) -> f32 {
        texts
            .filter_map(|t| self.math_h(t, size))
            .fold(least, f32::max)
    }

    /// The room a note takes in its signal's rows: a line each, or its formula's height.
    fn note_room(&self, text: &str) -> f32 {
        let size = self.theme.font_size * 0.85;
        self.math_h(text, size)
            .unwrap_or_else(|| text.lines().count() as f32 * self.theme.line_height)
    }

    /// The height of a note's box.
    fn note_h(&self, text: &str) -> f32 {
        let size = self.theme.font_size * 0.85;
        self.math_h(text, size)
            .unwrap_or_else(|| text.lines().count().max(1) as f32 * self.theme.line_height)
    }

    fn width(&self, text: &str, size: f32) -> f32 {
        if let Some(b) = self.block(text, size) {
            return b.0;
        }
        text.lines()
            .map(|l| self.metrics.width(l, size))
            .fold(0.0, f32::max)
    }

    /// The formula block of `text` centered on `center`, in the painter's owner; `false` for
    /// plain text, which the caller draws.
    fn math(&mut self, text: &str, center: Pt, size: f32, color: Color) -> bool {
        let Some(runs) = tag_runs(text, self.tags) else {
            return false;
        };
        let theme = sized(self.theme, size);
        let drawn = draw_lines(self.metrics, &self.formulas, &theme, &runs, center, color);
        self.elements
            .extend(drawn.into_iter().map(|primitive| Element {
                owner: Owner::Diagram,
                primitive,
            }));
        true
    }

    fn path(&mut self, segments: Vec<Seg>, fill: Option<Color>, stroke: Option<Stroke>) {
        self.elements.push(Element {
            owner: Owner::Diagram,
            primitive: Primitive::Path {
                clip: None,
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
        if self.math(text, Pt::new(x, y), size, color) {
            return;
        }
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

    /// Text starting at `x`, its first line centered on `y`; a formula's block is centered on `y`.
    fn text_at(&mut self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        if self.block(text, size).is_some() {
            let w = self.width(text, size);
            return self.text_c(text, x + w / 2.0, y, size, color);
        }
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
    /// What the signal's constraint line is given above it: a line, or its tallest formula.
    c_h: f32,
}

/// Draws `timing` (TIMING-4).
pub fn timing(timing: &Timing, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let mut p = Painter {
        metrics,
        theme,
        elements: Vec::new(),
        right: 0.0,
        tags: timing.tags,
        formulas: Formulas::default(),
    };
    let fs = theme.font_size;
    let small = fs * 0.85;
    let lh = theme.line_height;
    let title = timing
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_h = title.map_or(0.0, |t| {
        let h = title_extent(metrics, &p.formulas, theme, t, timing.tags).map_or(lh, |e| e.1);
        h + 16.0
    });
    let Some(range) = timing.range() else {
        return finish(p, title, MARGIN * 2.0 + title_h);
    };
    let (t0, t1) = range;
    // A column of signal names, then one of robust state labels.
    let levels: Vec<Levels> = timing.signals.iter().map(|s| s.levels()).collect();
    let mut name_w: f32 = 60.0;
    let mut state_w: f32 = 0.0;
    for (s, lv) in timing.signals.iter().zip(&levels) {
        name_w = name_w.max(p.width(&s.label, fs) + 2.0 * PAD);
        if s.kind == SignalKind::Robust {
            for l in &lv.names {
                state_w = state_w.max(p.width(lv.label(l), small) + PAD);
            }
        }
    }
    let index: HashMap<&str, usize> = timing
        .signals
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();
    let mut constraints: HashMap<&str, Vec<&Constraint>> = HashMap::new();
    for c in &timing.constraints {
        if let Some(id) = c.signal.as_deref() {
            constraints.entry(id).or_default().push(c);
        }
    }
    let x0 = MARGIN + name_w + state_w;
    let x1 = x0 + CHART_W;
    let span = (t1 - t0).max(1e-9);
    let x = |t: f64| x0 + ((t - t0) / span * f64::from(CHART_W)) as f32;
    p.right = x1;

    // Rows.
    let mut y = MARGIN + title_h;
    // The lane of highlight captions is as tall as the tallest formula in one.
    let lane_h = timing
        .highlights
        .iter()
        .filter_map(|h| p.math_h(&h.label, small))
        .fold(LABEL_H, f32::max);
    let highlight_lane = timing.highlights.iter().any(|h| !h.label.is_empty());
    let highlight_y = y;
    if highlight_lane {
        y += lane_h;
    }
    let rows_top = y;
    // What each signal's formulas ask of its rows: a formula taller than the text grows the
    // level, the band, the name's room, the change-note line, or the constraint line it is in.
    let heights: Vec<(f32, f32, f32, f32)> = timing
        .signals
        .iter()
        .zip(&levels)
        .map(|(s, lv)| {
            let labels = || lv.names.iter().map(|l| lv.label(l));
            let level = if s.kind == SignalKind::Robust {
                p.tallest(labels(), small, LEVEL)
            } else {
                LEVEL
            };
            let band = if matches!(s.kind, SignalKind::Concise | SignalKind::Rectangle) {
                p.tallest(labels(), small, BAND - 2.0) + 2.0
            } else {
                BAND
            };
            let notes = s.changes.iter().filter_map(|c| c.note.as_deref());
            let change_notes = p.tallest(notes, small, lh);
            let name = p.math_h(&s.label, fs).unwrap_or(0.0);
            (level, band, change_notes, name)
        })
        .collect();
    let constraint_h: Vec<f32> = timing
        .signals
        .iter()
        .map(|s| {
            let labels = constraints
                .get(s.id.as_str())
                .into_iter()
                .flatten()
                .map(|c| c.label.as_str());
            p.tallest(labels, small, lh)
        })
        .collect();
    let layout = |extra: &[f32]| -> (Vec<Row>, f32) {
        let mut y = rows_top;
        let mut rows = Vec::new();
        for (i, (s, lv)) in timing.signals.iter().zip(&levels).enumerate() {
            let (level, band, change_notes, name) = heights[i];
            let top = y;
            for n in s.notes.iter().filter(|n| n.above) {
                y += p.note_room(&n.text) + PAD;
            }
            y += PAD;
            let h = match s.kind {
                SignalKind::Robust => lv.len().max(2) as f32 * level,
                SignalKind::Binary | SignalKind::Clock => 2.0 * LEVEL,
                SignalKind::Concise | SignalKind::Rectangle => band,
                SignalKind::Analog => ANALOG_H,
            }
            .max(name);
            // Change notes sit above the band.
            if s.changes.iter().any(|c| c.note.is_some()) {
                y += change_notes;
            }
            let band_y = y;
            y += h + PAD;
            let c_h = constraint_h[i];
            let constraint_y = constraints.contains_key(s.id.as_str()).then(|| {
                let cy = y + c_h;
                y += c_h + PAD;
                cy
            });
            for n in s.notes.iter().filter(|n| !n.above) {
                y += p.note_room(&n.text) + PAD;
            }
            y += extra[i];
            rows.push(Row {
                top,
                y: band_y,
                h,
                constraint_y,
                c_h,
            });
        }
        (rows, y)
    };
    let mut gaps = vec![0.0; timing.signals.len()];
    let (mut rows, mut rows_bottom) = layout(&gaps);
    // A message's label sits in the gap between its two signals, so a formula taller than the
    // gap grows the gap below the upper signal.
    let label_h = |m: &crate::timing::Message| p.math_h(&m.label, small);
    let ends = |m: &crate::timing::Message| -> Option<(usize, usize)> {
        let (&a, &b) = (index.get(m.from.as_str())?, index.get(m.to.as_str())?);
        (a != b).then_some((a.min(b), a.max(b)))
    };
    let mut grown = false;
    for m in &timing.messages {
        if let (Some(h), Some((a, b))) = (label_h(m), ends(m)) {
            let room = rows[b].y - (rows[a].y + rows[a].h);
            if room < h {
                gaps[a] = gaps[a].max(h - room);
                grown = true;
            }
        }
    }
    if grown {
        (rows, rows_bottom) = layout(&gaps);
    }
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
    // Each free constraint's line sits under its label, which is a line tall or as tall as
    // its formula.
    let free_h: Vec<f32> = free_constraints
        .iter()
        .map(|c| p.math_h(&c.label, small).unwrap_or(lh).max(lh))
        .collect();
    let mut free_ys = Vec::new();
    let mut free_at = height;
    for h in &free_h {
        free_ys.push(free_at + h);
        free_at += h + PAD;
    }
    if !free_constraints.is_empty() {
        height = free_at + PAD;
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
                highlight_y + lane_h / 2.0,
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

    for ((s, row), lv) in timing.signals.iter().zip(&rows).zip(&levels) {
        // Name, and a rule between signals.
        let name_y = p.lead(&s.label, row.y + small / 2.0, fs);
        p.text_at(&s.label, MARGIN, name_y, fs, theme.text);
        if row.top > rows_top {
            p.line(Pt::new(MARGIN, row.top), Pt::new(x1, row.top), rule);
        }
        let mut ny = row.top + PAD + lh / 2.0;
        for n in s.notes.iter().filter(|n| n.above) {
            note(&mut p, &n.text, x0, ny - lh / 2.0);
            ny += p.note_room(&n.text) + PAD;
        }
        let segs = timing.segments(s, range);
        match s.kind {
            SignalKind::Robust | SignalKind::Binary | SignalKind::Clock | SignalKind::Analog => {
                let (lo, hi) = s.analog_scale();
                let level_y = |v: &str| -> Option<f32> {
                    if s.kind == SignalKind::Analog {
                        let f = ((v.trim().parse::<f64>().ok()? - lo) / (hi - lo)).clamp(0.0, 1.0);
                        return Some(row.y + row.h - f as f32 * row.h);
                    }
                    let n = lv.len().max(2) as f32;
                    let h = row.h / n;
                    lv.level(v).map(|i| row.y + h * (i as f32 + 0.5))
                };
                if s.kind == SignalKind::Robust {
                    for l in &lv.names {
                        if let Some(ly) = level_y(l) {
                            let label = lv.label(l);
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
                // Each state's label is measured once, however many segments show it.
                let mut widths: HashMap<&str, f32> = HashMap::new();
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
                            let label = lv.label(v);
                            let w = *widths.entry(label).or_insert_with(|| p.width(label, small));
                            if w + 2.0 * d + 2.0 <= b - a {
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
                let ny = p.trail(n, row.y - lh / 2.0, small);
                p.text_at(n, x(c.at) + 3.0, ny, small, theme.text);
            }
        }
        if let Some(cy) = row.constraint_y {
            for c in constraints.get(s.id.as_str()).into_iter().flatten() {
                constraint(&mut p, x(c.from), x(c.to), cy, &c.label, small);
            }
        }
        let mut ny = row.y + row.h + PAD + row.constraint_y.map_or(0.0, |_| row.c_h + PAD);
        for n in s.notes.iter().filter(|n| !n.above) {
            note(&mut p, &n.text, x0, ny);
            ny += p.note_room(&n.text) + PAD;
        }
    }

    // Messages.
    let center = |id: &str| -> Option<f32> {
        let i = *index.get(id)?;
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
            // A formula is centered in the gap between the two signals, which was grown to
            // hold it; plain text hangs above the arrow's middle.
            let in_gap = index
                .get(m.from.as_str())
                .zip(index.get(m.to.as_str()))
                .filter(|(f, t)| f != t)
                .map(|(&f, &t)| {
                    let (a, b) = (f.min(t), f.max(t));
                    (rows[a].y + rows[a].h + rows[b].y) / 2.0
                });
            let ly = match in_gap {
                Some(gap) if p.math_h(&m.label, small).is_some() => gap,
                _ => p.trail(&m.label, my - lh / 2.0, small),
            };
            p.text_at(&m.label, mx + 4.0, ly, small, theme.text);
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
    for (c, &cy) in free_constraints.iter().zip(&free_ys) {
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
        let ly = p.trail(label, y - p.theme.line_height / 2.0, size);
        p.text_c(label, (a + b) / 2.0, ly, size, p.theme.text);
    }
}

/// A note box with its top left at `(x, y)`.
fn note<M: TextMetrics>(p: &mut Painter<'_, M>, text: &str, x: f32, y: f32) {
    let size = p.theme.font_size * 0.85;
    let lh = p.theme.line_height;
    let w = p.width(text, size) + 2.0 * PAD;
    let h = p.note_h(text);
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
    let ty = p.lead(text, y + lh / 2.0, size);
    p.text_at(text, x + PAD, ty, size, p.theme.text);
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
    let (formulas, tags) = (&p.formulas, p.tags);
    let size = p.theme.font_size * 1.2;
    let title_w = title.map_or(0.0, |t| {
        title_extent(p.metrics, formulas, p.theme, t, tags)
            .map_or_else(|| p.width(t, size), |e| e.0)
    });
    let width = (p.right + MARGIN).max(title_w + 2.0 * MARGIN);
    let formula_title = title
        .and_then(|t| title_elements(p.metrics, formulas, p.theme, t, tags, (width / 2.0, MARGIN)));
    let mut elements = p.elements;
    if let Some(drawn) = formula_title {
        elements.splice(0..0, drawn);
    } else if let Some(t) = title {
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
