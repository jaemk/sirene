//! Git graph to scene (MGIT-4).

use super::paint::rounded_polyline;
use super::shapes::{self, BoxF};
use super::{
    Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle, Theme,
};
use crate::git::{Commit, CommitType, Direction, GitGraph};

const MARGIN: f32 = 16.0;
/// Radius of a commit.
const R: f32 = 9.0;
/// Least distance between commits along the lanes, left to right.
const STEP: f32 = 50.0;
/// Distance between commits along the lanes, top to bottom.
const STEP_DOWN: f32 = 44.0;
/// Least space between two labels on one lane.
const GAP: f32 = 12.0;
/// Space between labels, and between a label and its commit.
const SPACE: f32 = 6.0;
/// Horizontal padding inside a label.
const PAD: f32 = 6.0;
/// Space between lanes, beyond their labels.
const LANE_GAP: f32 = 24.0;
/// Radius of a line's corners.
const CORNER: f32 = 10.0;

/// Branch colors on a light background, then on a dark one.
const LIGHT: [u32; 8] = [
    0x4c6ef5, 0xe8590c, 0x2b8a3e, 0xae3ec9, 0x1098ad, 0xe03131, 0xf08c00, 0x5c940d,
];
const DARK: [u32; 8] = [
    0x748ffc, 0xff922b, 0x51cf66, 0xcc5de8, 0x22b8cf, 0xff6b6b, 0xfcc419, 0x94d82d,
];

fn luminance(c: Color) -> f32 {
    (0.2126 * f32::from(c.r) + 0.7152 * f32::from(c.g) + 0.0722 * f32::from(c.b)) / 255.0
}

/// Branch `k`'s color in `theme`.
pub(super) fn branch_color(theme: &Theme, k: usize) -> Color {
    let palette = if luminance(theme.background) < 0.5 {
        DARK
    } else {
        LIGHT
    };
    Color::rgb(palette[k % palette.len()])
}

/// Black or white, whichever reads better on `fill`.
fn ink(fill: Color) -> Color {
    if luminance(fill) > 0.6 {
        Color::rgb(0x222222)
    } else {
        Color::rgb(0xffffff)
    }
}

fn mirror_segment(s: Segment, axis: f32) -> Segment {
    let m = |p: Pt| Pt::new(p.x, axis - p.y);
    match s {
        Segment::Move(p) => Segment::Move(m(p)),
        Segment::Line(p) => Segment::Line(m(p)),
        Segment::Arc {
            radii,
            large,
            sweep,
            to,
        } => Segment::Arc {
            radii,
            large,
            sweep: !sweep,
            to: m(to),
        },
        Segment::Cubic { c1, c2, to } => Segment::Cubic {
            c1: m(c1),
            c2: m(c2),
            to: m(to),
        },
        Segment::Close => Segment::Close,
    }
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

    /// The width of a label holding `text`.
    fn label_width(&self, text: &str) -> f32 {
        self.width(text) + 2.0 * PAD
    }

    fn label_height(&self) -> f32 {
        self.theme.line_height + 4.0
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

    /// A label: `text` in a box centered on `center`.
    fn label(&mut self, text: &str, center: Pt, fill: Color, stroke: Option<Color>, color: Color) {
        let (w, h) = (self.label_width(text), self.label_height());
        let b = BoxF {
            x: center.x - w / 2.0,
            y: center.y - h / 2.0,
            w,
            h,
        };
        let stroke = stroke.map(|color| Stroke {
            width: 1.0,
            color,
            dash: None,
        });
        self.path(shapes::rounded(b, 3.0), Some(fill), stroke);
        self.text(text, center, self.theme.font_size, color);
    }

    fn commit(&mut self, c: &Commit, at: Pt, color: Color) {
        let bg = self.theme.background;
        let kind = c.kind;
        if kind == CommitType::Highlight {
            let corners = [
                (at.x - R, at.y - R),
                (at.x + R, at.y - R),
                (at.x + R, at.y + R),
                (at.x - R, at.y + R),
            ];
            self.path(shapes::polygon(&corners), Some(color), None);
            return;
        }
        self.path(shapes::ellipse(at, R, R), Some(color), None);
        let d = R * 0.5;
        if kind == CommitType::Reverse {
            let cross = vec![
                Segment::Move(Pt::new(at.x - d, at.y - d)),
                Segment::Line(Pt::new(at.x + d, at.y + d)),
                Segment::Move(Pt::new(at.x + d, at.y - d)),
                Segment::Line(Pt::new(at.x - d, at.y + d)),
            ];
            let stroke = Stroke {
                width: 2.0,
                color: bg,
                dash: None,
            };
            self.path(cross, None, Some(stroke));
        } else if c.is_merge() {
            self.path(shapes::ellipse(at, d, d), Some(bg), None);
        } else if c.cherry_pick.is_some() {
            for dx in [-3.0, 3.0] {
                let dot = Pt::new(at.x + dx, at.y + 2.0);
                self.path(shapes::ellipse(dot, 2.0, 2.0), Some(bg), None);
            }
        }
    }
}

/// Draws `graph` (MGIT-4).
pub fn git_graph(graph: &GitGraph, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let mut p = Painter {
        metrics,
        theme,
        elements: Vec::new(),
    };
    let lh = theme.line_height;
    let label_h = p.label_height();
    let branch_h = lh + 8.0;
    let title = graph
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_h = if title.is_some() { lh + 12.0 } else { 0.0 };
    let top = MARGIN + title_h;
    let lanes = graph.branches.len();
    let branch_w: Vec<f32> = graph
        .branches
        .iter()
        .map(|b| p.label_width(&b.name) + 2.0 * PAD)
        .collect();
    let id_w: Vec<f32> = graph.commits.iter().map(|c| p.label_width(&c.id)).collect();
    let tag_w: Vec<Vec<f32>> = graph
        .commits
        .iter()
        .map(|c| c.tags.iter().map(|t| p.label_width(t)).collect())
        .collect();
    let on_lane = |k: usize| {
        graph
            .commits
            .iter()
            .enumerate()
            .filter(move |(_, c)| c.branch == k)
    };

    // Each commit's position along its lane, each lane's across, and where lanes start and end.
    let mut along = Vec::with_capacity(graph.commits.len());
    let mut cross = Vec::with_capacity(lanes);
    let (start, end, width, height);
    if graph.direction == Direction::LeftRight {
        let label_w = branch_w.iter().copied().fold(0.0, f32::max);
        start = MARGIN + label_w;
        let mut last_on: Vec<Option<usize>> = vec![None; lanes];
        let wide = |i: usize| tag_w[i].iter().copied().fold(id_w[i], f32::max);
        for (i, c) in graph.commits.iter().enumerate() {
            let mut a = match along.last() {
                Some(&prev) => prev + STEP,
                None => start + 24.0 + wide(i) / 2.0,
            };
            if let Some(j) = last_on[c.branch] {
                a = f32::max(a, along[j] + (wide(j) + wide(i)) / 2.0 + GAP);
            }
            along.push(a);
            last_on[c.branch] = Some(i);
        }
        end = along.last().map_or(start + 48.0, |&a| a + 24.0);
        let mut y = top;
        for k in 0..lanes {
            let tags = on_lane(k).map(|(i, _)| tag_w[i].len()).max().unwrap_or(0);
            let above = if tags > 0 {
                R + SPACE + tags as f32 * (label_h + 4.0)
            } else {
                R
            };
            let above = above.max(branch_h / 2.0);
            let below = (R + 4.0 + label_h).max(branch_h / 2.0);
            if k > 0 {
                y += LANE_GAP;
            }
            cross.push(y + above);
            y += above + below;
        }
        let tail = along
            .iter()
            .zip(&id_w)
            .map(|(a, w)| a + w / 2.0)
            .fold(end, f32::max);
        width = tail + MARGIN;
        height = y + MARGIN;
    } else {
        // Laid out top to bottom; bottom to top mirrors it below.
        let label_top = top;
        start = label_top + branch_h;
        for i in 0..graph.commits.len() {
            along.push(start + 24.0 + i as f32 * STEP_DOWN);
        }
        end = along.last().map_or(start + 48.0, |&a| a + 24.0);
        // Ids to the left, where the first lane has no lines; tags to the right.
        let left = |k: usize| {
            let ids = on_lane(k).map(|(i, _)| id_w[i]).fold(0.0, f32::max);
            (R + SPACE + ids).max(branch_w[k] / 2.0)
        };
        let right = |k: usize| {
            let tags = on_lane(k)
                .map(|(i, _)| tag_w[i].iter().map(|w| w + 4.0).sum::<f32>())
                .fold(0.0, f32::max);
            let tags = if tags > 0.0 { R + SPACE + tags } else { R };
            tags.max(branch_w[k] / 2.0)
        };
        let mut x = MARGIN;
        for k in 0..lanes {
            if k > 0 {
                x += LANE_GAP;
            }
            cross.push(x + left(k));
            x += left(k) + right(k);
        }
        width = x + MARGIN;
        height = end + MARGIN;
    }
    let width =
        width.max(title.map_or(0.0, |t| metrics.width(t, theme.font_size * 1.2)) + 2.0 * MARGIN);
    let lr = graph.direction == Direction::LeftRight;
    let pt = |a: f32, c: f32| if lr { Pt::new(a, c) } else { Pt::new(c, a) };

    // Lanes and their names.
    for (k, b) in graph.branches.iter().enumerate() {
        let color = branch_color(theme, k);
        let (from, label) = if lr {
            let w = branch_w[k];
            (MARGIN + w, Pt::new(MARGIN + w / 2.0, cross[k]))
        } else {
            (start, Pt::new(cross[k], top + branch_h / 2.0))
        };
        let lane = vec![
            Segment::Move(pt(from, cross[k])),
            Segment::Line(pt(end, cross[k])),
        ];
        let stroke = Stroke {
            width: 1.0,
            color: color.with_alpha(0x99),
            dash: Some((4.0, 4.0)),
        };
        p.path(lane, None, Some(stroke));
        let w = branch_w[k];
        let b_box = BoxF {
            x: label.x - w / 2.0,
            y: label.y - branch_h / 2.0,
            w,
            h: branch_h,
        };
        p.path(shapes::rounded(b_box, 4.0), Some(color), None);
        p.text(&b.name, label, theme.font_size, ink(color));
    }

    // Lines from each parent; on another lane, across first, then along to the child, except
    // that a merge's other parent runs along its own lane first.
    let at = |i: usize| pt(along[i], cross[graph.commits[i].branch]);
    for (i, c) in graph.commits.iter().enumerate() {
        for (n, &parent) in c.parents.iter().enumerate() {
            let pc = &graph.commits[parent];
            let (from, to) = (at(parent), at(i));
            let (points, color) = if pc.branch == c.branch {
                (vec![from, to], c.branch)
            } else if n > 0 {
                let corner = pt(along[i], cross[pc.branch]);
                (vec![from, corner, to], pc.branch)
            } else {
                let corner = pt(along[parent], cross[c.branch]);
                (vec![from, corner, to], c.branch)
            };
            let stroke = Stroke {
                width: 2.0,
                color: branch_color(theme, color),
                dash: None,
            };
            p.path(rounded_polyline(&points, CORNER), None, Some(stroke));
        }
    }

    // Commits, their ids, and their tags.
    for (i, c) in graph.commits.iter().enumerate() {
        let at = at(i);
        p.commit(c, at, branch_color(theme, c.branch));
        let id_center = if lr {
            Pt::new(at.x, at.y + R + 4.0 + label_h / 2.0)
        } else {
            Pt::new(at.x - R - SPACE - id_w[i] / 2.0, at.y)
        };
        p.label(&c.id, id_center, theme.edge_label_fill, None, theme.text);
        let mut offset = R + SPACE;
        for (t, w) in c.tags.iter().zip(&tag_w[i]) {
            let center = if lr {
                Pt::new(at.x, at.y - offset - label_h / 2.0)
            } else {
                Pt::new(at.x + offset + w / 2.0, at.y)
            };
            p.label(
                t,
                center,
                theme.node_fill,
                Some(theme.node_stroke),
                theme.text,
            );
            offset += if lr { label_h } else { *w } + 4.0;
        }
    }

    let mut elements = p.elements;
    if graph.direction == Direction::BottomTop {
        // Mirror the drawing below the title.
        let axis = top + height - MARGIN;
        for e in &mut elements {
            match &mut e.primitive {
                Primitive::Path { segments, .. } => {
                    for s in segments.iter_mut() {
                        *s = mirror_segment(*s, axis);
                    }
                }
                Primitive::Text { center, .. } => center.y = axis - center.y,
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirrored_arcs_turn_the_other_way() {
        let arc = Segment::Arc {
            radii: Pt::new(2.0, 2.0),
            large: false,
            sweep: true,
            to: Pt::new(1.0, 3.0),
        };
        assert_eq!(
            mirror_segment(arc, 10.0),
            Segment::Arc {
                radii: Pt::new(2.0, 2.0),
                large: false,
                sweep: false,
                to: Pt::new(1.0, 7.0),
            }
        );
        assert_eq!(ink(Color::rgb(0xffffff)), Color::rgb(0x222222));
        assert_eq!(ink(Color::rgb(0x000000)), Color::rgb(0xffffff));
        assert_ne!(
            branch_color(&Theme::LIGHT, 0),
            branch_color(&Theme::DARK, 0)
        );
        assert_eq!(
            branch_color(&Theme::LIGHT, 8),
            branch_color(&Theme::LIGHT, 0)
        );
    }
}
