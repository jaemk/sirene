//! Architecture diagram as text (MARCH-4): services as boxes with the icon name after the label,
//! junctions as a dot, groups as boxes titled on the top border, and edges with right-angle
//! turns, placed by the same grid as the drawing.

use super::{Cells, Options, draw};
use crate::architecture::layout::{self, Sizes};
use crate::architecture::{Architecture, Item, Side};
use crate::flowchart::{Label, Measure, Shape};
use crate::layout::{Layout, Point, Rect, Route, Size};

pub fn render(a: &Architecture, options: &Options) -> String {
    // What text shows: a label (or the id) with the icon name, and a dot for a junction.
    let mut chart = a.chart.clone();
    for (n, item) in chart.nodes.iter_mut().zip(&a.items) {
        match item {
            Item::Service { icon } => {
                let name = if n.label.text.trim().is_empty() {
                    n.id.clone()
                } else {
                    n.label.text.clone()
                };
                let text = match icon {
                    Some(i) => format!("{name} ({i})"),
                    None => name,
                };
                n.label = Label {
                    text,
                    ..n.label.clone()
                };
                n.shape = Shape::Rect;
            }
            Item::Junction => {
                n.shape = Shape::SmallCircle;
                n.label = Label::default();
            }
        }
    }
    for (s, icon) in chart.subgraphs.iter_mut().zip(&a.group_icons) {
        if let Some(i) = icon {
            let t = s.title.text.trim();
            let text = if t.is_empty() {
                format!("({i})")
            } else {
                format!("{t} ({i})")
            };
            s.title = Label {
                text,
                ..s.title.clone()
            };
        }
    }
    let node = |i: usize| {
        let s = Cells.node(&chart.nodes[i]);
        ((f64::from(s.w), f64::from(s.h)), (0.0, 0.0))
    };
    let title = |g: usize| {
        let s = Cells.subgraph_title(&chart.subgraphs[g]);
        (f64::from(s.w + 2), 0.0)
    };
    let placed = layout::layout(
        a,
        &Sizes {
            node: &node,
            title: &title,
            gap_x: 6.0,
            gap_y: 3.0,
            pad: 2.0,
        },
    );
    let rect = |r: layout::Rect| Rect {
        x: r.x.round() as i32,
        y: r.y.round() as i32,
        w: r.w.round() as i32,
        h: r.h.round() as i32,
    };
    let nodes: Vec<Rect> = placed.nodes.iter().map(|&r| rect(r)).collect();
    let clusters: Vec<Rect> = placed.groups.iter().map(|&r| rect(r)).collect();
    let titles = (0..chart.subgraphs.len())
        .map(|g| {
            let s = Cells.subgraph_title(&chart.subgraphs[g]);
            (s.w > 0).then(|| Rect {
                x: clusters[g].x + 1,
                y: clusters[g].y,
                w: s.w,
                h: 1,
            })
        })
        .collect();
    let edges = chart
        .edges
        .iter()
        .zip(&placed.edges)
        .zip(&a.ports)
        .map(|((e, (points, mid)), port)| {
            let mut pts: Vec<Point> = points
                .iter()
                // Centers of odd-sized boxes fall on half cells; take the middle cell.
                .map(|&(x, y)| Point::new(x.floor() as i32, y.floor() as i32))
                .collect();
            // A box's right and bottom sides are its last column and row.
            let pull = |p: &mut Point, side: Side| match side {
                Side::Right => p.x -= 1,
                Side::Bottom => p.y -= 1,
                _ => {}
            };
            let n = pts.len();
            pull(&mut pts[0], port.from_side);
            pull(&mut pts[n - 1], port.to_side);
            // Keep the runs next to the ends straight.
            if n > 2 {
                if port.from_side.is_x() {
                    pts[1].y = pts[0].y;
                } else {
                    pts[1].x = pts[0].x;
                }
                if port.to_side.is_x() {
                    pts[n - 2].y = pts[n - 1].y;
                } else {
                    pts[n - 2].x = pts[n - 1].x;
                }
            } else if port.from_side.is_x() {
                pts[1].y = pts[0].y;
            } else {
                pts[1].x = pts[0].x;
            }
            let label = e.label.as_ref().and_then(|l| Cells.edge_label(l)).map(|s| {
                let m = Point::new(mid.0.floor() as i32, mid.1.floor() as i32);
                Rect {
                    x: m.x - s.w / 2,
                    y: m.y - s.h / 2,
                    w: s.w,
                    h: s.h,
                }
            });
            Some(Route { points: pts, label })
        })
        .collect();
    let layout = Layout {
        size: Size::new(placed.width.round() as i32, placed.height.round() as i32),
        nodes,
        clusters,
        titles,
        edges,
        notes: Vec::new(),
    };
    draw(&chart, &layout, options)
}

#[cfg(test)]
mod tests {
    use crate::diagram::{self, Diagram};

    fn ascii(src: &str) -> String {
        let Diagram::Architecture(a) = diagram::lower(src).diagram else {
            panic!("not an architecture diagram")
        };
        super::render(&a, &super::Options::default())
    }

    #[test]
    fn services_side_by_side() {
        let out = ascii("architecture-beta\nservice a(server)[A]\nservice b[B]\na:R --> L:b\n");
        assert_eq!(
            out,
            "┌────────────┐      ┌───┐\n│ A (server) ├─────►│ B │\n└────────────┘      └───┘\n"
        );
    }
}
