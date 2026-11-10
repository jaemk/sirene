//! Sankey diagram to scene (MSANKEY-3): node bars in columns, and a band per link as wide as
//! its value, colored by its source, its target, or a blend of both.

use super::Theme;
use super::git::branch_color;
use super::paint::Painter;
use super::shapes;
use super::{Color, Element, Owner, Primitive, Pt, Scene, Segment, TextMetrics, TextStyle};
use crate::sankey::{LinkColor, Sankey};
use crate::scene::parse_color;

const MARGIN: f32 = 16.0;
/// Space between a node and its label.
const LABEL_GAP: f32 = 6.0;
/// A link's opacity.
const LINK_ALPHA: u8 = 0x80;
/// Slices a gradient link is drawn in.
const SLICES: usize = 16;

/// Node `k`'s color.
fn node_color(theme: &Theme, k: usize) -> Color {
    branch_color(theme, k)
}

fn lerp(a: Pt, b: Pt, t: f32) -> Pt {
    Pt::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

/// The part of the cubic `c` between `t0` and `t1`.
fn sub_cubic(c: [Pt; 4], t0: f32, t1: f32) -> [Pt; 4] {
    // Split at t1 and keep the first half, then split that at t0 / t1 and keep the second.
    let split = |c: [Pt; 4], t: f32| -> ([Pt; 4], [Pt; 4]) {
        let (a, b, cc) = (
            lerp(c[0], c[1], t),
            lerp(c[1], c[2], t),
            lerp(c[2], c[3], t),
        );
        let (d, e) = (lerp(a, b, t), lerp(b, cc, t));
        let f = lerp(d, e, t);
        ([c[0], a, d, f], [f, e, cc, c[3]])
    };
    let (head, _) = split(c, t1);
    if t1 <= 0.0 {
        return [c[0]; 4];
    }
    split(head, t0 / t1).1
}

/// The band between `top` and `bottom` (the same curve shifted down) from `t0` to `t1`.
fn band(top: [Pt; 4], bottom: [Pt; 4], t0: f32, t1: f32) -> Vec<Segment> {
    let a = sub_cubic(top, t0, t1);
    let b = sub_cubic(bottom, t0, t1);
    vec![
        Segment::Move(a[0]),
        Segment::Cubic {
            c1: a[1],
            c2: a[2],
            to: a[3],
        },
        Segment::Line(b[3]),
        Segment::Cubic {
            c1: b[2],
            c2: b[1],
            to: b[0],
        },
        Segment::Close,
    ]
}

/// `a` blended toward `b` by `t`.
fn blend(a: Color, b: Color, t: f32) -> Color {
    b.with_alpha((t.clamp(0.0, 1.0) * 255.0).round() as u8)
        .over(a)
}

/// Draws `sankey` (MSANKEY-3).
pub fn sankey(sankey: &Sankey, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let (fs, lh) = (theme.font_size, theme.line_height);
    let small = (fs - 2.0).max(8.0);
    let title = sankey
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_size = fs * 1.2;
    let layout = sankey.layout();
    let top = MARGIN + if title.is_some() { lh + 16.0 } else { 0.0 };
    let width = (sankey.width + 2.0 * MARGIN)
        .max(title.map_or(0.0, |t| metrics.width(t, title_size)) + 2.0 * MARGIN);
    let left = (width - sankey.width) / 2.0;
    let height = top + sankey.height + MARGIN;
    let at = |x: f32, y: f32| Pt::new(left + x, top + y);

    let owner = Owner::Diagram;
    let mut p = Painter {
        theme,
        metrics,
        elements: Vec::new(),
    };
    let text = |p: &mut Painter, s: &str, center: Pt, size: f32| {
        p.elements.push(Element {
            owner,
            primitive: Primitive::Text {
                text: s.to_owned(),
                center,
                size,
                color: theme.text,
                style: TextStyle::default(),
            },
        });
    };
    if let Some(t) = title {
        text(
            &mut p,
            t,
            Pt::new(width / 2.0, MARGIN + lh / 2.0),
            title_size,
        );
    }

    // Links under the nodes.
    let fixed = match &sankey.link_color {
        LinkColor::Fixed(c) => parse_color(c),
        _ => None,
    };
    for (l, b) in sankey.links.iter().zip(&layout.links) {
        let (s, t) = (layout.nodes[l.source], layout.nodes[l.target]);
        let (x0, x1) = (s.x1, t.x0);
        let mid = (x0 + x1) / 2.0;
        let w = b.width.max(1.0);
        let curve = |dy: f32| {
            [
                at(x0, b.y0 + dy),
                at(mid, b.y0 + dy),
                at(mid, b.y1 + dy),
                at(x1, b.y1 + dy),
            ]
        };
        let (upper, lower) = (curve(-w / 2.0), curve(w / 2.0));
        let (cs, ct) = (node_color(theme, l.source), node_color(theme, l.target));
        match (&sankey.link_color, fixed) {
            (LinkColor::Gradient, _) => {
                // Opaque slices, each reaching a little into the next, so no seam shows
                // between them; the color is the link's at half opacity over the background.
                for k in 0..SLICES {
                    let (t0, t1) = (k as f32 / SLICES as f32, (k + 1) as f32 / SLICES as f32);
                    let c = blend(cs, ct, (t0 + t1) / 2.0)
                        .with_alpha(LINK_ALPHA)
                        .over(theme.background);
                    let reach = if k + 1 < SLICES { 0.01 } else { 0.0 };
                    p.path(owner, band(upper, lower, t0, t1 + reach), Some(c), None);
                }
            }
            (kind, fixed) => {
                let c = match kind {
                    LinkColor::Target => ct,
                    LinkColor::Fixed(_) => fixed.unwrap_or(theme.edge),
                    _ => cs,
                };
                p.path(
                    owner,
                    band(upper, lower, 0.0, 1.0),
                    Some(c.with_alpha(LINK_ALPHA)),
                    None,
                );
            }
        }
    }

    // Nodes, and their labels toward the middle.
    for (k, (name, n)) in sankey.nodes.iter().zip(&layout.nodes).enumerate() {
        let (a, b) = (at(n.x0, n.y0), at(n.x1, n.y1.max(n.y0 + 1.0)));
        p.path(
            owner,
            shapes::polygon(&[(a.x, a.y), (b.x, a.y), (b.x, b.y), (a.x, b.y)]),
            Some(node_color(theme, k)),
            None,
        );
        // One line, the value after the name, as Mermaid's label reads.
        let label = if sankey.show_values {
            format!("{name} {}", sankey.format_value(n.value))
        } else {
            name.clone()
        };
        let w = metrics.width(&label, small);
        let x = if n.x0 < sankey.width / 2.0 {
            b.x + LABEL_GAP + w / 2.0
        } else {
            a.x - LABEL_GAP - w / 2.0
        };
        text(&mut p, &label, Pt::new(x, (a.y + b.y) / 2.0), small);
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

    #[test]
    fn sub_curves() {
        let c = [
            Pt::new(0.0, 0.0),
            Pt::new(1.0, 0.0),
            Pt::new(2.0, 0.0),
            Pt::new(3.0, 0.0),
        ];
        let s = sub_cubic(c, 0.0, 1.0);
        assert_eq!(s, c);
        let s = sub_cubic(c, 0.5, 1.0);
        assert!((s[0].x - 1.5).abs() < 1e-5 && (s[3].x - 3.0).abs() < 1e-5);
        let s = sub_cubic(c, 0.25, 0.5);
        assert!((s[0].x - 0.75).abs() < 1e-5 && (s[3].x - 1.5).abs() < 1e-5);
    }

    #[test]
    fn blends() {
        let (a, b) = (Color::rgb(0x000000), Color::rgb(0xffffff));
        assert_eq!(blend(a, b, 0.0), a);
        assert_eq!(blend(a, b, 1.0), b);
        assert_eq!(blend(a, b, 0.5).r, 128);
    }
}
