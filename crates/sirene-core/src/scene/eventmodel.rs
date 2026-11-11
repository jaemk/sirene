//! Event modeling diagram to scene (MEVM-3): swimlanes stacked top to bottom, frame boxes
//! along the timeline, each starting just before the previous one ends, and straight relation
//! arrows between them.

use super::shapes;
use super::{
    Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle, Theme,
};
use crate::eventmodel::{EntityType, EventModel};

const NAME_FONT: f32 = 16.0;
const NAME_LINE: f32 = 20.0;
const DATA_FONT: f32 = 13.0;
const DATA_LINE: f32 = 17.0;
const LABEL_FONT: f32 = 16.0;
const TITLE_FONT: f32 = 18.0;
/// The widest a line of a box's text runs before wrapping.
const TEXT_MAX: f32 = 430.0;
/// A box's text area is clamped to these, then padded by `BOX_PAD` on each side.
const BOX_MIN_W: f32 = 80.0;
const BOX_MAX_W: f32 = 450.0;
const BOX_MIN_H: f32 = 80.0;
const BOX_MAX_H: f32 = 750.0;
const BOX_PAD: f32 = 10.0;
/// How far a box in another lane starts before the previous box ends.
const OVERLAP: f32 = 70.0;
const LANE_MIN_H: f32 = 70.0;
const LANE_PAD: f32 = 15.0;
const LANE_GAP: f32 = 10.0;
/// Where the first box starts, leaving room for the lane labels.
const CONTENT_X: f32 = 250.0;
const PADDING: f32 = 30.0;
const ARROW: (f32, f32) = (10.0, 7.0);

const BOLD: TextStyle = TextStyle {
    bold: true,
    italic: false,
};

/// A frame type's fill and stroke, as Mermaid's defaults.
fn colors(kind: EntityType) -> (Color, Color) {
    let (fill, stroke) = match kind {
        EntityType::Ui => (0xffffff, 0xdbdada),
        EntityType::Processor => (0xedb3f6, 0xb88cbf),
        EntityType::ReadModel => (0xd3f1a2, 0xa3b732),
        EntityType::Command => (0xbcd6fe, 0x679ac3),
        EntityType::Event => (0xffb778, 0xc19a0f),
    };
    (Color::rgb(fill), Color::rgb(stroke))
}

/// Words of `s` in lines no wider than `max`, never breaking a word; leading spaces are kept.
fn wrap(metrics: &impl TextMetrics, s: &str, size: f32, style: TextStyle, max: f32) -> Vec<String> {
    let lead = &s[..s.len() - s.trim_start().len()];
    let mut lines: Vec<String> = Vec::new();
    for word in s.split_whitespace() {
        match lines.last_mut() {
            Some(line) if metrics.styled_width(&format!("{line} {word}"), size, style) <= max => {
                line.push(' ');
                line.push_str(word);
            }
            Some(_) => lines.push(word.to_owned()),
            None => lines.push(format!("{lead}{word}")),
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// A frame's box: its text, size, and place.
pub(crate) struct Placed {
    pub name: Vec<String>,
    pub data: Vec<String>,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Placed {
    fn right(&self) -> f32 {
        self.x + self.w
    }
}

/// A lane's top and height.
pub(crate) struct LaneBox {
    pub label: String,
    pub y: f32,
    pub h: f32,
}

/// The layout of a diagram's lanes and boxes, before padding.
pub(crate) struct Layout {
    pub boxes: Vec<Placed>,
    pub lanes: Vec<LaneBox>,
    pub width: f32,
    pub height: f32,
}

pub(crate) fn layout(chart: &EventModel, metrics: &impl TextMetrics) -> Layout {
    let regular = TextStyle::default();
    let labels: Vec<String> = chart.lanes.iter().map(|l| l.label()).collect();
    let start = labels
        .iter()
        .map(|l| metrics.styled_width(l, LABEL_FONT, BOLD) + 60.0)
        .fold(CONTENT_X, f32::max);
    let mut boxes: Vec<Placed> = Vec::with_capacity(chart.frames.len());
    // Each lane's right edge so far, and its tallest box.
    let mut lane_r: Vec<Option<f32>> = vec![None; chart.lanes.len()];
    let mut lane_h = vec![0.0f32; chart.lanes.len()];
    for (i, f) in chart.frames.iter().enumerate() {
        let name = wrap(metrics, &f.name, NAME_FONT, BOLD, TEXT_MAX);
        let data: Vec<String> = f
            .data
            .iter()
            .flat_map(|l| wrap(metrics, l, DATA_FONT, regular, TEXT_MAX))
            .collect();
        let mut tw = name
            .iter()
            .map(|l| metrics.styled_width(l, NAME_FONT, BOLD))
            .fold(0.0, f32::max);
        tw = data
            .iter()
            .map(|l| metrics.width(l, DATA_FONT))
            .fold(tw, f32::max);
        let mut th = name.len() as f32 * NAME_LINE;
        if !data.is_empty() {
            th += BOX_PAD + data.len() as f32 * DATA_LINE;
        }
        let w = (tw + 2.0 * BOX_PAD).clamp(BOX_MIN_W, BOX_MAX_W) + 2.0 * BOX_PAD;
        let h = (th + 2.0 * BOX_PAD).clamp(BOX_MIN_H, BOX_MAX_H) + 2.0 * BOX_PAD;
        let x = match (
            i.checked_sub(1).map(|p| (&boxes[p], chart.frames[p].lane)),
            lane_r[f.lane],
        ) {
            (None, _) => start,
            (Some((_, pl)), Some(r)) if pl == f.lane => r + BOX_PAD,
            (Some((prev, _)), r) => {
                let x = prev.right() - OVERLAP;
                r.map_or(x, |r| x.max(r + BOX_PAD))
            }
        };
        lane_r[f.lane] = Some(x + w);
        lane_h[f.lane] = lane_h[f.lane].max(h);
        boxes.push(Placed {
            name,
            data,
            x,
            y: 0.0,
            w,
            h,
        });
    }
    let mut lanes = Vec::with_capacity(chart.lanes.len());
    let mut y = 0.0;
    for (label, h) in labels.into_iter().zip(&lane_h) {
        let h = h.max(LANE_MIN_H) + 2.0 * LANE_PAD;
        lanes.push(LaneBox { label, y, h });
        y += h + LANE_GAP;
    }
    for (b, f) in boxes.iter_mut().zip(&chart.frames) {
        b.y = lanes[f.lane].y + LANE_PAD;
    }
    let right = boxes
        .iter()
        .map(|b| b.right() + BOX_PAD)
        .fold(start, f32::max);
    Layout {
        boxes,
        width: right + LANE_PAD,
        height: (y - LANE_GAP).max(0.0),
        lanes,
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
        if text.trim().is_empty() {
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

fn stroke(width: f32, color: Color) -> Option<Stroke> {
    Some(Stroke {
        width,
        color,
        dash: None,
    })
}

/// Draws `chart` (MEVM-3).
pub fn eventmodel(chart: &EventModel, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let l = layout(chart, metrics);
    let title = chart
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_w = title.map_or(0.0, |t| metrics.styled_width(t, TITLE_FONT, BOLD));
    let top = PADDING + if title.is_some() { 30.0 } else { 0.0 };
    let width = l.width.max(title_w) + 2.0 * PADDING;
    let height = l.height + top + PADDING;
    let at = |x: f32, y: f32| Pt::new(x + PADDING, y + top);
    let mut dr = Draw { out: Vec::new() };

    let lane_fill = theme.text.with_alpha(0x08).over(theme.background);
    let lane_stroke = theme.text.with_alpha(0x14).over(theme.background);
    for lane in &l.lanes {
        let p = at(0.0, lane.y);
        let r = shapes::BoxF {
            x: p.x,
            y: p.y,
            w: l.width,
            h: lane.h,
        };
        dr.path(
            shapes::rounded(r, 3.0),
            Some(lane_fill),
            stroke(1.0, lane_stroke),
        );
        let lw = metrics.styled_width(&lane.label, LABEL_FONT, BOLD);
        dr.text(
            &lane.label,
            Pt::new(p.x + 30.0 + lw / 2.0, p.y + 25.0),
            LABEL_FONT,
            theme.text,
            BOLD,
        );
    }

    let ink = Color::rgb(0x222222);
    for (b, f) in l.boxes.iter().zip(&chart.frames) {
        let (fill, line) = colors(f.kind);
        let p = at(b.x, b.y);
        let r = shapes::BoxF {
            x: p.x,
            y: p.y,
            w: b.w,
            h: b.h,
        };
        dr.path(shapes::rounded(r, 3.0), Some(fill), stroke(1.0, line));
        let mut th = b.name.len() as f32 * NAME_LINE;
        if !b.data.is_empty() {
            th += BOX_PAD + b.data.len() as f32 * DATA_LINE;
        }
        let mut y = p.y + (b.h - th).max(0.0) / 2.0;
        let cx = p.x + b.w / 2.0;
        for n in &b.name {
            dr.text(n, Pt::new(cx, y + NAME_LINE / 2.0), NAME_FONT, ink, BOLD);
            y += NAME_LINE;
        }
        if !b.data.is_empty() {
            y += BOX_PAD;
            let dw = b
                .data
                .iter()
                .map(|d| metrics.width(d, DATA_FONT))
                .fold(0.0, f32::max);
            let x0 = cx - dw / 2.0;
            for d in &b.data {
                let w = metrics.width(d, DATA_FONT);
                dr.text(
                    d,
                    Pt::new(x0 + w / 2.0, y + DATA_LINE / 2.0),
                    DATA_FONT,
                    ink,
                    TextStyle::default(),
                );
                y += DATA_LINE;
            }
        }
    }

    for rel in &chart.relations {
        let (s, t) = (&l.boxes[rel.from], &l.boxes[rel.to]);
        let (from, to) = if s.y == t.y {
            // In one lane: from one box's side to the other's, at mid height.
            let (sx, tx) = if t.x >= s.x {
                (s.right(), t.x)
            } else {
                (s.x, t.right())
            };
            (at(sx, s.y + s.h / 2.0), at(tx, t.y + t.h / 2.0))
        } else {
            let sx = s.x + s.w * 2.0 / 3.0;
            let tx = t.x + t.w / 3.0;
            if s.y > t.y {
                (at(sx, s.y), at(tx, t.y + t.h))
            } else {
                (at(sx, s.y + s.h), at(tx, t.y))
            }
        };
        let (dx, dy) = (to.x - from.x, to.y - from.y);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1.0 {
            continue;
        }
        let dir = Pt::new(dx / len, dy / len);
        let back = Pt::new(to.x - dir.x * ARROW.0, to.y - dir.y * ARROW.0);
        dr.path(
            vec![Segment::Move(from), Segment::Line(back)],
            None,
            stroke(1.0, theme.edge),
        );
        let (nx, ny) = (-dir.y * ARROW.1 / 2.0, dir.x * ARROW.1 / 2.0);
        dr.path(
            shapes::polygon(&[
                (to.x, to.y),
                (back.x + nx, back.y + ny),
                (back.x - nx, back.y - ny),
            ]),
            Some(theme.edge),
            None,
        );
    }

    if let Some(t) = title {
        dr.text(
            t,
            Pt::new(width / 2.0, PADDING + 10.0),
            TITLE_FONT,
            theme.text,
            BOLD,
        );
    }
    Scene {
        width,
        height,
        background: theme.background,
        elements: dr.out,
    }
}
