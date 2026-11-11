//! Pie chart to scene (MPIE-3).

use std::f32::consts::{FRAC_PI_2, TAU};

use super::git::{ink, pie_color};
use super::shapes::{self, BoxF};
use super::{
    Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle, Theme,
};
use crate::pie::{Pie, percent};

const MARGIN: f32 = 16.0;
const RADIUS: f32 = 130.0;
/// Space between the circle and the legend.
const LEGEND_GAP: f32 = 40.0;
/// Side of a legend swatch.
const SWATCH: f32 = 14.0;
/// Space between a swatch and its text.
const SWATCH_GAP: f32 = 8.0;
/// Height of a legend row.
const ROW: f32 = 22.0;
/// Where a slice's percentage sits, as a fraction of the radius.
const LABEL_AT: f32 = 0.7;

fn path(segments: Vec<Segment>, fill: Option<Color>, stroke: Option<Stroke>) -> Element {
    Element {
        owner: Owner::Diagram,
        primitive: Primitive::Path {
            segments,
            fill,
            stroke,
        },
    }
}

fn text(text: String, center: Pt, size: f32, color: Color) -> Element {
    Element {
        owner: Owner::Diagram,
        primitive: Primitive::Text {
            text,
            center,
            size,
            color,
            style: TextStyle::default(),
        },
    }
}

/// The point at `angle` (radians, clockwise from three o'clock) on a circle.
fn on_circle(c: Pt, r: f32, angle: f32) -> Pt {
    Pt::new(c.x + r * angle.cos(), c.y + r * angle.sin())
}

/// The legend line for a slice: its label, and with `showData` its value.
fn legend_text(pie: &Pie, label: &str, value: f64) -> String {
    if pie.show_data {
        format!("{label} [{value}]")
    } else {
        label.to_owned()
    }
}

/// Draws `pie` (MPIE-3).
pub fn pie(pie: &Pie, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let lh = theme.line_height;
    let fs = theme.font_size;
    let title = pie
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_h = if title.is_some() { lh + 16.0 } else { 0.0 };
    let top = MARGIN + title_h;
    let ordered = pie.ordered();
    let total = pie.total();
    let legend: Vec<String> = ordered
        .iter()
        .map(|s| legend_text(pie, &s.label, s.value))
        .collect();
    let legend_h = legend.len() as f32 * ROW;
    let body_h = (2.0 * RADIUS).max(legend_h);
    let c = Pt::new(MARGIN + RADIUS, top + body_h / 2.0);
    let mut elements = Vec::new();
    let edge = Stroke {
        width: 2.0,
        color: theme.background,
        dash: None,
    };
    let mut angle = -FRAC_PI_2;
    for (i, s) in ordered.iter().enumerate() {
        let share = if total > 0.0 {
            (s.value / total) as f32
        } else {
            0.0
        };
        let sweep = share * TAU;
        let start = angle;
        angle += sweep;
        if percent(s.value, total) == 0 {
            continue;
        }
        let fill = pie_color(theme, i);
        let segments = if share >= 0.9999 {
            shapes::ellipse(c, RADIUS, RADIUS)
        } else {
            vec![
                Segment::Move(c),
                Segment::Line(on_circle(c, RADIUS, start)),
                Segment::Arc {
                    radii: Pt::new(RADIUS, RADIUS),
                    large: sweep > std::f32::consts::PI,
                    sweep: true,
                    to: on_circle(c, RADIUS, angle),
                },
                Segment::Close,
            ]
        };
        elements.push(path(segments, Some(fill), Some(edge)));
        let at = if share >= 0.9999 {
            c
        } else {
            on_circle(c, RADIUS * LABEL_AT, start + sweep / 2.0)
        };
        let pct = format!("{}%", percent(s.value, total));
        elements.push(text(pct, at, fs, ink(fill)));
    }
    let outline = Stroke {
        width: 1.5,
        color: theme.edge,
        dash: None,
    };
    elements.push(path(
        shapes::ellipse(c, RADIUS, RADIUS),
        None,
        Some(outline),
    ));
    // The legend, centered beside the circle.
    let lx = MARGIN + 2.0 * RADIUS + LEGEND_GAP;
    let ly = c.y - legend_h / 2.0;
    let mut right = MARGIN + 2.0 * RADIUS;
    for (i, line) in legend.into_iter().enumerate() {
        let cy = ly + (i as f32 + 0.5) * ROW;
        let b = BoxF {
            x: lx,
            y: cy - SWATCH / 2.0,
            w: SWATCH,
            h: SWATCH,
        };
        elements.push(path(
            shapes::rounded(b, 2.0),
            Some(pie_color(theme, i)),
            None,
        ));
        let w = metrics.width(&line, fs);
        let tx = lx + SWATCH + SWATCH_GAP;
        right = right.max(tx + w);
        elements.push(text(line, Pt::new(tx + w / 2.0, cy), fs, theme.text));
    }
    let title_size = fs * 1.2;
    let width =
        (right + MARGIN).max(title.map_or(0.0, |t| metrics.width(t, title_size)) + 2.0 * MARGIN);
    if let Some(t) = title {
        let at = Pt::new(width / 2.0, MARGIN + lh / 2.0);
        elements.insert(0, text(t.to_owned(), at, title_size, theme.text));
    }
    Scene {
        width,
        height: top + body_h + MARGIN,
        background: theme.background,
        elements,
    }
}
