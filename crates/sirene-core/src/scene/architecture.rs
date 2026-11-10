//! Architecture diagram to scene (MARCH-3): services as icons with their labels below, groups as
//! dashed boxes titled at the top left, junctions as points, and edges with right-angle turns.

use super::paint::{self, Px};
use super::shapes::{self, BoxF};
use super::{
    Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle, Theme,
};
use crate::architecture::layout::{self, Sizes};
use crate::architecture::{Architecture, Item};
use crate::flowchart::{Label, Measure, Shape};
use crate::layout::{Layout, Point, Rect, Route, Size};

/// A service's icon is this many pixels square.
const ICON: f32 = 80.0;
/// A group's icon beside its title.
const SMALL: f32 = 20.0;
/// Mermaid's icon color.
const BLUE: Color = Color::rgb(0x087ebf);
const WHITE: Color = Color::rgb(0xffffff);

pub fn architecture(a: &Architecture, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let px = Px { metrics, theme };
    let label_size = |l: &Label| {
        px.edge_label(l)
            .map_or((0.0, 0.0), |s| (f64::from(s.w), f64::from(s.h)))
    };
    let node = |i: usize| match &a.items[i] {
        Item::Service { .. } => (
            (f64::from(ICON), f64::from(ICON)),
            label_size(&a.chart.nodes[i].label),
        ),
        Item::Junction => ((2.0, 2.0), (0.0, 0.0)),
    };
    let title = |g: usize| {
        let s = px.subgraph_title(&a.chart.subgraphs[g]);
        let icon = if a.group_icons[g].is_some() {
            f64::from(SMALL) + 6.0
        } else {
            0.0
        };
        let h = f64::from(s.h).max(if icon > 0.0 {
            f64::from(SMALL) + 8.0
        } else {
            0.0
        });
        (f64::from(s.w) + icon, h)
    };
    let placed = layout::layout(
        a,
        &Sizes {
            node: &node,
            title: &title,
            gap_x: 60.0,
            gap_y: 60.0,
            pad: 24.0,
        },
    );
    let pt = |(x, y): (f64, f64)| Point::new(x.round() as i32, y.round() as i32);
    let rect = |r: layout::Rect| Rect {
        x: r.x.round() as i32,
        y: r.y.round() as i32,
        w: r.w.round() as i32 + 1,
        h: r.h.round() as i32 + 1,
    };
    // A node's box takes in its label, so an edge from below starts under the label.
    let nodes: Vec<Rect> = placed
        .nodes
        .iter()
        .enumerate()
        .map(|(i, &r)| {
            let (_, (_, lh)) = node(i);
            rect(layout::Rect { h: r.h + lh, ..r })
        })
        .collect();
    let clusters: Vec<Rect> = placed.groups.iter().map(|&r| rect(r)).collect();
    let titles = (0..a.chart.subgraphs.len())
        .map(|g| {
            let s = px.subgraph_title(&a.chart.subgraphs[g]);
            (s.h > 0).then(|| {
                let r = clusters[g];
                let icon = if a.group_icons[g].is_some() {
                    SMALL as i32 + 10
                } else {
                    4
                };
                Rect {
                    x: r.x + icon,
                    y: r.y + 4,
                    w: s.w,
                    h: s.h,
                }
            })
        })
        .collect();
    let edges = a
        .chart
        .edges
        .iter()
        .zip(&placed.edges)
        .map(|(e, (points, mid))| {
            let label = e.label.as_ref().and_then(|l| px.edge_label(l)).map(|s| {
                let m = pt(*mid);
                Rect {
                    x: m.x - s.w / 2,
                    y: m.y - s.h / 2,
                    w: s.w,
                    h: s.h,
                }
            });
            Some(Route {
                points: points.iter().map(|&p| pt(p)).collect(),
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
    // Nodes paint nothing of their own: icons and labels are added below.
    let mut chart = a.chart.clone();
    for n in &mut chart.nodes {
        n.shape = Shape::Text;
        n.label = Label::default();
    }
    for s in &mut chart.subgraphs {
        s.region = true;
    }
    let (mut scene, off) = paint::draw(&chart, &layout, metrics, theme);

    for (i, item) in a.items.iter().enumerate() {
        let Item::Service { icon } = item else {
            continue;
        };
        let r = placed.nodes[i];
        let b = BoxF {
            x: r.x.round() as f32 + off.x,
            y: r.y.round() as f32 + off.y,
            w: r.w.round() as f32,
            h: r.h.round() as f32,
        };
        let owner = Owner::Node(i);
        for primitive in icon_paths(icon.as_deref(), b) {
            scene.elements.push(Element { owner, primitive });
        }
        let label = &a.chart.nodes[i].label;
        for (k, line) in label.lines().iter().enumerate() {
            scene.elements.push(Element {
                owner,
                primitive: Primitive::Text {
                    text: line.clone(),
                    center: Pt::new(
                        b.x + b.w / 2.0,
                        b.bottom() + 4.0 + theme.line_height * (k as f32 + 0.5),
                    ),
                    size: theme.font_size,
                    color: theme.text,
                    style: TextStyle::default(),
                },
            });
        }
    }
    for (g, icon) in a.group_icons.iter().enumerate() {
        let Some(icon) = icon else { continue };
        let r = layout.clusters[g];
        let b = BoxF {
            x: r.x as f32 + off.x + 6.0,
            y: r.y as f32 + off.y + 6.0,
            w: SMALL,
            h: SMALL,
        };
        for primitive in icon_paths(Some(icon), b) {
            scene.elements.push(Element {
                owner: Owner::Cluster(g),
                primitive,
            });
        }
    }
    scene
}

fn line(width: f32) -> Option<Stroke> {
    Some(Stroke {
        width,
        color: WHITE,
        dash: None,
    })
}

/// A built-in icon (`cloud`, `database`, `disk`, `internet`, `server`) in white on blue filling
/// `b`; any other name is a question mark, and no icon is a plain box.
fn icon_paths(icon: Option<&str>, b: BoxF) -> Vec<Primitive> {
    let s = b.w / 80.0;
    let p = |x: f32, y: f32| Pt::new(b.x + x * s, b.y + y * s);
    let w = 2.0 * s.max(0.5);
    let mut out = vec![Primitive::Path {
        segments: shapes::rounded(b, 4.0 * s),
        fill: Some(BLUE),
        stroke: None,
    }];
    let path = |segments: Vec<Segment>| Primitive::Path {
        segments,
        fill: None,
        stroke: line(w),
    };
    let arc = |rx: f32, ry: f32, sweep: bool, large: bool, to: Pt| Segment::Arc {
        radii: Pt::new(rx * s, ry * s),
        large,
        sweep,
        to,
    };
    match icon {
        None => {}
        Some("server") => {
            for k in 0..3 {
                let y = 16.0 + k as f32 * 17.0;
                out.push(path(shapes::rounded(
                    BoxF {
                        x: b.x + 18.0 * s,
                        y: b.y + y * s,
                        w: 44.0 * s,
                        h: 13.0 * s,
                    },
                    2.0 * s,
                )));
                out.push(path(vec![
                    Segment::Move(p(26.0, y + 6.5)),
                    Segment::Line(p(30.0, y + 6.5)),
                ]));
            }
        }
        Some("database") => {
            out.push(path(shapes::ellipse(p(40.0, 22.0), 20.0 * s, 7.0 * s)));
            out.push(path(vec![
                Segment::Move(p(20.0, 22.0)),
                Segment::Line(p(20.0, 58.0)),
                arc(20.0, 7.0, false, false, p(60.0, 58.0)),
                Segment::Line(p(60.0, 22.0)),
            ]));
            for y in [34.0, 46.0] {
                out.push(path(vec![
                    Segment::Move(p(20.0, y)),
                    arc(20.0, 7.0, false, false, p(60.0, y)),
                ]));
            }
        }
        Some("disk") => {
            out.push(path(shapes::rounded(
                BoxF {
                    x: b.x + 20.0 * s,
                    y: b.y + 14.0 * s,
                    w: 40.0 * s,
                    h: 52.0 * s,
                },
                3.0 * s,
            )));
            out.push(path(shapes::ellipse(p(40.0, 36.0), 13.0 * s, 13.0 * s)));
            out.push(path(shapes::ellipse(p(40.0, 36.0), 3.0 * s, 3.0 * s)));
            out.push(path(vec![
                Segment::Move(p(30.0, 58.0)),
                Segment::Line(p(50.0, 58.0)),
            ]));
        }
        Some("cloud") => {
            out.push(path(vec![
                Segment::Move(p(24.0, 54.0)),
                arc(10.0, 10.0, true, false, p(26.0, 36.0)),
                arc(14.0, 14.0, true, false, p(50.0, 30.0)),
                arc(11.0, 11.0, true, false, p(58.0, 54.0)),
                Segment::Close,
            ]));
        }
        Some("internet") => {
            let c = p(40.0, 40.0);
            out.push(path(shapes::ellipse(c, 22.0 * s, 22.0 * s)));
            out.push(path(shapes::ellipse(c, 9.0 * s, 22.0 * s)));
            for y in [32.0, 48.0] {
                out.push(path(vec![
                    Segment::Move(p(20.0, y)),
                    Segment::Line(p(60.0, y)),
                ]));
            }
            out.push(path(vec![
                Segment::Move(p(40.0, 18.0)),
                Segment::Line(p(40.0, 62.0)),
            ]));
        }
        Some(_) => {
            out.push(Primitive::Text {
                text: "?".to_owned(),
                center: p(40.0, 40.0),
                size: 44.0 * s,
                color: WHITE,
                style: TextStyle {
                    bold: true,
                    italic: false,
                },
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_have_glyphs() {
        let b = BoxF {
            x: 0.0,
            y: 0.0,
            w: 80.0,
            h: 80.0,
        };
        assert_eq!(icon_paths(None, b).len(), 1);
        for name in [
            "server",
            "database",
            "disk",
            "cloud",
            "internet",
            "logos:aws-s3",
        ] {
            assert!(icon_paths(Some(name), b).len() > 1, "{name}");
        }
    }
}
