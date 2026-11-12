use super::*;
use crate::flowchart::{self, Flowchart, Shape};

fn chart(src: &str) -> Flowchart {
    flowchart::parse(src).lower().flowchart
}

fn draw(src: &str) -> Scene {
    flowchart(&chart(src), &Approx, &Theme::LIGHT)
}

fn owned(s: &Scene, owner: Owner) -> Vec<&Primitive> {
    s.elements
        .iter()
        .filter(|e| e.owner == owner)
        .map(|e| &e.primitive)
        .collect()
}

fn texts(s: &Scene) -> Vec<&str> {
    s.elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Text { text, .. } => Some(text.as_str()),
            Primitive::Path { .. } => None,
        })
        .collect()
}

/// Distance from `p` to the closest side of `polygon`.
fn distance_to_outline(p: Pt, polygon: &[Pt]) -> f32 {
    let n = polygon.len();
    (0..n)
        .map(|i| {
            let (a, b) = (polygon[i], polygon[(i + 1) % n]);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let len2 = dx * dx + dy * dy;
            let t = if len2 == 0.0 {
                0.0
            } else {
                (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
            };
            (p.x - a.x - t * dx).hypot(p.y - a.y - t * dy)
        })
        .fold(f32::INFINITY, f32::min)
}

#[test]
fn approx_metrics() {
    assert_eq!(Approx.width("abc", 10.0), 18.0);
    assert_eq!(Approx.width("日本", 10.0), 24.0);
}

#[test]
fn empty_diagram() {
    let s = draw("flowchart\n");
    assert!(s.elements.is_empty());
    assert_eq!((s.width, s.height), (32.0, 32.0));
    assert_eq!(s.background, Theme::LIGHT.background);
    let s = flowchart(&chart("flowchart\n"), &Approx, &Theme::DARK);
    assert_eq!(s.background, Theme::DARK.background);
    assert_eq!(Theme::default(), Theme::LIGHT);
}

#[test]
fn title_is_centered_above() {
    let s = draw("---\ntitle: A long diagram title\n---\nflowchart\nA\n");
    let Some(Primitive::Text { text, center, .. }) = owned(&s, Owner::Diagram).first().copied()
    else {
        panic!("no title")
    };
    assert_eq!(text, "A long diagram title");
    assert_eq!(center.x, s.width / 2.0);
    // The title is wider than the node, which is centered under it.
    let Primitive::Path { segments, .. } = owned(&s, Owner::Node(0))[0] else {
        panic!("no node")
    };
    let Segment::Move(p) = segments[0] else {
        panic!("path starts with a move")
    };
    assert!(p.x > 16.0 && p.y > center.y, "{p:?}");
    assert!(
        draw("---\ntitle: \"  \"\n---\nflowchart\nA\n")
            .elements
            .len()
            == 2
    );
}

/// A scene that grows on one side to hold an edge's end label keeps its title centered.
#[test]
fn title_stays_centered_when_the_scene_grows() {
    let mut c = chart("---\ntitle: T\n---\nflowchart LR\nA --> B\n");
    c.edges[0].end_label = Some(crate::flowchart::Label::plain(
        "a very long label at the head end of the edge",
    ));
    let s = flowchart(&c, &Approx, &Theme::LIGHT);
    let Some(Primitive::Text { center, .. }) = owned(&s, Owner::Diagram).first().copied() else {
        panic!("no title")
    };
    assert_eq!(center.x, s.width / 2.0);
}

/// Every edge's arrow tip lands on its target's outline, for every shape (DISPLA-7).
#[test]
fn edges_end_on_every_outline() {
    for dir in ["TD", "LR", "BT", "RL"] {
        for &shape in Shape::ALL.iter().filter(|&&s| s != Shape::Text) {
            let mut c = chart(&format!(
                "flowchart {dir}\nX --> A\nY --> A\nZ --> A\nA --> W\n"
            ));
            c.nodes[1].shape = shape;
            let s = flowchart(&c, &Approx, &Theme::LIGHT);
            let node = owned(&s, Owner::Node(1));
            // Every stroke counts: a person's head is drawn apart from its body.
            let outlines: Vec<Vec<Pt>> = node
                .iter()
                .filter_map(|p| match p {
                    Primitive::Path { segments, .. } => Some(shapes::flatten(segments)),
                    _ => None,
                })
                .filter(|o| !o.is_empty())
                .collect();
            assert!(!outlines.is_empty(), "{shape:?} draws nothing");
            // An hourglass clips edges to its box: a tip only has to land on one of its sides.
            let boxed = shape == Shape::Hourglass;
            let (x0, y0, x1, y1) = outlines.iter().flatten().fold(
                (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
                |(a, b, c, d), p| (a.min(p.x), b.min(p.y), c.max(p.x), d.max(p.y)),
            );
            for e in 0..3 {
                let Primitive::Path { segments, .. } = owned(&s, Owner::Edge(e))[0] else {
                    panic!("no arrowhead")
                };
                let Segment::Move(tip) = segments[0] else {
                    panic!("arrowhead starts with a move")
                };
                let d = if boxed {
                    let horizontal = matches!(dir, "LR" | "RL");
                    if horizontal {
                        (tip.x - x0).abs().min((tip.x - x1).abs())
                    } else {
                        (tip.y - y0).abs().min((tip.y - y1).abs())
                    }
                } else {
                    outlines
                        .iter()
                        .map(|o| distance_to_outline(tip, o))
                        .fold(f32::INFINITY, f32::min)
                };
                assert!(d < 1.0, "{dir} {shape:?} edge {e}: tip {tip:?} is {d} off");
            }
        }
    }
}

#[test]
fn shapes_without_labels() {
    for shape in [
        Shape::SmallCircle,
        Shape::FilledCircle,
        Shape::FramedCircle,
        Shape::Fork,
    ] {
        let mut c = chart("flowchart\nA[hidden]\n");
        c.nodes[0].shape = shape;
        let s = flowchart(&c, &Approx, &Theme::LIGHT);
        assert!(texts(&s).is_empty(), "{shape:?}");
    }
    let mut c = chart("flowchart\nA[shown]\n");
    c.nodes[0].shape = Shape::Text;
    let s = flowchart(&c, &Approx, &Theme::LIGHT);
    assert_eq!(texts(&s), ["shown"]);
}

#[test]
fn arrowheads() {
    let s = draw("flowchart LR\nA <--> B\nC o--o D\nE x--x F\nG --- H\n");
    let count = |e: usize, filled: bool| {
        owned(&s, Owner::Edge(e))
            .iter()
            .filter(|p| matches!(p, Primitive::Path { fill: Some(_), .. }) == filled)
            .count()
    };
    // Two filled heads plus the line.
    assert_eq!((count(0, true), count(0, false)), (2, 1));
    assert_eq!((count(1, true), count(1, false)), (2, 1));
    // Two stroked arms per cross plus the line.
    assert_eq!((count(2, true), count(2, false)), (0, 5));
    assert_eq!((count(3, true), count(3, false)), (0, 1));
}

#[test]
fn stroke_kinds() {
    let s = draw("flowchart LR\nA --> B\nC ==> D\nE -.-> F\nG ~~~ H\n");
    let line = |e: usize| {
        owned(&s, Owner::Edge(e)).into_iter().find_map(|p| match p {
            Primitive::Path {
                stroke: Some(st), ..
            } => Some(*st),
            _ => None,
        })
    };
    assert_eq!(line(0).map(|s| (s.width, s.dash)), Some((1.5, None)));
    assert_eq!(line(1).map(|s| (s.width, s.dash)), Some((3.5, None)));
    assert_eq!(
        line(2).map(|s| (s.width, s.dash)),
        Some((2.0, Some((3.0, 3.0))))
    );
    assert!(owned(&s, Owner::Edge(3)).is_empty());
}

#[test]
fn edge_labels() {
    let s = draw("flowchart LR\nA -->|two<br>lines| B\nC ~~~|hidden| D\n");
    let t = texts(&s);
    assert!(t.contains(&"two") && t.contains(&"lines"), "{t:?}");
    assert!(!t.contains(&"hidden"));
    let fills: Vec<_> = owned(&s, Owner::Edge(0))
        .into_iter()
        .filter_map(|p| match p {
            Primitive::Path {
                fill, stroke: None, ..
            } => *fill,
            _ => None,
        })
        .collect();
    assert!(fills.contains(&Theme::LIGHT.edge_label_fill));
}

#[test]
fn styles_apply() {
    let s = draw(
        "flowchart LR\n\
         classDef default fill:#eee\n\
         classDef hot stroke:#f00\n\
         A:::hot -->|go| B\n\
         style B fill:#0f0,stroke-width:3px,color:#00f,stroke-dasharray:4 2\n\
         linkStyle default stroke:#123456\n\
         linkStyle 0 color:#654321,stroke-width:4px\n\
         subgraph S [Box]\nC\nend\n\
         style S fill:#abcdef,color:#fedcba\n",
    );
    let body = |n: usize| match owned(&s, Owner::Node(n))[0] {
        Primitive::Path { fill, stroke, .. } => (*fill, *stroke),
        p => panic!("{p:?}"),
    };
    let (fill, stroke) = body(0);
    assert_eq!(fill, parse_color("#eee"));
    assert_eq!(stroke.map(|s| s.color), parse_color("#f00"));
    let (fill, stroke) = body(1);
    assert_eq!(fill, parse_color("#0f0"));
    let stroke = stroke.unwrap();
    assert_eq!((stroke.width, stroke.dash), (3.0, Some((4.0, 2.0))));
    assert_eq!(stroke.color, Theme::LIGHT.node_stroke);
    let color_of = |owner: Owner, wanted: &str| {
        owned(&s, owner).into_iter().find_map(|p| match p {
            Primitive::Text { text, color, .. } if text == wanted => Some(*color),
            _ => None,
        })
    };
    assert_eq!(color_of(Owner::Node(1), "B"), parse_color("#00f"));
    assert_eq!(color_of(Owner::Edge(0), "go"), parse_color("#654321"));
    assert_eq!(color_of(Owner::Cluster(0), "Box"), parse_color("#fedcba"));
    let edge_line = owned(&s, Owner::Edge(0)).into_iter().find_map(|p| match p {
        Primitive::Path {
            fill: None,
            stroke: Some(st),
            ..
        } => Some(*st),
        _ => None,
    });
    assert_eq!(
        edge_line.map(|s| (s.color, s.width)),
        Some((parse_color("#123456").unwrap(), 4.0))
    );
    let Primitive::Path { fill, .. } = owned(&s, Owner::Cluster(0))[0] else {
        panic!("no cluster body")
    };
    assert_eq!(*fill, parse_color("#abcdef"));
}

/// Without `color`, labels stay readable on a styled fill, in either theme.
#[test]
fn label_colors_follow_the_fill() {
    let src = "flowchart LR\n\
               A[light]:::light --> B[dark]:::dark --> C[plain] --> D[set]:::set --> E[clear]\n\
               classDef light fill:#e8f1fb\n\
               classDef dark fill:#1f3a5f\n\
               classDef set fill:#e8f1fb,color:#ff0000\n\
               style E fill:transparent\n\
               subgraph S [Group]\nF\nend\n\
               style S fill:#fdf2e3\n";
    let color_of = |theme: &Theme, wanted: &str| {
        let s = flowchart(&chart(src), &Approx, theme);
        s.elements
            .iter()
            .find_map(|e| match &e.primitive {
                Primitive::Text { text, color, .. } if text == wanted => Some(*color),
                _ => None,
            })
            .unwrap()
    };
    let (light, dark) = (&Theme::LIGHT, &Theme::DARK);
    assert_eq!(color_of(dark, "light"), light.text);
    assert_eq!(color_of(dark, "Group"), light.text);
    assert_eq!(color_of(light, "dark"), dark.text);
    // The theme's text stays where it reads.
    assert_eq!(color_of(light, "light"), light.text);
    assert_eq!(color_of(dark, "dark"), dark.text);
    assert_eq!(color_of(dark, "plain"), dark.text);
    assert_eq!(color_of(dark, "clear"), dark.text);
    assert_eq!(color_of(dark, "set"), parse_color("#ff0000").unwrap());
}

/// Nodes stacked in a column connect with straight lines, whatever their widths.
#[cfg(feature = "png")]
#[test]
fn chains_draw_straight() {
    let src = "flowchart TD\n\
               rect[Rectangle] --> round(Rounded) --> stadium([Stadium]) --> sub[[Subroutine]]\n\
               cyl[(Cylinder)] --> circle((Circle)) --> dbl(((Double circle))) --> odd>Asymmetric]\n\
               diamond{Diamond} --> hex{{Hexagon}} --> leanR[/Lean right/] --> leanL[\\Lean left\\]\n\
               trap[/Trapezoid\\] --> inv[\\Inverted trapezoid/] --> ellipse(-Ellipse-) --> bare\n";
    for metrics in [&Approx as &dyn TextMetrics, &png::FontMetrics] {
        let s = flowchart(&chart(src), &Dyn(metrics), &Theme::LIGHT);
        for e in 0..12 {
            let line = owned(&s, Owner::Edge(e))
                .into_iter()
                .find_map(|p| match p {
                    Primitive::Path {
                        segments,
                        fill: None,
                        ..
                    } => Some(segments),
                    _ => None,
                })
                .unwrap();
            assert_eq!(line.len(), 2, "edge {e}: {line:?}");
        }
    }
}

/// Curves and corners finish before the arrowhead, so the head sits on a straight run.
#[cfg(feature = "png")]
#[test]
fn arrowheads_sit_on_straight_runs() {
    let sources = [
        "flowchart LR\n\
         cart[Cart] --> placed[Placed]\n\
         placed ==>|paid| paid[Paid]\n\
         placed -. timeout .-> cancelled[Cancelled]\n\
         paid --> packed[Packed] --> shipped[Shipped]\n\
         shipped ==> delivered([Delivered])\n\
         shipped --x lost[Lost in transit]\n\
         delivered <-->|exchange| paid\n\
         delivered --o review[Review]\n",
        "flowchart TB\nA -- text --> B -- text2 --> C\na --> b & c--> d\nA & B--> C & D\nX[x] & Y(y) --> Z{z}\n",
        "flowchart TD\nA[Start] --> B{Is it?}\nB -->|Yes| C[OK]\nC --> D[Rethink]\nD --> B\nB ---->|No| E[End]\n",
    ];
    for src in sources {
        for metrics in [&Approx as &dyn TextMetrics, &png::FontMetrics] {
            let c = chart(src);
            let s = flowchart(&c, &Dyn(metrics), &Theme::LIGHT);
            for (i, e) in c.edges.iter().enumerate() {
                if e.end != Some(flowchart::Arrow::Point) {
                    continue;
                }
                let prims = owned(&s, Owner::Edge(i));
                let head = prims.iter().find_map(|p| match p {
                    Primitive::Path {
                        segments,
                        fill: Some(_),
                        ..
                    } => Some(segments),
                    _ => None,
                });
                let line = prims.iter().find_map(|p| match p {
                    Primitive::Path {
                        segments,
                        fill: None,
                        ..
                    } => Some(segments),
                    _ => None,
                });
                let (Some(head), Some(line)) = (head, line) else {
                    continue;
                };
                // The tip is the head's first point; its base is the line's end.
                let Segment::Move(tip) = head[0] else {
                    panic!("head starts with a move")
                };
                let curve_end = line.iter().rev().find_map(|s| match *s {
                    Segment::Cubic { to, .. } | Segment::Arc { to, .. } => Some(to),
                    _ => None,
                });
                let Some(curve_end) = curve_end else { continue };
                let reach = (tip.x - curve_end.x).hypot(tip.y - curve_end.y);
                let len = if e.stroke == flowchart::Stroke::Thick {
                    13.5
                } else {
                    9.0
                };
                assert!(
                    reach >= len + 3.0,
                    "{src}\nedge {i}: curve ends {reach} from the tip: {line:?}"
                );
            }
        }
    }
}

/// `TextMetrics` through a trait object.
#[cfg(feature = "png")]
struct Dyn<'a>(&'a dyn TextMetrics);

#[cfg(feature = "png")]
impl TextMetrics for Dyn<'_> {
    fn width(&self, text: &str, size: f32) -> f32 {
        self.0.width(text, size)
    }
}

#[cfg(feature = "png")]
fn runs(s: &Scene, owner: Owner) -> Vec<(String, Pt, TextStyle)> {
    owned(s, owner)
        .into_iter()
        .filter_map(|p| match p {
            Primitive::Text {
                text,
                center,
                style,
                ..
            } => Some((text.clone(), *center, *style)),
            Primitive::Path { .. } => None,
        })
        .collect()
}

/// Markdown emphasis draws as bold and italic runs, left to right on one line.
#[cfg(feature = "png")]
#[test]
fn markdown_runs() {
    let s = flowchart(
        &chart("flowchart\nA[\"`a **bold** and _it_ end`\"]\n"),
        &png::FontMetrics,
        &Theme::LIGHT,
    );
    let r = runs(&s, Owner::Node(0));
    let texts: Vec<(&str, bool, bool)> = r
        .iter()
        .map(|(t, _, st)| (t.as_str(), st.bold, st.italic))
        .collect();
    assert_eq!(
        texts,
        [
            ("a", false, false),
            ("bold", true, false),
            ("and", false, false),
            ("it", false, true),
            ("end", false, false)
        ]
    );
    assert!(
        r.windows(2)
            .all(|w| w[0].1.x < w[1].1.x && w[0].1.y == w[1].1.y)
    );
    // The runs are centered as a line: the outer gaps to the node's middle match.
    let m = &png::FontMetrics;
    let left = r[0].1.x - m.width("a", 14.0) / 2.0;
    let right = r[4].1.x + m.width("end", 14.0) / 2.0;
    let Primitive::Path { segments, .. } = owned(&s, Owner::Node(0))[0] else {
        panic!("no body")
    };
    let xs: Vec<f32> = shapes::flatten(segments).iter().map(|p| p.x).collect();
    let mid = (xs.iter().copied().fold(f32::MAX, f32::min)
        + xs.iter().copied().fold(f32::MIN, f32::max))
        / 2.0;
    assert!(
        ((left + right) / 2.0 - mid).abs() < 0.5,
        "{left} {right} {mid}"
    );
}

/// Long labels wrap at spaces near 200px; diamonds wrap to stay compact.
#[cfg(feature = "png")]
#[test]
fn long_labels_wrap() {
    let long = "a label long enough that it has to wrap onto more lines than one";
    let s = flowchart(
        &chart(&format!(
            "flowchart\nA[{long}]\nB -->|{long}| C\nD{{Coverage at least 98%?}}\nE[short]\nF{{Is it ok?}}\n"
        )),
        &png::FontMetrics,
        &Theme::LIGHT,
    );
    let lines = |owner| {
        let mut ys: Vec<i32> = runs(&s, owner).iter().map(|r| r.1.y as i32).collect();
        ys.dedup();
        ys.len()
    };
    let words = |owner| -> String {
        runs(&s, owner)
            .iter()
            .map(|r| r.0.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    };
    assert!(lines(Owner::Node(0)) >= 2, "{:?}", runs(&s, Owner::Node(0)));
    assert_eq!(words(Owner::Node(0)), long);
    assert!(lines(Owner::Edge(0)) >= 2);
    assert_eq!(words(Owner::Edge(0)), long);
    assert!((2..=3).contains(&lines(Owner::Node(3))));
    assert_eq!(words(Owner::Node(3)), "Coverage at least 98%?");
    assert_eq!(lines(Owner::Node(4)), 1);
    // Short decisions stay on one line.
    assert_eq!(lines(Owner::Node(5)), 1);
    for (text, at, _) in runs(&s, Owner::Node(0)) {
        assert!(
            png::FontMetrics.width(&text, 14.0) <= 200.0,
            "{text} at {at:?}"
        );
    }
}

#[test]
fn colors_over() {
    let white = Color::rgb(0xffffff);
    assert_eq!(Color::rgb(0x102030).over(white), Color::rgb(0x102030));
    assert_eq!(Color::rgb(0).with_alpha(0).over(white), white);
    assert_eq!(
        Color::rgb(0).with_alpha(128).over(white),
        Color::rgb(0x7f7f7f)
    );
}

#[test]
fn nested_clusters_paint_outer_first() {
    let s = draw("flowchart TD\nsubgraph inner\nA\nend\nsubgraph outer\ninner\nend\nA --> outer\n");
    let first = s
        .elements
        .iter()
        .position(|e| e.owner == Owner::Cluster(1))
        .unwrap();
    let second = s
        .elements
        .iter()
        .position(|e| e.owner == Owner::Cluster(0))
        .unwrap();
    assert!(first < second);
}

#[test]
fn edges_to_clusters_end_on_their_border() {
    let s = draw("flowchart TD\nX --> S\nsubgraph S\nA\nend\n");
    let Primitive::Path { segments, .. } = owned(&s, Owner::Cluster(0))[0] else {
        panic!("no cluster")
    };
    let border = shapes::flatten(segments);
    let Primitive::Path { segments, .. } = owned(&s, Owner::Edge(0))[0] else {
        panic!("no edge")
    };
    let Segment::Move(tip) = segments[0] else {
        panic!("arrowhead starts with a move")
    };
    assert!(distance_to_outline(tip, &border) < 0.01, "{tip:?}");
}

#[test]
fn self_loops_draw() {
    let s = draw("flowchart TD\nA -->|again| A\n");
    assert_eq!(owned(&s, Owner::Edge(0)).len(), 4);
}

/// The box around every path point and text center.
fn extent(s: &Scene) -> (Pt, Pt) {
    let mut points = Vec::new();
    for e in &s.elements {
        match &e.primitive {
            Primitive::Path { segments, .. } => points.extend(shapes::flatten(segments)),
            Primitive::Text { center, .. } => points.push(*center),
        }
    }
    points.iter().fold(
        (Pt::new(f32::MAX, f32::MAX), Pt::new(f32::MIN, f32::MIN)),
        |(lo, hi), p| {
            (
                Pt::new(lo.x.min(p.x), lo.y.min(p.y)),
                Pt::new(hi.x.max(p.x), hi.y.max(p.y)),
            )
        },
    )
}

/// The scene grows to hold the labels at an edge's ends, which sit beside its route
/// (DISPLA-1).
#[test]
fn end_labels_stay_inside() {
    for dir in ["TD", "LR", "BT", "RL"] {
        let mut c = chart(&format!("flowchart {dir}\nA --> B\n"));
        let plain = flowchart(&c, &Approx, &Theme::LIGHT);
        c.edges[0].start_label = Some(flowchart::Label::plain("a label at the tail"));
        c.edges[0].end_label = Some(flowchart::Label::plain("one\nat\nthe\nhead"));
        let s = flowchart(&c, &Approx, &Theme::LIGHT);
        assert!(texts(&s).contains(&"a label at the tail"), "{dir}");
        assert!(s.width > plain.width || s.height > plain.height, "{dir}");
        let (lo, hi) = extent(&s);
        assert!(lo.x >= 0.0 && lo.y >= 0.0, "{dir}: {lo:?}");
        assert!(hi.x <= s.width && hi.y <= s.height, "{dir}: {hi:?}");
        // The label's whole box is inside, not only its center.
        let half = Approx.width("a label at the tail", Theme::LIGHT.font_size) / 2.0;
        let center = s
            .elements
            .iter()
            .find_map(|e| match &e.primitive {
                Primitive::Text { text, center, .. } if text == "a label at the tail" => {
                    Some(*center)
                }
                _ => None,
            })
            .expect("the tail label is drawn");
        assert!(
            center.x - half >= 0.0 && center.x + half <= s.width,
            "{dir}"
        );
    }
}

/// A line wider than the margin, with the markers that scale with it, stays inside
/// (DISPLA-1), and no line is wider than 32px (DISPLA-6).
#[test]
fn wide_lines_stay_inside() {
    for dir in ["TD", "LR"] {
        let s = draw(&format!(
            "flowchart {dir}\nA <--> B\nlinkStyle 0 stroke-width:99px\n"
        ));
        let mut widest: f32 = 0.0;
        for p in owned(&s, Owner::Edge(0)) {
            let Primitive::Path {
                segments, stroke, ..
            } = p
            else {
                continue;
            };
            // Each path's ink: its points and half its stroke around them.
            let half = stroke.map_or(0.0, |st| st.width / 2.0);
            widest = widest.max(half * 2.0);
            for p in shapes::flatten(segments) {
                assert!(p.x - half >= -0.01 && p.y - half >= -0.01, "{dir}: {p:?}");
                assert!(
                    p.x + half <= s.width + 0.01 && p.y + half <= s.height + 0.01,
                    "{dir}: {p:?} in {}x{}",
                    s.width,
                    s.height
                );
            }
        }
        assert_eq!(widest, 32.0, "{dir}");
        let (lo, hi) = extent(&s);
        assert!(lo.x >= 0.0 && lo.y >= 0.0, "{dir}: {lo:?}");
        assert!(hi.x <= s.width && hi.y <= s.height, "{dir}: {hi:?}");
    }
}

#[test]
fn flatten_arcs() {
    let p = |x, y| Pt::new(x, y);
    let arc = |radii, large, sweep, to| Segment::Arc {
        radii,
        large,
        sweep,
        to,
    };
    // A zero radius or a zero-length arc is a straight step.
    let pts = shapes::flatten(&[
        Segment::Move(p(0.0, 0.0)),
        arc(p(0.0, 5.0), false, true, p(10.0, 0.0)),
        arc(p(5.0, 5.0), false, true, p(10.0, 0.0)),
    ]);
    assert_eq!(pts, [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 0.0)]);
    // Radii too small to reach are scaled up, giving a half circle through (5, -5) when
    // sweeping clockwise from (0, 0) to (10, 0).
    let pts = shapes::flatten(&[
        Segment::Move(p(0.0, 0.0)),
        arc(p(1.0, 1.0), false, true, p(10.0, 0.0)),
    ]);
    assert!(
        pts.iter()
            .any(|q| (q.x - 5.0).abs() < 0.01 && (q.y + 5.0).abs() < 0.01)
    );
    assert_eq!(*pts.last().unwrap(), p(10.0, 0.0));
    // A half circle is 16 steps (after the start point) even when the sweep rounds a little
    // over pi.
    assert_eq!(pts.len(), 1 + 16);
    // Counterclockwise goes the other way.
    let pts = shapes::flatten(&[
        Segment::Move(p(0.0, 0.0)),
        arc(p(5.0, 5.0), false, false, p(10.0, 0.0)),
    ]);
    assert!(pts.iter().all(|q| q.y >= -0.01));
    // A large arc covers more than half the circle.
    let pts = shapes::flatten(&[
        Segment::Move(p(0.0, 0.0)),
        arc(p(10.0, 10.0), true, true, p(10.0, 0.0)),
    ]);
    assert!(pts.iter().any(|q| q.y < -15.0));
    // Cubic curves are sampled through their end.
    let pts = shapes::flatten(&[
        Segment::Move(p(0.0, 0.0)),
        Segment::Cubic {
            c1: p(0.0, 10.0),
            c2: p(10.0, 10.0),
            to: p(10.0, 0.0),
        },
        Segment::Close,
    ]);
    assert_eq!(pts.len(), 9);
    assert_eq!(pts[4], p(5.0, 7.5));
}

#[test]
fn ray_hits() {
    let square = [
        Pt::new(0.0, 0.0),
        Pt::new(10.0, 0.0),
        Pt::new(10.0, 10.0),
        Pt::new(0.0, 10.0),
    ];
    let down = Pt::new(0.0, 1.0);
    assert_eq!(
        shapes::ray_hit(&square, Pt::new(5.0, -5.0), down, 0.0),
        Some(Pt::new(5.0, 0.0))
    );
    assert_eq!(
        shapes::ray_hit(&square, Pt::new(20.0, -5.0), down, 0.0),
        None
    );
    assert_eq!(
        shapes::ray_hit(&square, Pt::new(5.0, -5.0), Pt::new(1.0, 0.0), 0.0),
        None
    );
}

#[test]
fn svg_details() {
    let scene = Scene {
        width: 10.5,
        height: -0.001,
        background: Color::rgb(0x102030).with_alpha(128),
        elements: vec![
            Element {
                owner: Owner::Diagram,
                primitive: Primitive::Path {
                    segments: vec![
                        Segment::Move(Pt::new(1.0, 2.0)),
                        Segment::Cubic {
                            c1: Pt::new(1.0, 3.0),
                            c2: Pt::new(2.0, 3.0),
                            to: Pt::new(2.0, 2.0),
                        },
                        Segment::Arc {
                            radii: Pt::new(1.0, 1.0),
                            large: true,
                            sweep: false,
                            to: Pt::new(1.0, 2.0),
                        },
                        Segment::Close,
                    ],
                    fill: None,
                    stroke: Some(Stroke {
                        width: 1.25,
                        color: Color::rgb(0xff0000),
                        dash: Some((2.0, 1.0)),
                    }),
                },
            },
            Element {
                owner: Owner::Diagram,
                primitive: Primitive::Text {
                    text: "a<b>&\"c\"".into(),
                    center: Pt::new(0.333, 4.0),
                    size: 12.0,
                    color: Color::rgb(0),
                    style: TextStyle::default(),
                },
            },
            Element {
                owner: Owner::Diagram,
                primitive: Primitive::Text {
                    text: "b".into(),
                    center: Pt::new(1.0, 4.0),
                    size: 12.0,
                    color: Color::rgb(0),
                    style: TextStyle {
                        bold: true,
                        italic: true,
                    },
                },
            },
        ],
    };
    assert_eq!(
        svg::write(&scene),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10.5\" height=\"0\" \
         viewBox=\"0 0 10.5 0\" font-family=\"trebuchet ms, verdana, arial, sans-serif\">\n\
         <rect width=\"100%\" height=\"100%\" fill=\"#102030\" fill-opacity=\"0.5\"/>\n\
         <path d=\"M1 2 C1 3 2 3 2 2 A1 1 0 1 0 1 2 Z\" fill=\"none\" stroke=\"#ff0000\" \
         stroke-width=\"1.25\" stroke-dasharray=\"2 1\"/>\n\
         <text x=\"0.33\" y=\"4\" font-size=\"12\" text-anchor=\"middle\" \
         dominant-baseline=\"central\" fill=\"#000000\">a&lt;b&gt;&amp;&quot;c&quot;</text>\n\
         <text x=\"1\" y=\"4\" font-size=\"12\" font-weight=\"bold\" font-style=\"italic\" \
         text-anchor=\"middle\" dominant-baseline=\"central\" fill=\"#000000\">b</text>\n\
         </svg>\n"
    );
}
