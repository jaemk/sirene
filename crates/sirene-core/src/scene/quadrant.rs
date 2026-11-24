//! Quadrant chart to scene (MQUAD-3): four tinted quadrants, axis labels, and named points.

use super::Theme;
use super::paint::{Painter, Px};
use super::shapes::{self, BoxF};
use super::{Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle};
use crate::flowchart::{Label, Span};
use crate::quadrant::QuadrantChart;

const MARGIN: f32 = 16.0;
/// Side of the square holding the quadrants.
const SIDE: f32 = 400.0;
/// Padding inside a quadrant.
const PAD: f32 = 8.0;
/// Space between the square and the axis labels.
const GAP: f32 = 10.0;
/// A point's radius unless styled.
const RADIUS: f32 = 5.0;
/// Space between a point and its name.
const NAME_GAP: f32 = 5.0;
/// The tint of each quadrant's fill, from `quadrant-1` to `quadrant-4`.
const TINTS: [u8; 4] = [0x50, 0x38, 0x20, 0x38];

/// The top-left corner of quadrant `k` (0 for `quadrant-1`) in the square.
fn corner(k: usize, side: f32) -> Pt {
    let h = side / 2.0;
    match k {
        0 => Pt::new(h, 0.0),
        1 => Pt::new(0.0, 0.0),
        2 => Pt::new(0.0, h),
        _ => Pt::new(h, h),
    }
}

fn rect(b: BoxF) -> Vec<Segment> {
    let (r, bottom) = (b.right(), b.bottom());
    shapes::polygon(&[(b.x, b.y), (r, b.y), (r, bottom), (b.x, bottom)])
}

/// Wrap labels at spaces and split overlong words without repeatedly measuring prefixes.
fn label_lines(px: &Px<'_, impl TextMetrics>, label: &str, max: f32) -> Vec<Vec<Span>> {
    let mut result = Vec::new();
    let push = |result: &mut Vec<Vec<Span>>, part: String| {
        if px
            .metrics
            .styled_width(&part, px.theme.font_size, TextStyle::default())
            > max
        {
            result.extend(part.chars().map(|ch| vec![Span::plain(ch.to_string())]));
        } else {
            result.push(vec![Span::plain(part)]);
        }
    };
    for line in px.wrap(&Label::plain(label).spans(), max) {
        if px.text_size(std::slice::from_ref(&line)).0 <= max {
            result.push(line);
            continue;
        }
        let mut part = String::new();
        let mut width = 0.0;
        for ch in line.iter().flat_map(|span| span.text.chars()) {
            let advance = px.metrics.styled_width(
                ch.encode_utf8(&mut [0; 4]),
                px.theme.font_size,
                TextStyle::default(),
            );
            if !part.is_empty() && width + advance > max {
                push(&mut result, std::mem::take(&mut part));
                width = 0.0;
            }
            part.push(ch);
            width += advance;
        }
        if !part.is_empty() {
            push(&mut result, part);
        }
    }
    result
}

/// Draws `chart` (MQUAD-3).
pub fn quadrant(chart: &QuadrantChart, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let px = Px::new(metrics, theme);
    let (fs, lh) = (theme.font_size, theme.line_height);
    let small = fs - 2.0;
    let title = chart
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_size = fs * 1.2;
    let title_h = if title.is_some() { lh + 16.0 } else { 0.0 };
    let text_width = |s: &str, size: f32| metrics.styled_width(s, size, TextStyle::default());
    // A single unusually wide glyph must also fit inside a quadrant.
    let glyph_width = chart
        .quadrants
        .iter()
        .flatten()
        .flat_map(|s| s.chars())
        .map(|ch| text_width(ch.encode_utf8(&mut [0; 4]), fs))
        .fold(0.0f32, f32::max);
    let label_width = (SIDE / 2.0 - 2.0 * PAD).max(glyph_width);
    let quadrants: Vec<Option<Vec<Vec<Span>>>> = chart
        .quadrants
        .iter()
        .map(|q| q.as_deref().map(|t| label_lines(&px, t, label_width)))
        .collect();
    // A tall wrapped label gets enough room within its quadrant, including padding.
    let label_height = quadrants
        .iter()
        .flatten()
        .map(|lines| px.text_size(lines).1)
        .fold(0.0f32, f32::max);
    let side = SIDE
        .max(2.0 * (label_width + 2.0 * PAD))
        .max(2.0 * (label_height + 2.0 * PAD));
    let half = side / 2.0;
    let x_labels = chart.x_axis.placed();
    let y_labels = chart.y_axis.placed();
    let gutter = y_labels
        .iter()
        .map(|(t, _)| text_width(t, fs) + GAP)
        .fold(0.0f32, f32::max);
    let x_overlap = x_labels.len() == 2
        && (text_width(x_labels[0].0, fs) + text_width(x_labels[1].0, fs)) / 2.0
            > (x_labels[1].1 - x_labels[0].1) as f32 * side;
    let x_band = if x_labels.is_empty() {
        0.0
    } else {
        GAP + lh * if x_overlap { 2.0 } else { 1.0 }
    };

    // Each point's center, radius, and name, in the square's coordinates.
    let points: Vec<(Pt, f32, f32)> = chart
        .points
        .iter()
        .map(|p| {
            let c = Pt::new(p.x as f32 * side, (1.0 - p.y as f32) * side);
            (
                c,
                p.style.radius.unwrap_or(RADIUS),
                text_width(&p.name, small),
            )
        })
        .collect();
    let (mut left, mut right, mut top, mut bottom) = (-gutter, side, 0.0f32, side + x_band);
    for (point, &(c, r, w)) in chart.points.iter().zip(&points) {
        let edge = r + point.style.stroke_width.unwrap_or(0.0) / 2.0;
        left = left.min(c.x - edge).min(c.x - w / 2.0);
        right = right.max(c.x + edge).max(c.x + w / 2.0);
        top = top.min(c.y - edge);
        bottom = bottom.max(c.y + edge).max(c.y + r + NAME_GAP + lh);
    }
    for (label, f) in &x_labels {
        let x = *f as f32 * side;
        let w = text_width(label, fs);
        left = left.min(x - w / 2.0);
        right = right.max(x + w / 2.0);
    }
    let content_w = right - left;
    let width = (content_w + 2.0 * MARGIN)
        .max(title.map_or(0.0, |t| text_width(t, title_size)) + 2.0 * MARGIN);
    let o = Pt::new((width - content_w) / 2.0 - left, MARGIN + title_h - top);
    let at = |p: Pt| Pt::new(o.x + p.x, o.y + p.y);
    let height = o.y + bottom + MARGIN;

    let owner = Owner::Diagram;
    let mut p = Painter::new(theme, metrics);
    let text = |p: &mut Painter, s: &str, center: Pt, color: Color, size: f32| {
        p.elements.push(Element {
            owner,
            primitive: Primitive::Text {
                text: s.to_owned(),
                center,
                size,
                color,
                style: TextStyle::default(),
            },
        });
    };
    if let Some(t) = title {
        text(
            &mut p,
            t,
            Pt::new(width / 2.0, MARGIN + lh / 2.0),
            theme.text,
            title_size,
        );
    }
    for (k, tint) in TINTS.iter().enumerate() {
        let c = at(corner(k, side));
        let bx = BoxF {
            x: c.x,
            y: c.y,
            w: half,
            h: half,
        };
        let fill = theme.node_stroke.with_alpha(*tint).over(theme.background);
        p.path(owner, rect(bx), Some(fill), None);
    }
    let line = |width: f32| {
        Some(Stroke {
            width,
            color: theme.node_stroke,
            dash: None,
        })
    };
    p.path(
        owner,
        vec![
            Segment::Move(at(Pt::new(half, 0.0))),
            Segment::Line(at(Pt::new(half, side))),
            Segment::Move(at(Pt::new(0.0, half))),
            Segment::Line(at(Pt::new(side, half))),
        ],
        None,
        line(1.0),
    );
    let o2 = at(Pt::new(0.0, 0.0));
    let square = BoxF {
        x: o2.x,
        y: o2.y,
        w: side,
        h: side,
    };
    p.path(owner, rect(square), None, line(2.0));
    // Quadrant labels: at the top of their quadrant, or in its middle when there are no points.
    for (k, lines) in quadrants.iter().enumerate() {
        let Some(lines) = lines else { continue };
        let c = corner(k, side);
        let (_, h) = px.text_size(lines);
        let cy = if points.is_empty() {
            c.y + half / 2.0
        } else {
            c.y + PAD + h / 2.0
        };
        p.lines(owner, lines, at(Pt::new(c.x + half / 2.0, cy)), theme.text);
    }
    for (i, (t, f)) in x_labels.iter().enumerate() {
        let row = if x_overlap { i as f32 } else { 0.0 };
        let c = at(Pt::new(*f as f32 * side, side + GAP + (row + 0.5) * lh));
        text(&mut p, t, c, theme.text, fs);
    }
    for (t, f) in &y_labels {
        let w = text_width(t, fs);
        let c = at(Pt::new(-GAP - w / 2.0, (1.0 - *f as f32) * side));
        text(&mut p, t, c, theme.text, fs);
    }
    // Point `i` and its name are owned by `Owner::Node(i)` (WYSIWY-28).
    for (i, (pt, &(c, r, _))) in chart.points.iter().zip(&points).enumerate() {
        let fill = pt.style.color.unwrap_or(theme.node_stroke);
        let stroke = pt.style.stroke_width.map(|width| Stroke {
            width,
            color: pt.style.stroke_color.unwrap_or(fill),
            dash: None,
        });
        let owner = Owner::Node(i);
        p.path(owner, shapes::ellipse(at(c), r, r), Some(fill), stroke);
        let name_at = at(Pt::new(c.x, c.y + r + NAME_GAP + lh / 2.0));
        p.elements.push(Element {
            owner,
            primitive: Primitive::Text {
                text: pt.name.clone(),
                center: name_at,
                size: small,
                color: theme.text,
                style: TextStyle::default(),
            },
        });
    }
    Scene {
        width,
        height,
        background: theme.background,
        elements: p.elements,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quadrant::{Axis, Point, PointStyle};

    struct Shaped;
    impl TextMetrics for Shaped {
        fn width(&self, text: &str, _: f32) -> f32 {
            text.chars().count() as f32 * 4.0
        }
        fn styled_width(&self, text: &str, _: f32, _: TextStyle) -> f32 {
            // Whole runs can be wider than the sum of their glyphs.
            text.chars().count() as f32 * if text.chars().count() > 1 { 16.0 } else { 10.0 }
        }
    }

    #[test]
    fn overlapping_axis_labels_use_separate_rows() {
        let chart = QuadrantChart {
            x_axis: Axis {
                low: Some("lower axis label long enough to overlap".into()),
                high: Some("higher axis label long enough to overlap".into()),
            },
            ..Default::default()
        };
        let scene = quadrant(&chart, &Shaped, &Theme::LIGHT);
        let locations: Vec<_> = scene
            .elements
            .iter()
            .filter_map(|e| match &e.primitive {
                Primitive::Text { text, center, .. } if text.contains("axis label") => {
                    Some(*center)
                }
                _ => None,
            })
            .collect();
        assert_eq!(locations.len(), 2);
        assert!(locations[1].y - locations[0].y >= Theme::LIGHT.line_height);
        assert!(locations[1].y < scene.height);
    }

    #[test]
    fn labels_and_strokes_fit_scene_with_shaped_metrics() {
        let chart = QuadrantChart {
            x_axis: Axis {
                low: Some("a very long lower axis label that goes beyond the chart".into()),
                high: Some("high".into()),
            },
            quadrants: [
                Some("anunbrokenlabelthatwrapsintomanyseparatelines".repeat(6)),
                None,
                None,
                None,
            ],
            points: vec![Point {
                name: "dot".into(),
                x: 0.0,
                y: 1.0,
                class: None,
                style: PointStyle {
                    radius: Some(10.0),
                    stroke_width: Some(80.0),
                    ..Default::default()
                },
            }],
            ..Default::default()
        };
        let scene = quadrant(&chart, &Shaped, &Theme::LIGHT);
        for element in &scene.elements {
            if let Primitive::Text {
                text,
                center,
                size,
                style,
                ..
            } = &element.primitive
            {
                let w = Shaped.styled_width(text, *size, *style);
                assert!(center.x - w / 2.0 >= 0.0, "{text}");
                assert!(center.x + w / 2.0 <= scene.width, "{text}");
                assert!(center.y >= 0.0 && center.y <= scene.height, "{text}");
            }
        }
        // The chart is enlarged to fit the many wrapped lines in a single quadrant.
        assert!(scene.height > SIDE + 2.0 * MARGIN);
    }
}
