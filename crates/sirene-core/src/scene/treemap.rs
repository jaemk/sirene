//! Treemap to scene (MTREE-3): sections as tinted boxes with a header, and leaves as boxes inside
//! them sized by value, labeled as large as they fit.

use std::collections::HashMap;

use super::Theme;
use super::git::branch_color;
use super::paint::Painter;
use super::shapes;
use super::style;
use super::{Color, Element, Owner, Primitive, Pt, Scene, Stroke, TextMetrics, TextStyle};
use crate::treemap::{Cell, Treemap};

/// A section's header height, as the layout leaves room for.
const HEADER: f32 = 25.0;
/// A section's fill opacity, and a leaf's.
const SECTION_ALPHA: f32 = 0.6;
const LEAF_ALPHA: f32 = 0.3;

/// Leaf label sizes, as Mermaid picks them: smaller once there are more than 20 leaves.
struct Sizes {
    label: f32,
    value: f32,
    min_label: f32,
    min_value: f32,
    padding: f32,
    threshold: f32,
    gap: f32,
}

impl Sizes {
    fn new(leaves: usize) -> Sizes {
        if leaves > 20 {
            Sizes {
                label: 16.0,
                value: 14.0,
                min_label: 4.0,
                min_value: 4.0,
                padding: 2.0,
                threshold: 8.0,
                gap: 1.0,
            }
        } else {
            Sizes {
                label: 38.0,
                value: 28.0,
                min_label: 8.0,
                min_value: 6.0,
                padding: 4.0,
                threshold: 10.0,
                gap: 2.0,
            }
        }
    }

    /// The value's size for a label of `label`.
    fn value_for(&self, label: f32) -> f32 {
        (label * 0.6).round().clamp(self.min_value, self.value)
    }
}

/// The label size and, when it fits under it, the value size for a `w` by `h` leaf; `None` when
/// the label does not fit.
fn fit_leaf(
    metrics: &dyn TextMetrics,
    label: &str,
    value: Option<&str>,
    w: f32,
    h: f32,
    s: &Sizes,
) -> Option<(f32, Option<f32>)> {
    let (aw, ah) = (w - 2.0 * s.padding, h - 2.0 * s.padding);
    if aw < s.threshold || ah < s.threshold {
        return None;
    }
    let mut size = s.label;
    while metrics.width(label, size) > aw && size > s.min_label {
        size -= 1.0;
    }
    while size + s.gap + s.value_for(size) > ah && size > s.min_label {
        size -= 1.0;
    }
    if metrics.width(label, size) > aw || ah < size {
        return None;
    }
    let value = value.and_then(|v| {
        let vs = s.value_for(size);
        let top = h / 2.0 + size / 2.0 + s.gap;
        (metrics.width(v, vs) <= aw && top + vs <= h - 4.0).then_some(vs)
    });
    Some((size, value))
}

/// `s` cut short with `...` to fit `max` at `size`, or empty when even `...` does not fit.
fn truncate(metrics: &dyn TextMetrics, s: &str, size: f32, max: f32) -> String {
    let fits = |t: &str| metrics.styled_width(t, size, BOLD) <= max;
    if fits(s) {
        return s.to_owned();
    }
    let chars: Vec<char> = s.chars().collect();
    for n in (1..chars.len()).rev() {
        let t = format!("{}...", chars[..n].iter().collect::<String>());
        if fits(&t) {
            return t;
        }
    }
    if fits("...") {
        "...".to_owned()
    } else {
        String::new()
    }
}

const BOLD: TextStyle = TextStyle {
    bold: true,
    italic: false,
};
const ITALIC: TextStyle = TextStyle {
    bold: false,
    italic: true,
};

fn alpha(c: Color, a: f32) -> Color {
    c.with_alpha((f32::from(c.a) * a).round() as u8)
}

/// Draws `treemap` (MTREE-3).
pub fn treemap(treemap: &Treemap, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let (fs, lh) = (theme.font_size, theme.line_height);
    let title = treemap
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_size = fs * 1.2;
    let dp = treemap.diagram_padding;
    let top = dp + if title.is_some() { lh + 16.0 } else { 0.0 };
    let width = (treemap.width + 2.0 * dp)
        .max(title.map_or(0.0, |t| metrics.width(t, title_size)) + 2.0 * dp);
    let left = (width - treemap.width) / 2.0;
    let height = top + treemap.height + dp;
    let at = |c: &Cell| {
        (
            Pt::new(left + c.x0, top + c.y0),
            Pt::new(left + c.x1, top + c.y1),
        )
    };

    let owner = Owner::Diagram;
    let mut p = Painter {
        theme,
        metrics,
        elements: Vec::new(),
    };
    let text = |p: &mut Painter, s: &str, center: Pt, size: f32, color: Color, style| {
        p.elements.push(Element {
            owner,
            primitive: Primitive::Text {
                text: s.to_owned(),
                center,
                size,
                color,
                style,
            },
        });
    };
    if let Some(t) = title {
        text(
            &mut p,
            t,
            Pt::new(width / 2.0, dp + lh / 2.0),
            title_size,
            theme.text,
            BOLD,
        );
    }

    let cells = treemap.layout();
    let values = treemap.values();
    // Sections breadth first, largest first, as Mermaid colors them: one color per name.
    let mut order = Vec::new();
    let mut level = treemap.by_value(treemap.roots(), &values);
    while !level.is_empty() {
        let mut next = Vec::new();
        for &i in &level {
            let item = &treemap.items[i];
            if !item.children.is_empty() {
                order.push(i);
                next.extend(treemap.by_value(item.children.iter().copied(), &values));
            }
        }
        level = next;
    }
    let mut colors: HashMap<&str, Color> = HashMap::new();
    for &i in &order {
        let n = colors.len();
        colors
            .entry(treemap.items[i].name.as_str())
            .or_insert_with(|| branch_color(theme, n));
    }
    let rect = |a: Pt, b: Pt| shapes::polygon(&[(a.x, a.y), (b.x, a.y), (b.x, b.y), (a.x, b.y)]);

    for &i in &order {
        let item = &treemap.items[i];
        let (a, b) = at(&cells[i]);
        let r = style::resolve(&item.styles, &[], &[], &[]);
        let c = colors[item.name.as_str()];
        p.path(
            owner,
            rect(a, b),
            Some(alpha(r.fill.unwrap_or(c), SECTION_ALPHA)),
            Some(Stroke {
                width: r.stroke_width.unwrap_or(2.0),
                color: r.stroke.unwrap_or(alpha(c, 0.4)),
                dash: None,
            }),
        );
        let ink = r.color.unwrap_or(theme.text);
        let w = b.x - a.x;
        let value =
            (treemap.show_values && values[i] > 0.0).then(|| treemap.format_value(values[i]));
        let room = if value.is_some() {
            w - 6.0 - 50.0
        } else {
            w - 12.0
        };
        let label = truncate(metrics, &item.name, 12.0, room.max(15.0));
        if !label.is_empty() {
            let lw = metrics.styled_width(&label, 12.0, BOLD);
            text(
                &mut p,
                &label,
                Pt::new(a.x + 6.0 + lw / 2.0, a.y + HEADER / 2.0),
                12.0,
                ink,
                BOLD,
            );
        }
        if let Some(v) = value {
            let vw = metrics.styled_width(&v, 10.0, ITALIC);
            text(
                &mut p,
                &v,
                Pt::new(b.x - 10.0 - vw / 2.0, a.y + HEADER / 2.0),
                10.0,
                ink,
                ITALIC,
            );
        }
    }

    let leaves: Vec<usize> = (0..treemap.items.len())
        .filter(|&i| treemap.items[i].children.is_empty())
        .collect();
    let sizes = Sizes::new(leaves.len());
    for &i in &leaves {
        let item = &treemap.items[i];
        let (a, b) = at(&cells[i]);
        let r = style::resolve(&item.styles, &[], &[], &[]);
        let parent = item
            .parent
            .and_then(|p| colors.get(treemap.items[p].name.as_str()));
        let (fill, stroke) = match parent {
            Some(&c) => (
                Some(alpha(r.fill.unwrap_or(c), LEAF_ALPHA)),
                Stroke {
                    width: r.stroke_width.unwrap_or(3.0),
                    color: r.stroke.unwrap_or(c),
                    dash: None,
                },
            ),
            // A top-level leaf has no section to take a color from.
            None => (
                r.fill.map(|f| alpha(f, LEAF_ALPHA)),
                Stroke {
                    width: r.stroke_width.unwrap_or(1.0),
                    color: r.stroke.unwrap_or(theme.edge),
                    dash: None,
                },
            ),
        };
        p.path(owner, rect(a, b), fill, Some(stroke));
        let value =
            (treemap.show_values && values[i] > 0.0).then(|| treemap.format_value(values[i]));
        let (w, h) = (b.x - a.x, b.y - a.y);
        let Some((size, value_size)) =
            fit_leaf(metrics, &item.name, value.as_deref(), w, h, &sizes)
        else {
            continue;
        };
        let ink = r.color.unwrap_or(theme.text);
        let cy = (a.y + b.y) / 2.0;
        text(
            &mut p,
            &item.name,
            Pt::new((a.x + b.x) / 2.0, cy),
            size,
            ink,
            TextStyle::default(),
        );
        if let (Some(v), Some(vs)) = (value, value_size) {
            let y = cy + size / 2.0 + sizes.gap + vs / 2.0;
            text(
                &mut p,
                &v,
                Pt::new((a.x + b.x) / 2.0, y),
                vs,
                ink,
                TextStyle::default(),
            );
        }
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
    use crate::scene::Approx;

    #[test]
    fn leaf_labels_shrink_to_fit() {
        let s = Sizes::new(3);
        let big = fit_leaf(&Approx, "A", Some("10"), 400.0, 300.0, &s).unwrap();
        assert_eq!(big, (38.0, Some(23.0)));
        let small = fit_leaf(&Approx, "A long name", Some("10"), 80.0, 300.0, &s).unwrap();
        assert!(small.0 < 38.0);
        assert!(fit_leaf(&Approx, "A", None, 12.0, 300.0, &s).is_none());
        assert_eq!(Sizes::new(21).label, 16.0);
    }

    #[test]
    fn truncation() {
        assert_eq!(truncate(&Approx, "abc", 12.0, 1000.0), "abc");
        let t = truncate(&Approx, "a long section name", 12.0, 60.0);
        assert!(t.ends_with("...") && t.len() < 19, "{t}");
        assert_eq!(truncate(&Approx, "abc", 12.0, 1.0), "");
    }
}
