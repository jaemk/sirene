//! Mindmap to scene (MMIND-3): the root in the middle and its branches out to either side.

use std::f32::consts::{PI, SQRT_2};

use super::Theme;
use super::git::{branch_color, ink};
use super::paint::{Painter, Px};
use super::shapes::{self, BoxF};
use super::{Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle};
use crate::flowchart::Span;
use crate::mindmap::{Mindmap, Shape};

const MARGIN: f32 = 16.0;
/// Padding between a node's text and its sides.
const PAD_X: f32 = 12.0;
const PAD_Y: f32 = 8.0;
/// Text wider than this wraps, as Mermaid's `maxNodeWidth`.
const WRAP: f32 = 200.0;
/// Space between a node and its children.
const H_GAP: f32 = 40.0;
/// Space between siblings' subtrees.
const V_GAP: f32 = 12.0;
/// How far a cloud's bumps reach past its text box.
const BUMP: f32 = 8.0;
/// The length of a cloud bump along its side.
const BUMP_LEN: f32 = 24.0;
/// Corner radius of the default and rounded shapes.
const CORNER: f32 = 5.0;
const ROUND: f32 = 12.0;

/// The size of a node of `shape` around text `tw` by `th`.
fn size(shape: Shape, tw: f32, th: f32) -> (f32, f32) {
    let (w0, h0) = (tw + 2.0 * PAD_X, th + 2.0 * PAD_Y);
    match shape {
        Shape::Default | Shape::Square => (w0, h0),
        Shape::Rounded => (w0 + ROUND, h0),
        Shape::Hexagon => (w0 + h0 / 2.0, h0),
        Shape::Circle => {
            let d = (tw.hypot(th) + 2.0 * PAD_Y).max(h0);
            (d, d)
        }
        Shape::Cloud => (w0 + 2.0 * BUMP, h0 + 2.0 * BUMP),
        // An ellipse holding the text box, plus room for the spikes' dents.
        Shape::Bang => (
            (tw + PAD_X) * SQRT_2 + 4.0 * BUMP,
            (th + PAD_Y) * SQRT_2 + 4.0 * BUMP,
        ),
    }
}

fn arc(r: f32, sweep: bool, to: Pt) -> Segment {
    Segment::Arc {
        radii: Pt::new(r, r),
        large: false,
        sweep,
        to,
    }
}

/// A cloud: arcs bulging out of the box inset by `BUMP`.
fn cloud(b: BoxF) -> Vec<Segment> {
    let (x0, y0, x1, y1) = (b.x + BUMP, b.y + BUMP, b.right() - BUMP, b.bottom() - BUMP);
    let corners = [
        Pt::new(x0, y0),
        Pt::new(x1, y0),
        Pt::new(x1, y1),
        Pt::new(x0, y1),
    ];
    let mut out = vec![Segment::Move(corners[0])];
    for k in 0..4 {
        let (a, c) = (corners[k], corners[(k + 1) % 4]);
        let len = (c.x - a.x).hypot(c.y - a.y);
        let n = (len / BUMP_LEN).round().max(1.0);
        let half = len / n / 2.0;
        let r = (half * half + BUMP * BUMP) / (2.0 * BUMP);
        for i in 1..=n as usize {
            let t = i as f32 / n;
            out.push(arc(
                r,
                true,
                Pt::new(a.x + (c.x - a.x) * t, a.y + (c.y - a.y) * t),
            ));
        }
    }
    out.push(Segment::Close);
    out
}

/// A bang: points on an ellipse joined by arcs curving in, so each point is a spike.
fn bang(b: BoxF) -> Vec<Segment> {
    // A multiple of 4, so spikes point up, down, and to the sides where edges meet them.
    const SPIKES: usize = 16;
    let c = b.center();
    let (rx, ry) = (b.w / 2.0, b.h / 2.0);
    let at = |k: usize| {
        let t = 2.0 * PI * k as f32 / SPIKES as f32 - PI / 2.0;
        Pt::new(c.x + rx * t.cos(), c.y + ry * t.sin())
    };
    let mut out = vec![Segment::Move(at(0))];
    for k in 1..=SPIKES {
        let (a, p) = (at(k - 1), at(k % SPIKES));
        let chord = (p.x - a.x).hypot(p.y - a.y);
        out.push(arc(chord * 0.7, false, p));
    }
    out.push(Segment::Close);
    out
}

/// The outline of a node of `shape` filling `b`.
fn outline(shape: Shape, b: BoxF) -> Vec<Segment> {
    let (x, y, r, bottom) = (b.x, b.y, b.right(), b.bottom());
    match shape {
        // Rounded on top, square at the bottom where the line runs.
        Shape::Default => vec![
            Segment::Move(Pt::new(x, bottom)),
            Segment::Line(Pt::new(x, y + CORNER)),
            arc(CORNER, true, Pt::new(x + CORNER, y)),
            Segment::Line(Pt::new(r - CORNER, y)),
            arc(CORNER, true, Pt::new(r, y + CORNER)),
            Segment::Line(Pt::new(r, bottom)),
            Segment::Close,
        ],
        Shape::Square => shapes::polygon(&[(x, y), (r, y), (r, bottom), (x, bottom)]),
        Shape::Rounded => shapes::rounded(b, ROUND),
        Shape::Circle => shapes::ellipse(b.center(), b.w / 2.0, b.h / 2.0),
        Shape::Hexagon => {
            let s = b.h / 4.0;
            let cy = b.center().y;
            shapes::polygon(&[
                (x + s, y),
                (r - s, y),
                (r, cy),
                (r - s, bottom),
                (x + s, bottom),
                (x, cy),
            ])
        }
        Shape::Cloud => cloud(b),
        Shape::Bang => bang(b),
    }
}

/// The fill of a node in `section`, the root's when `None`.
fn fill(theme: &Theme, section: Option<usize>) -> Color {
    section.map_or(theme.node_stroke, |s| branch_color(theme, s))
}

/// The width of the edge into a node at `depth`.
fn edge_width(depth: usize) -> f32 {
    (7.0 - 1.5 * depth as f32).max(2.0)
}

/// Draws `mindmap` (MMIND-3).
pub fn mindmap(mindmap: &Mindmap, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let px = Px { metrics, theme };
    let nodes = &mindmap.nodes;
    let lines: Vec<Vec<Vec<Span>>> = nodes
        .iter()
        .map(|n| px.wrap(&n.label.spans(), WRAP))
        .collect();
    let sizes: Vec<(f32, f32)> = nodes
        .iter()
        .zip(&lines)
        .map(|(n, l)| {
            let (tw, th) = px.text_size(l);
            size(n.shape, tw, th)
        })
        .collect();
    // The height of each subtree; children come after their parents.
    let mut heights = vec![0.0f32; nodes.len()];
    for i in (0..nodes.len()).rev() {
        let kids = &nodes[i].children;
        let sum: f32 = kids.iter().map(|&c| heights[c]).sum::<f32>()
            + V_GAP * kids.len().saturating_sub(1) as f32;
        heights[i] = sizes[i].1.max(sum);
    }
    // Each node's box, and whether it sits left of the root.
    let mut boxes = vec![BoxF::default(); nodes.len()];
    let mut left = vec![false; nodes.len()];
    if let Some(root) = nodes.first() {
        let (w, h) = sizes[0];
        boxes[0] = BoxF {
            x: -w / 2.0,
            y: -h / 2.0,
            w,
            h,
        };
        // Branches in order down the right side until it holds half the height, then the rest
        // down the left.
        let total: f32 = root.children.iter().map(|&c| heights[c]).sum();
        let mut right_h = 0.0;
        let mut split = 0;
        for &c in &root.children {
            if split > 0 && right_h + heights[c] / 2.0 > total / 2.0 {
                break;
            }
            right_h += heights[c];
            split += 1;
        }
        let (right, left_kids) = root.children.split_at(split);
        for (group, to_left) in [(right, false), (left_kids, true)] {
            let group_h: f32 = group.iter().map(|&c| heights[c]).sum::<f32>()
                + V_GAP * group.len().saturating_sub(1) as f32;
            let mut top = -group_h / 2.0;
            let x = if to_left {
                boxes[0].x - H_GAP
            } else {
                boxes[0].right() + H_GAP
            };
            for &c in group {
                place(
                    c, x, top, to_left, nodes, &sizes, &heights, &mut boxes, &mut left,
                );
                top += heights[c] + V_GAP;
            }
        }
    }

    let lh = theme.line_height;
    let title = mindmap
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_h = if title.is_some() { lh + 16.0 } else { 0.0 };
    let min_x = boxes.iter().map(|b| b.x).fold(f32::INFINITY, f32::min);
    let min_y = boxes.iter().map(|b| b.y).fold(f32::INFINITY, f32::min);
    let (dx, dy) = if nodes.is_empty() {
        (0.0, 0.0)
    } else {
        (MARGIN - min_x, MARGIN + title_h - min_y)
    };
    for b in &mut boxes {
        b.x += dx;
        b.y += dy;
    }
    let right = boxes.iter().map(|b| b.right()).fold(0.0, f32::max);
    let bottom = boxes
        .iter()
        .map(|b| b.bottom())
        .fold(MARGIN + title_h, f32::max);
    let width = (right + MARGIN)
        .max(title.map_or(0.0, |t| metrics.width(t, theme.font_size * 1.2)) + 2.0 * MARGIN);
    let height = bottom + MARGIN;

    let mut p = Painter {
        theme,
        metrics,
        elements: Vec::new(),
    };
    if let Some(t) = title {
        p.elements.push(Element {
            owner: Owner::Diagram,
            primitive: Primitive::Text {
                text: t.to_owned(),
                center: Pt::new(width / 2.0, MARGIN + lh / 2.0),
                size: theme.font_size * 1.2,
                color: theme.text,
                style: TextStyle::default(),
            },
        });
    }
    // Edges behind the nodes, each as a curve leaving its parent's side, in node order. Every
    // element is owned by the diagram, which is read-only (LANGS-2).
    let owner = Owner::Diagram;
    for (i, n) in nodes.iter().enumerate() {
        let Some(parent) = n.parent else { continue };
        let (pb, cb) = (boxes[parent], boxes[i]);
        let (from, to) = if left[i] {
            (
                Pt::new(pb.x, pb.center().y),
                Pt::new(cb.right(), cb.center().y),
            )
        } else {
            (
                Pt::new(pb.right(), pb.center().y),
                Pt::new(cb.x, cb.center().y),
            )
        };
        let mid = (from.x + to.x) / 2.0;
        let stroke = Stroke {
            width: edge_width(n.depth),
            color: fill(theme, n.section),
            dash: None,
        };
        p.path(
            owner,
            vec![
                Segment::Move(from),
                Segment::Cubic {
                    c1: Pt::new(mid, from.y),
                    c2: Pt::new(mid, to.y),
                    to,
                },
            ],
            None,
            Some(stroke),
        );
    }
    for (i, n) in nodes.iter().enumerate() {
        let b = boxes[i];
        let color = fill(theme, n.section);
        p.path(owner, outline(n.shape, b), Some(color), None);
        if n.shape == Shape::Default {
            // A darker line along the bottom.
            let dark = Color::rgb(0).with_alpha(0x50).over(color);
            p.path(
                owner,
                vec![
                    Segment::Move(Pt::new(b.x, b.bottom() - 1.5)),
                    Segment::Line(Pt::new(b.right(), b.bottom() - 1.5)),
                ],
                None,
                Some(Stroke {
                    width: 3.0,
                    color: dark,
                    dash: None,
                }),
            );
        }
        p.lines(owner, &lines[i], b.center(), ink(color));
    }
    Scene {
        width,
        height,
        background: theme.background,
        elements: p.elements,
    }
}

/// Places node `i`'s subtree in a band of its height from `top`, the node's facing side at `x`.
#[allow(clippy::too_many_arguments)]
fn place(
    i: usize,
    x: f32,
    top: f32,
    to_left: bool,
    nodes: &[crate::mindmap::Node],
    sizes: &[(f32, f32)],
    heights: &[f32],
    boxes: &mut [BoxF],
    left: &mut [bool],
) {
    let (w, h) = sizes[i];
    let b = BoxF {
        x: if to_left { x - w } else { x },
        y: top + (heights[i] - h) / 2.0,
        w,
        h,
    };
    boxes[i] = b;
    left[i] = to_left;
    let kids = &nodes[i].children;
    let kids_h: f32 =
        kids.iter().map(|&c| heights[c]).sum::<f32>() + V_GAP * kids.len().saturating_sub(1) as f32;
    let mut y = top + (heights[i] - kids_h) / 2.0;
    let next = if to_left {
        b.x - H_GAP
    } else {
        b.right() + H_GAP
    };
    for &c in kids {
        place(c, next, y, to_left, nodes, sizes, heights, boxes, left);
        y += heights[c] + V_GAP;
    }
}
