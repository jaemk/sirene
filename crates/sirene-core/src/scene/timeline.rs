//! Timeline to scene (TIMELI-3).

use super::git::{branch_color, ink};
use super::shapes::{self, BoxF};
use super::{
    Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle, Theme,
};
use crate::timeline::{Direction, Text, Timeline};

const MARGIN: f32 = 16.0;
/// Padding inside a box.
const PAD: f32 = 10.0;
/// Widest a box gets before its text wraps.
const MAX_W: f32 = 200.0;
/// Narrowest box.
const MIN_W: f32 = 80.0;
/// Space between boxes.
const GAP: f32 = 12.0;
/// Space between the periods and the arrow, and the arrow and the events.
const AXIS_GAP: f32 = 20.0;
/// Length of the arrowhead.
const HEAD: f32 = 10.0;

/// Wrapped text and the size of the box around it.
struct Block {
    lines: Vec<String>,
    w: f32,
    h: f32,
}

struct Painter<'a, M> {
    metrics: &'a M,
    theme: &'a Theme,
    elements: Vec<Element>,
}

impl<M: TextMetrics> Painter<'_, M> {
    fn width(&self, text: &str) -> f32 {
        self.metrics.width(text, self.theme.font_size)
    }

    /// `text` broken at spaces to fit `max`; a longer word keeps a line of its own.
    fn wrap(&self, text: &Text, max: f32) -> Vec<String> {
        let mut out = Vec::new();
        for line in text {
            let mut cur = String::new();
            for word in line.split_whitespace() {
                let next = if cur.is_empty() {
                    word.to_owned()
                } else {
                    format!("{cur} {word}")
                };
                if !cur.is_empty() && self.width(&next) > max {
                    out.push(std::mem::replace(&mut cur, word.to_owned()));
                } else {
                    cur = next;
                }
            }
            out.push(cur);
        }
        out
    }

    /// `text` wrapped into a box at least `min` wide.
    fn block(&self, text: &Text, min: f32) -> Block {
        let lines = self.wrap(text, MAX_W - 2.0 * PAD);
        let w = lines.iter().map(|l| self.width(l)).fold(0.0, f32::max) + 2.0 * PAD;
        Block {
            h: lines.len() as f32 * self.theme.line_height + 2.0 * PAD,
            w: w.max(min),
            lines,
        }
    }

    fn path(&mut self, segments: Vec<Segment>, fill: Option<Color>, stroke: Option<Stroke>) {
        self.elements.push(Element {
            owner: Owner::Diagram,
            primitive: Primitive::Path {
                segments,
                fill,
                stroke,
            },
        });
    }

    fn text(&mut self, text: &str, center: Pt, size: f32, color: Color) {
        self.elements.push(Element {
            owner: Owner::Diagram,
            primitive: Primitive::Text {
                text: text.to_owned(),
                center,
                size,
                color,
                style: TextStyle::default(),
            },
        });
    }

    /// A filled box with `lines` centered in it.
    fn boxed(&mut self, b: BoxF, lines: &[String], fill: Color, stroke: Option<Color>) {
        let stroke = stroke.map(|color| Stroke {
            width: 1.0,
            color,
            dash: None,
        });
        self.path(shapes::rounded(b, 4.0), Some(fill), stroke);
        let lh = self.theme.line_height;
        let top = b.y + (b.h - lines.len() as f32 * lh) / 2.0;
        for (i, l) in lines.iter().enumerate() {
            let center = Pt::new(b.x + b.w / 2.0, top + (i as f32 + 0.5) * lh);
            self.text(l, center, self.theme.font_size, ink(fill));
        }
    }

    fn dashed(&mut self, from: Pt, to: Pt, color: Color) {
        let stroke = Stroke {
            width: 1.5,
            color,
            dash: Some((4.0, 4.0)),
        };
        self.path(
            vec![Segment::Move(from), Segment::Line(to)],
            None,
            Some(stroke),
        );
    }

    /// A line from `from` to `to` ending in an arrowhead.
    fn arrow(&mut self, from: Pt, to: Pt) {
        let color = self.theme.edge;
        let stroke = Stroke {
            width: 2.0,
            color,
            dash: None,
        };
        let horizontal = from.y == to.y;
        let base = if horizontal {
            Pt::new(to.x - HEAD, to.y)
        } else {
            Pt::new(to.x, to.y - HEAD)
        };
        self.path(
            vec![Segment::Move(from), Segment::Line(base)],
            None,
            Some(stroke),
        );
        let half = HEAD * 0.6;
        let (a, b) = if horizontal {
            ((base.x, base.y - half), (base.x, base.y + half))
        } else {
            ((base.x - half, base.y), (base.x + half, base.y))
        };
        self.path(shapes::polygon(&[a, (to.x, to.y), b]), Some(color), None);
    }
}

/// The fills of section or period `k`: its box, and its events'.
fn fills(theme: &Theme, k: usize) -> (Color, Color) {
    let c = branch_color(theme, k);
    (
        c.with_alpha(0xd0).over(theme.background),
        c.with_alpha(0x40).over(theme.background),
    )
}

/// Draws `timeline` (TIMELI-3).
pub fn timeline(timeline: &Timeline, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let mut p = Painter {
        metrics,
        theme,
        elements: Vec::new(),
    };
    let lh = theme.line_height;
    let title = timeline
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_h = if title.is_some() { lh + 16.0 } else { 0.0 };
    let top = MARGIN + title_h;
    let named = timeline.has_named_sections();
    // Each period: its section, color, label block, and event blocks.
    let periods: Vec<(usize, usize, Block, Vec<Block>)> = timeline
        .periods()
        .enumerate()
        .map(|(i, (s, period))| {
            let color = if named { s } else { i };
            let events = period.events.iter().map(|e| p.block(e, MIN_W)).collect();
            (s, color, p.block(&period.label, MIN_W), events)
        })
        .collect();
    let sections: Vec<Option<Block>> = timeline
        .sections
        .iter()
        .map(|s| {
            // Only a section with periods has a box.
            s.name
                .as_ref()
                .filter(|_| !s.periods.is_empty())
                .map(|n| p.block(n, MIN_W))
        })
        .collect();
    let section_color = |s: usize| if named { s } else { 0 };
    let event_stroke = |k: usize| Some(branch_color(theme, k));

    let (width, height);
    if timeline.direction == Direction::LeftRight {
        // Columns as wide as the widest of their period and events, and their section's name
        // spread over them.
        let mut col_w: Vec<f32> = periods
            .iter()
            .map(|(_, _, b, ev)| ev.iter().map(|e| e.w).fold(b.w, f32::max))
            .collect();
        for (s, block) in sections.iter().enumerate() {
            let Some(block) = block else { continue };
            let cols: Vec<usize> = (0..periods.len()).filter(|&i| periods[i].0 == s).collect();
            if cols.is_empty() {
                continue;
            }
            let span: f32 =
                cols.iter().map(|&i| col_w[i]).sum::<f32>() + GAP * (cols.len() - 1) as f32;
            if block.w > span {
                let extra = (block.w - span) / cols.len() as f32;
                for &i in &cols {
                    col_w[i] += extra;
                }
            }
        }
        let section_h = sections.iter().flatten().map(|b| b.h).fold(0.0, f32::max);
        let period_h = periods.iter().map(|(_, _, b, _)| b.h).fold(0.0, f32::max);
        let period_y = if section_h > 0.0 {
            top + section_h + GAP
        } else {
            top
        };
        let axis_y = period_y + period_h + AXIS_GAP;
        let events_y = axis_y + AXIS_GAP;
        let mut x = MARGIN;
        let mut col_x = Vec::with_capacity(periods.len());
        for w in &col_w {
            col_x.push(x);
            x += w + GAP;
        }
        let right = (x - GAP).max(MARGIN);
        // Sections.
        for (s, block) in sections.iter().enumerate() {
            let Some(block) = block else { continue };
            let cols: Vec<usize> = (0..periods.len()).filter(|&i| periods[i].0 == s).collect();
            let (x0, x1) = match (cols.first(), cols.last()) {
                (Some(&a), Some(&b)) => (col_x[a], col_x[b] + col_w[b]),
                _ => continue,
            };
            let b = BoxF {
                x: x0,
                y: top,
                w: x1 - x0,
                h: section_h,
            };
            let fill = branch_color(theme, section_color(s));
            p.boxed(b, &block.lines, fill, None);
        }
        let mut bottom = events_y;
        // Connectors behind everything else, then the arrow.
        for (i, (_, _, _, events)) in periods.iter().enumerate() {
            let cx = col_x[i] + col_w[i] / 2.0;
            let last = events_y + events.iter().map(|e| e.h + GAP).sum::<f32>()
                - events.last().map_or(0.0, |e| e.h + GAP);
            if !events.is_empty() {
                let k = periods[i].1;
                p.dashed(
                    Pt::new(cx, period_y + period_h),
                    Pt::new(cx, last),
                    branch_color(theme, k),
                );
            }
        }
        p.arrow(Pt::new(MARGIN, axis_y), Pt::new(right + 2.0 * HEAD, axis_y));
        for (i, (_, k, label, events)) in periods.iter().enumerate() {
            let (fill, event_fill) = fills(theme, *k);
            let b = BoxF {
                x: col_x[i],
                y: period_y,
                w: col_w[i],
                h: period_h,
            };
            p.boxed(b, &label.lines, fill, None);
            let mut y = events_y;
            for e in events {
                let b = BoxF {
                    x: col_x[i],
                    y,
                    w: col_w[i],
                    h: e.h,
                };
                p.boxed(b, &e.lines, event_fill, event_stroke(*k));
                y += e.h + GAP;
            }
            bottom = bottom.max(y - GAP);
        }
        width = right + 2.0 * HEAD + MARGIN;
        height = bottom + MARGIN;
    } else {
        // Rows: a section column, the periods, the arrow, and each period's events in a row.
        let section_w = sections.iter().flatten().map(|b| b.w).fold(0.0, f32::max);
        let period_w = periods.iter().map(|(_, _, b, _)| b.w).fold(0.0, f32::max);
        let period_x = if section_w > 0.0 {
            MARGIN + section_w + GAP
        } else {
            MARGIN
        };
        let axis_x = period_x + period_w + AXIS_GAP;
        let events_x = axis_x + AXIS_GAP;
        let mut y = top;
        let mut row_y = Vec::with_capacity(periods.len());
        let mut row_h = Vec::with_capacity(periods.len());
        for (_, _, b, events) in &periods {
            let h = events.iter().map(|e| e.h).fold(b.h, f32::max);
            row_y.push(y);
            row_h.push(h);
            y += h + GAP;
        }
        let bottom = (y - GAP).max(top);
        for (s, block) in sections.iter().enumerate() {
            let Some(block) = block else { continue };
            let rows: Vec<usize> = (0..periods.len()).filter(|&i| periods[i].0 == s).collect();
            let (y0, y1) = match (rows.first(), rows.last()) {
                (Some(&a), Some(&b)) => (row_y[a], row_y[b] + row_h[b]),
                _ => continue,
            };
            let b = BoxF {
                x: MARGIN,
                y: y0,
                w: section_w,
                h: (y1 - y0).max(block.h),
            };
            p.boxed(b, &block.lines, branch_color(theme, section_color(s)), None);
        }
        let mut right = events_x;
        for (i, (_, k, _, events)) in periods.iter().enumerate() {
            if let Some(last) = events.last() {
                let cy = row_y[i] + row_h[i] / 2.0;
                let end = events_x + events.iter().map(|e| e.w + GAP).sum::<f32>() - last.w - GAP;
                p.dashed(
                    Pt::new(period_x + period_w, cy),
                    Pt::new(end, cy),
                    branch_color(theme, *k),
                );
            }
        }
        p.arrow(Pt::new(axis_x, top), Pt::new(axis_x, bottom + 2.0 * HEAD));
        for (i, (_, k, label, events)) in periods.iter().enumerate() {
            let (fill, event_fill) = fills(theme, *k);
            let b = BoxF {
                x: period_x,
                y: row_y[i],
                w: period_w,
                h: row_h[i],
            };
            p.boxed(b, &label.lines, fill, None);
            let mut x = events_x;
            for e in events {
                let b = BoxF {
                    x,
                    y: row_y[i] + (row_h[i] - e.h) / 2.0,
                    w: e.w,
                    h: e.h,
                };
                p.boxed(b, &e.lines, event_fill, event_stroke(*k));
                x += e.w + GAP;
            }
            right = right.max(x - GAP);
        }
        width = right + MARGIN;
        height = bottom + 2.0 * HEAD + MARGIN;
    }
    let width =
        width.max(title.map_or(0.0, |t| metrics.width(t, theme.font_size * 1.2)) + 2.0 * MARGIN);
    let mut elements = p.elements;
    if let Some(t) = title {
        elements.insert(
            0,
            Element {
                owner: Owner::Diagram,
                primitive: Primitive::Text {
                    text: t.to_owned(),
                    center: Pt::new(width / 2.0, MARGIN + lh / 2.0),
                    size: theme.font_size * 1.2,
                    color: theme.text,
                    style: TextStyle::default(),
                },
            },
        );
    }
    Scene {
        width,
        height,
        background: theme.background,
        elements,
    }
}
