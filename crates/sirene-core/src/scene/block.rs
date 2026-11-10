//! Block diagram to scene (MBLOCK-3): the grid places the blocks and groups, and they draw as a
//! flowchart's nodes, subgraphs, and edges do; a block arrow is drawn here.

use super::paint::{self, Px};
use super::shapes::{self, BoxF};
use super::style;
use super::{Element, Owner, Primitive, Scene, Stroke, TextMetrics, Theme};
use crate::block::layout::{self, Sizes};
use crate::block::{Block, Heading};
use crate::flowchart::Measure;
use crate::layout::{Layout, Point, Rect, Route, Size};

/// How far an arrow's head reaches past its shaft, as a share of the shaft's thickness.
const HEAD: f32 = 0.6;

pub fn block(b: &Block, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let px = Px { metrics, theme };
    let chart = &b.chart;
    let node = |i: usize| -> (f64, f64) {
        let s = px.node(&chart.nodes[i]);
        let (w, h) = ((s.w - 1) as f32, (s.h - 1) as f32);
        match &b.arrows[i] {
            Some(dirs) => {
                let (tw, th) = (w - 4.0, h - 4.0);
                let (bw, bh) = (tw + 2.0 * shapes::PAD_X, th + 2.0 * shapes::PAD_Y);
                let (hz, vt) = heads(dirs);
                let (w, h) = if vt > 0 && hz == 0 {
                    (bw + bh * HEAD, bh * (1.0 + HEAD * vt as f32))
                } else {
                    (bw + bh * HEAD * hz.max(1) as f32, bh * (1.0 + HEAD))
                };
                (f64::from(w), f64::from(h))
            }
            None => (f64::from(w), f64::from(h)),
        }
    };
    let title = |g: usize| -> f64 {
        b.groups[g]
            .subgraph
            .map(|s| px.subgraph_title(&chart.subgraphs[s]).h)
            .filter(|&h| h > 0)
            .map_or(0.0, |h| f64::from(h - 1))
    };
    let pad = f64::from(b.padding);
    let placed = layout::layout(
        b,
        &Sizes {
            node: &node,
            title: &title,
            pad_x: pad,
            pad_y: pad,
        },
    );

    let rect = |r: layout::Rect| Rect {
        x: r.x.round() as i32,
        y: r.y.round() as i32,
        w: r.w.round() as i32 + 1,
        h: r.h.round() as i32 + 1,
    };
    let nodes: Vec<Rect> = placed
        .nodes
        .iter()
        .map(|r| r.map(rect).unwrap_or_default())
        .collect();
    let clusters: Vec<Rect> = (0..chart.subgraphs.len())
        .map(|s| {
            b.group_of(s)
                .map_or_else(Rect::default, |g| rect(placed.groups[g]))
        })
        .collect();
    let titles = (0..chart.subgraphs.len())
        .map(|s| {
            let g = b.group_of(s)?;
            let h = title(g);
            (h > 0.0).then(|| {
                let r = clusters[s];
                Rect {
                    x: r.x,
                    y: r.y + (b.padding / 2.0).round() as i32,
                    w: r.w,
                    h: h.round() as i32 + 1,
                }
            })
        })
        .collect();
    let center = |id: &str| -> Option<Point> {
        let r = match chart.nodes.iter().position(|n| n.id == id) {
            Some(i) => nodes[i],
            None => clusters[chart.subgraphs.iter().position(|s| s.id == id)?],
        };
        Some(Point::new(r.x + (r.w - 1) / 2, r.y + (r.h - 1) / 2))
    };
    let edges = chart
        .edges
        .iter()
        .map(|e| {
            let (a, z) = (center(&e.from)?, center(&e.to)?);
            if a == z {
                return None;
            }
            let label = e.label.as_ref().and_then(|l| px.edge_label(l)).map(|s| {
                let mid = Point::new((a.x + z.x) / 2, (a.y + z.y) / 2);
                Rect {
                    x: mid.x - s.w / 2,
                    y: mid.y - s.h / 2,
                    w: s.w,
                    h: s.h,
                }
            });
            Some(Route {
                points: vec![a, z],
                label,
            })
        })
        .collect();
    let layout = Layout {
        size: Size::new(
            placed.width.round() as i32 + 1,
            placed.height.round() as i32 + 1,
        ),
        nodes,
        clusters,
        titles,
        edges,
        notes: Vec::new(),
    };
    let (mut scene, off) = paint::draw(chart, &layout, metrics, theme);

    // Block arrows go under their labels.
    for (i, dirs) in b.arrows.iter().enumerate() {
        let Some(dirs) = dirs else { continue };
        let r = layout.nodes[i];
        let bx = BoxF {
            x: r.x as f32 + off.x,
            y: r.y as f32 + off.y,
            w: (r.w - 1) as f32,
            h: (r.h - 1) as f32,
        };
        let n = &chart.nodes[i];
        let mut classes = vec!["default"];
        classes.extend(n.classes.iter().map(String::as_str));
        let s = style::resolve(&[], &classes, &n.styles, &chart.classes);
        let stroke = Stroke {
            width: s.stroke_width.unwrap_or(1.0),
            color: s.stroke.unwrap_or(theme.node_stroke),
            dash: s.dash.flatten(),
        };
        let fill = s.fill.unwrap_or(theme.node_fill);
        let at = scene
            .elements
            .iter()
            .position(|e| e.owner == Owner::Node(i))
            .unwrap_or(scene.elements.len());
        let shapes: Vec<Element> = arrow(bx, dirs)
            .into_iter()
            .map(|points| Element {
                owner: Owner::Node(i),
                primitive: Primitive::Path {
                    segments: shapes::polygon(&points),
                    fill: Some(fill),
                    stroke: Some(stroke),
                },
            })
            .collect();
        scene.elements.splice(at..at, shapes);
    }
    scene
}

/// How many heads point across and how many up or down.
fn heads(dirs: &[Heading]) -> (usize, usize) {
    let has = |h: Heading| dirs.contains(&h);
    let x = has(Heading::X);
    let y = has(Heading::Y);
    let across = usize::from(x || has(Heading::Left)) + usize::from(x || has(Heading::Right));
    let down = usize::from(y || has(Heading::Up)) + usize::from(y || has(Heading::Down));
    (across, down)
}

/// The outlines of a block arrow filling `b`: one across, one up or down, or both.
fn arrow(b: BoxF, dirs: &[Heading]) -> Vec<Vec<(f32, f32)>> {
    let has = |h: Heading| dirs.contains(&h);
    let (left, right) = (
        has(Heading::X) || has(Heading::Left),
        has(Heading::X) || has(Heading::Right),
    );
    let (up, down) = (
        has(Heading::Y) || has(Heading::Up),
        has(Heading::Y) || has(Heading::Down),
    );
    let mut out = Vec::new();
    if left || right || !(up || down) {
        out.push(horizontal(b, left, right || !left));
    }
    if up || down {
        // Turn the box, draw it across, and turn the points back.
        let t = BoxF {
            x: b.y,
            y: b.x,
            w: b.h,
            h: b.w,
        };
        out.push(
            horizontal(t, up, down)
                .into_iter()
                .map(|(x, y)| (y, x))
                .collect(),
        );
    }
    out
}

/// An arrow across `b` with a head at the left, the right, or both.
fn horizontal(b: BoxF, left: bool, right: bool) -> Vec<(f32, f32)> {
    let (x, y, r, bottom) = (b.x, b.y, b.right(), b.bottom());
    let cy = y + b.h / 2.0;
    // The shaft is the middle of the box; the heads reach its edges.
    let inset = b.h * HEAD / (2.0 * (1.0 + HEAD));
    let head = (b.h * 0.5).min(b.w / 3.0);
    let (s0, s1) = (y + inset, bottom - inset);
    let mut pts = Vec::new();
    if left {
        pts.extend([
            (x + head, s1),
            (x + head, bottom),
            (x, cy),
            (x + head, y),
            (x + head, s0),
        ]);
    } else {
        pts.extend([(x, s1), (x, s0)]);
    }
    if right {
        pts.extend([
            (r - head, s0),
            (r - head, y),
            (r, cy),
            (r - head, bottom),
            (r - head, s1),
        ]);
    } else {
        pts.extend([(r, s0), (r, s1)]);
    }
    pts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_point_where_asked() {
        let b = BoxF {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 40.0,
        };
        let right = arrow(b, &[Heading::Right]);
        assert_eq!(right.len(), 1);
        assert!(right[0].contains(&(100.0, 20.0)));
        assert!(
            !right[0]
                .iter()
                .any(|&(x, _)| x == 0.0 && right[0].contains(&(0.0, 20.0)))
        );
        let both = arrow(b, &[Heading::X]);
        assert!(both[0].contains(&(0.0, 20.0)) && both[0].contains(&(100.0, 20.0)));
        let down = arrow(b, &[Heading::Down]);
        assert!(down[0].contains(&(50.0, 40.0)));
        assert_eq!(arrow(b, &[Heading::Right, Heading::Down]).len(), 2);
        assert_eq!(heads(&[Heading::X, Heading::Up]), (2, 1));
    }
}
