//! Venn diagram to scene (MVENN-4), as Mermaid draws it.

use super::git::luminance;
use super::shapes;
use super::{
    Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle, Theme,
    parse_color,
};
use crate::venn::layout::{self, Area, Region};
use crate::venn::{Subset, TextNode, Venn};

const WIDTH: f32 = 800.0;
const HEIGHT: f32 = 450.0;
/// Mermaid's sizes are for a 1600 wide drawing; it draws at 800.
const SCALE: f32 = 0.5;
const PADDING: f64 = 15.0;
/// Mermaid's default theme's `venn1` to `venn8`.
const PALETTE: [u32; 8] = [
    0x5353ff, 0xffff45, 0xb5ff20, 0xff53ff, 0x53ffff, 0x45ff45, 0xff5353, 0x45ffff,
];
/// The fill opacity of a set's circle.
const FILL_OPACITY: f32 = 0.1;
const STROKE_OPACITY: f32 = 0.95;
/// Line height of wrapped labels, in ems.
const LINE: f32 = 1.1;

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

/// `c` with its HSL lightness moved by `delta` percentage points.
fn lighten(c: Color, delta: f32) -> Color {
    let (r, g, b) = (
        f32::from(c.r) / 255.0,
        f32::from(c.g) / 255.0,
        f32::from(c.b) / 255.0,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    let (h, s) = if d == 0.0 {
        (0.0, 0.0)
    } else {
        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };
        let h = if max == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        (h / 6.0, s)
    };
    let l = (l + delta / 100.0).clamp(0.0, 1.0);
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let channel = |t: f32| {
        let t = t.rem_euclid(1.0);
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v * 255.0).round() as u8
    };
    Color {
        r: channel(h + 1.0 / 3.0),
        g: channel(h),
        b: channel(h - 1.0 / 3.0),
        a: c.a,
    }
}

/// `c` at `opacity` times its alpha.
fn faded(c: Color, opacity: f32) -> Color {
    c.with_alpha((f32::from(c.a) * opacity.clamp(0.0, 1.0)).round() as u8)
}

/// A CSS number, with an optional `px`.
fn number(v: &str) -> Option<f32> {
    let v = v.trim();
    v.strip_suffix("px").unwrap_or(v).trim().parse().ok()
}

/// Words of `s` in lines no wider than `max`, never breaking a word.
fn wrap(metrics: &impl TextMetrics, s: &str, size: f32, max: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for word in s.split_whitespace() {
        match lines.last_mut() {
            Some(line) if metrics.width(&format!("{line} {word}"), size) <= max => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_owned()),
        }
    }
    lines
}

/// Words of a region's label in up to about three lines, as venn.js wraps them: a line breaks
/// once it is longer than a third of the label and wider than `max`.
fn wrap_label(metrics: &impl TextMetrics, label: &str, size: f32, max: f32) -> Vec<String> {
    let words: Vec<&str> = label.split_whitespace().collect();
    let min_chars = (label.chars().count() + words.len()) as f32 / 3.0;
    let mut lines: Vec<String> = Vec::new();
    for word in words {
        match lines.last_mut() {
            Some(line) => {
                let joined = format!("{line} {word}");
                if joined.chars().count() as f32 > min_chars && metrics.width(&joined, size) > max {
                    lines.push(word.to_owned());
                } else {
                    *line = joined;
                }
            }
            None => lines.push(word.to_owned()),
        }
    }
    lines
}

/// `lines` centered on `at`, `LINE` ems apart.
fn lines_at(out: &mut Vec<Element>, lines: Vec<String>, at: Pt, size: f32, color: Color) {
    let n = lines.len() as f32;
    for (i, l) in lines.into_iter().enumerate() {
        let dy = (i as f32 - (n - 1.0) / 2.0) * LINE * size;
        out.push(text(l, Pt::new(at.x, at.y + dy), size, color));
    }
}

/// The outline of a region, moved down by `dy`.
fn outline(region: &Region, dy: f32) -> Vec<Segment> {
    let pt = |p: layout::Point| Pt::new(p.x as f32, p.y as f32 + dy);
    match region.arcs.as_slice() {
        [] => Vec::new(),
        [only] => {
            let c = only.circle;
            let r = c.radius as f32;
            shapes::ellipse(Pt::new(c.x as f32, c.y as f32 + dy), r, r)
        }
        arcs => {
            let mut out = vec![Segment::Move(pt(arcs[0].p2))];
            for a in arcs {
                let r = a.circle.radius as f32;
                out.push(Segment::Arc {
                    radii: Pt::new(r, r),
                    large: a.large,
                    sweep: true,
                    to: pt(a.p1),
                });
            }
            out.push(Segment::Close);
            out
        }
    }
}

/// Draws `chart` (MVENN-4).
pub fn venn(chart: &Venn, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let title = chart
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let top = if title.is_some() { 48.0 * SCALE } else { 0.0 };
    let regions = chart.regions();
    let areas: Vec<Area> = regions
        .iter()
        .map(|s| Area {
            sets: s.sets.clone(),
            size: s.size,
        })
        .collect();
    let laid = layout::layout(&areas, f64::from(WIDTH), f64::from(HEIGHT - top), PADDING);
    let dark = luminance(theme.background) < 0.5;
    let mut elements = Vec::new();
    if let Some(t) = title {
        elements.push(text(
            t.to_owned(),
            Pt::new(WIDTH / 2.0, 32.0 * SCALE),
            32.0 * SCALE,
            theme.text,
        ));
    }
    let label_size = 48.0 * SCALE;
    let mut circles = 0;
    for (subset, region) in regions.iter().zip(&laid) {
        let Some(region) = region else { continue };
        let style = |p: &str| chart.style(&subset.sets, p);
        let color = style("color").and_then(parse_color);
        let label_color = if subset.sets.len() == 1 {
            let base = style("fill")
                .and_then(parse_color)
                .unwrap_or(Color::rgb(PALETTE[circles % PALETTE.len()]));
            circles += 1;
            let opacity = style("fill-opacity")
                .and_then(number)
                .unwrap_or(FILL_OPACITY);
            let stroke = style("stroke").and_then(parse_color).unwrap_or(base);
            let width = style("stroke-width")
                .and_then(number)
                .unwrap_or(5.0 * SCALE);
            elements.push(Element {
                owner: Owner::Diagram,
                primitive: Primitive::Path {
                    segments: outline(region, top),
                    fill: Some(faded(base, opacity)),
                    stroke: (width > 0.0).then_some(Stroke {
                        width,
                        color: faded(stroke, STROKE_OPACITY),
                        dash: None,
                    }),
                },
            });
            color.unwrap_or_else(|| lighten(base, if dark { 30.0 } else { -30.0 }))
        } else {
            if let Some(fill) = style("fill").and_then(parse_color) {
                elements.push(Element {
                    owner: Owner::Diagram,
                    primitive: Primitive::Path {
                        segments: outline(region, top),
                        fill: Some(fill),
                        stroke: None,
                    },
                });
            }
            color.unwrap_or(theme.text)
        };
        let label = subset
            .label
            .as_deref()
            .filter(|l| !l.is_empty())
            .or((subset.sets.len() == 1).then(|| subset.sets[0].as_str()));
        if let (Some(label), Some(at)) = (label, region.text) {
            let max = region.circles.first().map_or(50.0, |c| c.radius as f32);
            let lines = wrap_label(metrics, label, label_size, max);
            let at = Pt::new(at.x.floor() as f32, at.y.floor() as f32 + top);
            lines_at(&mut elements, lines, at, label_size, label_color);
        }
    }
    text_nodes(chart, &regions, &laid, metrics, theme, top, &mut elements);
    Scene {
        width: WIDTH,
        height: HEIGHT,
        background: theme.background,
        elements,
    }
}

/// Draws the `text` nodes of each region in a grid inside it, as Mermaid does.
fn text_nodes(
    chart: &Venn,
    regions: &[Subset],
    laid: &[Option<Region>],
    metrics: &impl TextMetrics,
    theme: &Theme,
    top: f32,
    out: &mut Vec<Element>,
) {
    let mut groups: Vec<(&[String], Vec<&TextNode>)> = Vec::new();
    for t in &chart.texts {
        match groups.iter_mut().find(|(k, _)| *k == t.sets.as_slice()) {
            Some((_, nodes)) => nodes.push(t),
            None => groups.push((&t.sets, vec![t])),
        }
    }
    let size = 40.0 * SCALE;
    let s = f64::from(SCALE);
    for (key, nodes) in groups {
        // The last region with these sets.
        let Some((subset, region)) = regions
            .iter()
            .zip(laid)
            .rev()
            .find_map(|(s, r)| (s.sets == key).then_some((s, r.as_ref()?)))
        else {
            continue;
        };
        let Some(c) = region.text else { continue };
        let min_radius = region
            .circles
            .iter()
            .map(|c| c.radius)
            .fold(f64::INFINITY, f64::min);
        let raw = region
            .circles
            .iter()
            .map(|k| k.radius - ((c.x - k.x).powi(2) + (c.y - k.y).powi(2)).sqrt())
            .fold(f64::INFINITY, f64::min);
        let mut inner = if raw.is_finite() { raw.max(0.0) } else { 0.0 };
        if inner == 0.0 && min_radius.is_finite() {
            inner = min_radius * 0.6;
        }
        let inner_w = (80.0 * s).max(inner * 2.0 * 0.95);
        let inner_h = (60.0 * s).max(inner * 2.0 * 0.95);
        let has_label = subset.label.as_deref().is_some_and(|l| !l.is_empty());
        let label_offset = if has_label {
            (32.0 * s).min(inner * 0.25)
        } else {
            0.0
        } + if nodes.len() <= 2 { 30.0 * s } else { 0.0 };
        let start_x = c.x - inner_w / 2.0;
        let start_y = c.y - inner_h / 2.0 + label_offset;
        let cols = (nodes.len() as f64).sqrt().ceil().max(1.0) as usize;
        let rows = nodes.len().div_ceil(cols).max(1);
        let cell_w = inner_w / cols as f64;
        let cell_h = inner_h / rows as f64;
        for (i, node) in nodes.into_iter().enumerate() {
            let (col, row) = (i % cols, i / cols);
            let x = start_x + cell_w * (col as f64 + 0.5);
            let y = start_y + cell_h * (row as f64 + 0.5);
            let color = chart
                .style(std::slice::from_ref(&node.id), "color")
                .and_then(parse_color)
                .unwrap_or(theme.text);
            let lines = wrap(metrics, node.text(), size, (cell_w * 0.9) as f32);
            lines_at(out, lines, Pt::new(x as f32, y as f32 + top), size, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Approx;

    #[test]
    fn lightness() {
        // Mermaid's darken of its first venn color.
        assert_eq!(lighten(Color::rgb(0x5353ff), -30.0), Color::rgb(0x0000b9));
        assert_eq!(lighten(Color::rgb(0x808080), 100.0), Color::rgb(0xffffff));
    }

    #[test]
    fn wrapping() {
        assert_eq!(
            wrap(&Approx, "Design Systems", 10.0, 50.0),
            ["Design", "Systems"]
        );
        assert_eq!(wrap(&Approx, "API", 10.0, 1.0), ["API"]);
        // A short label stays on one line however narrow.
        assert_eq!(wrap_label(&Approx, "AB", 24.0, 1.0), ["AB"]);
        assert_eq!(
            wrap_label(&Approx, "a much longer label", 24.0, 100.0).len(),
            3
        );
    }
}
