//! Treemap to scene (MTREE-3): sections as tinted boxes with a header, and leaves as boxes inside
//! them sized by value, labeled as large as they fit.

use std::collections::HashMap;

use super::Theme;
use super::git::scale_color;
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
    while (metrics.width(label, size) > aw || size > ah) && size > s.min_label {
        size -= 1.0;
    }
    if metrics.width(label, size) > aw || ah < size {
        return None;
    }
    let label_only = size;
    if let Some(v) = value {
        while size >= s.min_label {
            let vs = s.value_for(size);
            let top = h / 2.0 + size / 2.0 + s.gap;
            if metrics.width(v, vs) <= aw && top + vs <= h - s.padding {
                return Some((size, Some(vs)));
            }
            size -= 1.0;
        }
    }
    Some((label_only, None))
}

/// `s` cut short with `...` to fit `max` at `size`, or empty when even `...` does not fit.
fn truncate(metrics: &dyn TextMetrics, s: &str, size: f32, max: f32) -> String {
    let fits = |t: &str| metrics.styled_width(t, size, BOLD) <= max;
    if fits(s) {
        return s.to_owned();
    }
    if metrics.monotonic_prefixes(s, "...", size, BOLD) {
        let ends: Vec<usize> = s.char_indices().map(|(i, _)| i).collect();
        let mut candidate = String::with_capacity(s.len() + 3);
        let (mut lo, mut hi) = (0, ends.len());
        // Find the first non-fitting prefix. As in the fallback, the full label plus
        // ellipsis is excluded; an empty prefix represents the ellipsis alone.
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            candidate.clear();
            candidate.push_str(&s[..ends[mid]]);
            candidate.push_str("...");
            if fits(&candidate) {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        candidate.clear();
        if lo > 0 {
            candidate.push_str(&s[..ends[lo - 1]]);
            candidate.push_str("...");
        }
        return candidate;
    }
    let mut t = String::with_capacity(s.len() + 3);
    t.push_str(s);
    for (end, _) in s.char_indices().rev() {
        if end == 0 {
            break;
        }
        t.truncate(end);
        t.push_str("...");
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
        .max(title.map_or(0.0, |t| metrics.styled_width(t, title_size, BOLD)) + 2.0 * dp);
    let left = (width - treemap.width) / 2.0;
    let height = top + treemap.height + dp;
    let at = |c: &Cell| {
        (
            Pt::new(left + c.x0, top + c.y0),
            Pt::new(left + c.x1, top + c.y1),
        )
    };

    let owner = Owner::Diagram;
    let mut p = Painter::new(theme, metrics);
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
            .or_insert_with(|| scale_color(theme, n));
    }
    let rect = |a: Pt, b: Pt| shapes::polygon(&[(a.x, a.y), (b.x, a.y), (b.x, b.y), (a.x, b.y)]);

    for &i in &order {
        let item = &treemap.items[i];
        let (a, b) = at(&cells[i]);
        let r = style::resolve(&item.styles, &[], &[], &style::Defs::default());
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
        let header_height = (b.y - a.y).min(HEADER);
        let value = value
            .filter(|v| header_height >= 12.0 && metrics.styled_width(v, 10.0, ITALIC) + 22.0 <= w);
        let value_width = value
            .as_ref()
            .map_or(0.0, |v| metrics.styled_width(v, 10.0, ITALIC));
        let room = w
            - 12.0
            - if value.is_some() {
                value_width + 10.0
            } else {
                0.0
            };
        let label = if header_height >= 12.0 {
            truncate(metrics, &item.name, 12.0, room)
        } else {
            String::new()
        };
        // Prefer the section name if both text elements cannot fit side by side.
        let value = if label.is_empty() { None } else { value };
        let label = if value.is_none() && header_height >= 12.0 {
            truncate(metrics, &item.name, 12.0, w - 12.0)
        } else {
            label
        };
        if !label.is_empty() {
            let lw = metrics.styled_width(&label, 12.0, BOLD);
            text(
                &mut p,
                &label,
                Pt::new(a.x + 6.0 + lw / 2.0, a.y + header_height / 2.0),
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
                Pt::new(b.x - 10.0 - vw / 2.0, a.y + header_height / 2.0),
                10.0,
                ink,
                ITALIC,
            );
        }
    }

    let leaves: Vec<usize> = (0..treemap.items.len())
        .filter(|&i| !treemap.items[i].is_section())
        .collect();
    let sizes = Sizes::new(leaves.len());
    for &i in &leaves {
        let item = &treemap.items[i];
        let (a, b) = at(&cells[i]);
        let r = style::resolve(&item.styles, &[], &[], &style::Defs::default());
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
        assert_eq!(
            fit_leaf(&Approx, "A", None, 100.0, 50.0, &s),
            Some((38.0, None))
        );
        assert_eq!(
            fit_leaf(
                &Approx,
                "A",
                Some("an impossibly long value that cannot fit even at the minimum size"),
                100.0,
                50.0,
                &s
            ),
            Some((38.0, None))
        );
    }

    #[test]
    fn section_text_fits_and_empty_sections_are_not_drawn() {
        let (mut t, _) = crate::treemap::parse(
            "treemap-beta\n\"A long section\"\n  \"leaf\": 123456789\n\"empty\"\n",
        )
        .lower();
        t.width = 80.0;
        t.height = 60.0;
        let scene = treemap(&t, &Approx, &Theme::default());
        let cell = t.layout()[0];
        let left = (scene.width - t.width) / 2.0 + cell.x0;
        let right = (scene.width - t.width) / 2.0 + cell.x1;
        let texts: Vec<_> = scene
            .elements
            .iter()
            .filter_map(|e| match &e.primitive {
                Primitive::Text {
                    text,
                    center,
                    size,
                    style,
                    ..
                } if *style == BOLD || *style == ITALIC => Some((text, center, size, style)),
                _ => None,
            })
            .collect();
        assert!(!texts.is_empty());
        for (text, center, size, style) in &texts {
            let half = Approx.styled_width(text, **size, **style) / 2.0;
            assert!(center.x - half >= left && center.x + half <= right);
            assert_ne!(text.as_str(), "empty");
        }
        if texts.len() == 2 {
            let a = &texts[0];
            let b = &texts[1];
            assert!(
                a.1.x + Approx.styled_width(a.0, *a.2, *a.3) / 2.0
                    <= b.1.x - Approx.styled_width(b.0, *b.2, *b.3) / 2.0
            );
        }
    }

    #[test]
    fn section_label_and_value_do_not_overlap() {
        let (mut t, _) =
            crate::treemap::parse("treemap-beta\n\"section\"\n  \"leaf\": 123\n").lower();
        t.width = 200.0;
        let scene = treemap(&t, &Approx, &Theme::default());
        let texts: Vec<_> = scene
            .elements
            .iter()
            .filter_map(|e| match &e.primitive {
                Primitive::Text {
                    text,
                    center,
                    size,
                    style,
                    ..
                } if *style == BOLD || *style == ITALIC => Some((text, center, size, style)),
                _ => None,
            })
            .collect();
        assert_eq!(texts.len(), 2);
        assert_eq!(texts[0].0, "section");
        assert_eq!(texts[1].0, "123");
        assert!(
            texts[0].1.x + Approx.styled_width(texts[0].0, *texts[0].2, *texts[0].3) / 2.0
                <= texts[1].1.x - Approx.styled_width(texts[1].0, *texts[1].2, *texts[1].3) / 2.0
        );
    }

    #[test]
    fn title_width_uses_bold_metrics() {
        struct BoldWide;
        impl TextMetrics for BoldWide {
            fn width(&self, text: &str, size: f32) -> f32 {
                Approx.width(text, size)
            }
            fn styled_width(&self, text: &str, size: f32, style: TextStyle) -> f32 {
                self.width(text, size) * if style.bold { 2.0 } else { 1.0 }
            }
        }
        let t = Treemap {
            title: Some("A long title".into()),
            width: 1.0,
            ..Treemap::default()
        };
        let theme = Theme::default();
        let scene = treemap(&t, &BoldWide, &theme);
        assert!(
            scene.width
                >= BoldWide.styled_width(t.title.as_deref().unwrap(), theme.font_size * 1.2, BOLD)
                    + 2.0 * t.diagram_padding
        );
    }

    #[test]
    fn truncation() {
        assert_eq!(truncate(&Approx, "abc", 12.0, 1000.0), "abc");
        let t = truncate(&Approx, "a long section name", 12.0, 60.0);
        assert!(t.ends_with("...") && t.len() < 19, "{t}");
        assert_eq!(truncate(&Approx, "abc", 12.0, 1.0), "");
    }

    #[test]
    fn truncation_preserves_unicode_prefixes() {
        check_truncation(&Approx);
        #[cfg(feature = "png")]
        check_truncation(&crate::scene::png::FontMetrics);
    }

    fn check_truncation(metrics: &dyn TextMetrics) {
        for text in [
            "",
            "abc",
            "a long section name",
            "界",
            "é界abc",
            "a\u{301}界🙂xyz",
        ] {
            for max in [0.0, 22.0, 40.0, 60.0, 1000.0] {
                let chars: Vec<char> = text.chars().collect();
                let fits = |s: &str| metrics.styled_width(s, 12.0, BOLD) <= max;
                let expected = if fits(text) {
                    text.to_owned()
                } else {
                    (1..chars.len())
                        .rev()
                        .map(|n| format!("{}...", chars[..n].iter().collect::<String>()))
                        .find(|s| fits(s))
                        .unwrap_or_else(|| {
                            if fits("...") {
                                "...".to_owned()
                            } else {
                                String::new()
                            }
                        })
                };
                assert_eq!(truncate(metrics, text, 12.0, max), expected);
            }
        }
    }
}
