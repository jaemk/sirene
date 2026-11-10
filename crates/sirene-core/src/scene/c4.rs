//! C4 diagram to scene (MC4-3): elements as filled boxes, cylinders, and pipes in Mermaid's C4
//! colors with their type, label, technology, and description; boundaries as dashed boxes
//! headed at the top left; relations as straight arrows between centers, clipped to the ends.

use super::paint::{self, Px};
use super::shapes::BoxF;
use super::style::parse_color;
use super::{
    Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle, Theme,
    shapes,
};
use crate::c4::layout::{self, Sizes};
use crate::c4::{Base, Body, C4};
use crate::flowchart::{Label, Measure, Shape, Style};
use crate::layout::{Layout, Point, Rect, Route, Size};

/// An element's width, as in Mermaid.
const WIDTH: f32 = 216.0;
const PAD: f32 = 12.0;
/// The person icon's size.
const ICON: f32 = 40.0;
const SMALL: f32 = 12.0;
const LARGE: f32 = 16.0;
/// Boundary lines and relation lines.
const GREY: Color = Color::rgb(0x444444);

/// Mermaid's fill, border, and text colors for an element.
fn palette(base: Base, external: bool) -> (Color, Color, Color) {
    let white = Color::rgb(0xffffff);
    let black = Color::rgb(0x000000);
    match (base, external) {
        (Base::Person, false) => (Color::rgb(0x08427b), Color::rgb(0x073b6f), white),
        (Base::Person, true) => (Color::rgb(0x686868), Color::rgb(0x8a8a8a), white),
        (Base::System, false) => (Color::rgb(0x1168bd), Color::rgb(0x3c7fc0), white),
        (Base::System, true) => (Color::rgb(0x999999), Color::rgb(0x8a8a8a), white),
        (Base::Container, false) => (Color::rgb(0x438dd5), Color::rgb(0x3c7fc0), white),
        (Base::Container, true) => (Color::rgb(0xb3b3b3), Color::rgb(0xa6a6a6), white),
        (Base::Component, false) => (Color::rgb(0x85bbf0), Color::rgb(0x78a8d8), black),
        (Base::Component, true) => (Color::rgb(0xcccccc), Color::rgb(0xbfbfbf), black),
    }
}

/// One line of text in a box.
struct Line {
    text: String,
    size: f32,
    style: TextStyle,
}

struct Text<'a, M> {
    metrics: &'a M,
    theme: &'a Theme,
}

impl<M: TextMetrics> Text<'_, M> {
    fn width(&self, l: &Line) -> f32 {
        self.metrics.styled_width(&l.text, l.size, l.style)
    }

    fn height(&self, l: &Line) -> f32 {
        l.size * self.theme.line_height / self.theme.font_size
    }

    /// `text` broken at spaces to fit `max`, each line in `size` and `style`.
    fn wrap(&self, text: &str, size: f32, style: TextStyle, max: f32) -> Vec<Line> {
        let mut out = Vec::new();
        for para in text.lines() {
            let mut cur = String::new();
            for word in para.split_whitespace() {
                let next = if cur.is_empty() {
                    word.to_owned()
                } else {
                    format!("{cur} {word}")
                };
                if !cur.is_empty() && self.metrics.styled_width(&next, size, style) > max {
                    out.push(Line {
                        text: std::mem::replace(&mut cur, word.to_owned()),
                        size,
                        style,
                    });
                } else {
                    cur = next;
                }
            }
            out.push(Line {
                text: cur,
                size,
                style,
            });
        }
        out
    }
}

const PLAIN: TextStyle = TextStyle {
    bold: false,
    italic: false,
};
const BOLD: TextStyle = TextStyle {
    bold: true,
    italic: false,
};
const ITALIC: TextStyle = TextStyle {
    bold: false,
    italic: true,
};

pub fn c4(c: &C4, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let px = Px { metrics, theme };
    let t = Text { metrics, theme };
    let inner = WIDTH - 2.0 * PAD;
    // Each element's lines, above and below the person icon.
    let content: Vec<(Vec<Line>, Vec<Line>)> = c
        .elements
        .iter()
        .zip(&c.chart.nodes)
        .map(|(e, n)| {
            let top = vec![Line {
                text: format!("<<{}>>", e.type_name()),
                size: SMALL,
                style: PLAIN,
            }];
            let mut below = t.wrap(&n.label.text, LARGE, BOLD, inner);
            if let Some(techn) = &e.techn {
                below.extend(t.wrap(&format!("[{techn}]"), SMALL, ITALIC, inner));
            }
            if let Some(d) = &e.descr {
                below.extend(t.wrap(d, theme.font_size, PLAIN, inner));
            }
            (top, below)
        })
        .collect();
    let lines_h = |ls: &[Line]| ls.iter().map(|l| t.height(l)).sum::<f32>();
    let top_room = |i: usize| match c.elements[i].body {
        Body::Db => 10.0,
        _ => 0.0,
    };
    let node_size = |i: usize| -> (f64, f64) {
        let (top, below) = &content[i];
        let icon = if c.elements[i].base == Base::Person {
            ICON + 4.0
        } else {
            0.0
        };
        let h = 2.0 * PAD + top_room(i) + lines_h(top) + 4.0 + icon + lines_h(below);
        (f64::from(WIDTH), f64::from(h))
    };
    // Each boundary's heading: label, `[type]`, and description.
    let headings: Vec<Vec<Line>> = c
        .chart
        .subgraphs
        .iter()
        .zip(&c.boundaries)
        .map(|(s, b)| {
            let mut ls = t.wrap(&s.title.text, theme.font_size, BOLD, f32::INFINITY);
            if let Some(k) = &b.kind {
                ls.push(Line {
                    text: format!("[{k}]"),
                    size: SMALL,
                    style: PLAIN,
                });
            }
            if let Some(d) = &b.descr {
                ls.extend(t.wrap(d, SMALL, ITALIC, 2.0 * WIDTH));
            }
            ls
        })
        .collect();
    let title = |g: usize| -> (f64, f64) {
        let ls = &headings[g];
        let w = ls.iter().map(|l| t.width(l)).fold(0.0, f32::max);
        (f64::from(w), f64::from(lines_h(ls) + 8.0))
    };
    // A relation's label holds its technology too.
    let mut chart = c.chart.clone();
    for (e, r) in chart.edges.iter_mut().zip(&c.rels) {
        if let Some(techn) = &r.techn {
            let text = match &e.label {
                Some(l) => format!("{}\n[{techn}]", l.text),
                None => format!("[{techn}]"),
            };
            e.label = Some(Label::plain(text));
        }
        if let Some(v) = &r.colors.line {
            e.styles.push(style("stroke", v));
        }
        if let Some(v) = &r.colors.font {
            e.styles.push(style("color", v));
        }
    }
    // Elements side by side in a row are spread apart to fit the label of a relation between them.
    let row = |i: usize| {
        let parent = c.node_parent[i];
        let k = (0..i).filter(|&j| c.node_parent[j] == parent).count();
        (parent, k / c.shapes_in_row)
    };
    let index = |id: &str| c.chart.nodes.iter().position(|n| n.id == id);
    let gap_x = chart
        .edges
        .iter()
        .filter(|e| match (index(&e.from), index(&e.to)) {
            (Some(a), Some(b)) => a != b && row(a) == row(b),
            _ => false,
        })
        .filter_map(|e| px.edge_label(e.label.as_ref()?))
        .map(|s| f64::from(s.w) + 20.0)
        .fold(50.0, f64::max);
    let placed = layout::layout(
        c,
        &Sizes {
            node: &node_size,
            title: &title,
            gap_x,
            gap_y: 50.0,
            pad: 20.0,
        },
    );

    let rect = |r: layout::Rect| Rect {
        x: r.x.round() as i32,
        y: r.y.round() as i32,
        w: r.w.round() as i32 + 1,
        h: r.h.round() as i32 + 1,
    };
    let nodes: Vec<Rect> = placed.nodes.iter().map(|&r| rect(r)).collect();
    let clusters: Vec<Rect> = placed.boundaries.iter().map(|&r| rect(r)).collect();
    let center = |id: &str| -> Option<Point> {
        let r = match c.chart.nodes.iter().position(|n| n.id == id) {
            Some(i) => nodes[i],
            None => clusters[c.chart.subgraphs.iter().position(|s| s.id == id)?],
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
        titles: vec![None; c.chart.subgraphs.len()],
        edges,
        notes: Vec::new(),
    };
    // Elements and boundaries are filled and outlined by the painter; their text is added below.
    for (n, e) in chart.nodes.iter_mut().zip(&c.elements) {
        let (fill, border, _) = palette(e.base, e.external);
        n.label = Label::default();
        n.styles = vec![
            style("fill", e.colors.background.as_deref().unwrap_or(&hex(fill))),
            style("stroke", e.colors.line.as_deref().unwrap_or(&hex(border))),
        ];
        // A queue's pipe is drawn below; the painter keeps its box for clipping.
        if e.body == Body::Queue {
            n.shape = Shape::Rect;
            n.styles = vec![style("fill", "none"), style("stroke", "none")];
        }
    }
    for (s, b) in chart.subgraphs.iter_mut().zip(&c.boundaries) {
        s.title = Label::default();
        s.styles = vec![style("fill", "none"), style("stroke", &hex(GREY))];
        if !b.deployment {
            s.styles.push(style("stroke-dasharray", "7 7"));
        }
    }
    let (mut scene, off) = paint::draw(&chart, &layout, metrics, theme);

    let mut push = |owner: Owner, primitive: Primitive| {
        scene.elements.push(Element { owner, primitive });
    };
    for (i, e) in c.elements.iter().enumerate() {
        let r = placed.nodes[i];
        let b = BoxF {
            x: r.x as f32 + off.x,
            y: r.y as f32 + off.y,
            w: r.w as f32,
            h: r.h as f32,
        };
        let (_, _, text) = palette(e.base, e.external);
        let color = e
            .colors
            .font
            .as_deref()
            .and_then(parse_color)
            .unwrap_or(text);
        let owner = Owner::Node(i);
        if e.body == Body::Queue {
            let (fill, border, _) = palette(e.base, e.external);
            let color = |set: &Option<String>, default| {
                set.as_deref().and_then(parse_color).unwrap_or(default)
            };
            let (fill, border) = (
                color(&e.colors.background, fill),
                color(&e.colors.line, border),
            );
            let stroke = Some(Stroke {
                width: 1.0,
                color: border,
                dash: None,
            });
            let (body, end) = pipe(b);
            push(
                owner,
                Primitive::Path {
                    segments: body,
                    fill: Some(fill),
                    stroke,
                },
            );
            push(
                owner,
                Primitive::Path {
                    segments: end,
                    fill: None,
                    stroke,
                },
            );
        }
        let cx = b.x + b.w / 2.0;
        let mut y = b.y + PAD + top_room(i);
        let (top, below) = &content[i];
        let lines = |ls: &[Line], y: &mut f32, push: &mut dyn FnMut(Owner, Primitive)| {
            for l in ls {
                let h = t.height(l);
                push(
                    owner,
                    Primitive::Text {
                        text: l.text.clone(),
                        center: Pt::new(cx, *y + h / 2.0),
                        size: l.size,
                        color,
                        style: l.style,
                    },
                );
                *y += h;
            }
        };
        lines(top, &mut y, &mut push);
        y += 4.0;
        if e.base == Base::Person {
            for p in person(Pt::new(cx, y), color) {
                push(owner, p);
            }
            y += ICON + 4.0;
        }
        lines(below, &mut y, &mut push);
    }
    for (g, ls) in headings.iter().enumerate() {
        let r = placed.boundaries[g];
        let x = r.x as f32 + off.x + 20.0;
        let mut y = r.y as f32 + off.y + 20.0;
        for l in ls {
            let h = t.height(l);
            push(
                Owner::Cluster(g),
                Primitive::Text {
                    text: l.text.clone(),
                    center: Pt::new(x + t.width(l) / 2.0, y + h / 2.0),
                    size: l.size,
                    color: theme.text,
                    style: l.style,
                },
            );
            y += h;
        }
    }
    // Relations and their labels go over the elements' text.
    scene
        .elements
        .sort_by_key(|e| matches!(e.owner, Owner::Edge(_)));
    scene
}

fn style(property: &str, value: &str) -> Style {
    Style {
        property: property.to_owned(),
        value: value.to_owned(),
    }
}

fn hex(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

/// How far a queue's rounded ends reach in.
const CAP: f32 = 10.0;

/// A queue: a cylinder on its side filling `b`, and the rim of its right end.
fn pipe(b: BoxF) -> (Vec<Segment>, Vec<Segment>) {
    let ry = b.h / 2.0;
    let (l, r) = (b.x + CAP, b.right() - CAP);
    let arc = |sweep: bool, to: Pt| Segment::Arc {
        radii: Pt::new(CAP, ry),
        large: false,
        sweep,
        to,
    };
    let body = vec![
        Segment::Move(Pt::new(l, b.y)),
        Segment::Line(Pt::new(r, b.y)),
        arc(true, Pt::new(r, b.bottom())),
        Segment::Line(Pt::new(l, b.bottom())),
        arc(true, Pt::new(l, b.y)),
        Segment::Close,
    ];
    let end = vec![
        Segment::Move(Pt::new(r, b.y)),
        arc(false, Pt::new(r, b.bottom())),
    ];
    (body, end)
}

/// A person's head and shoulders, `ICON` tall, hanging from `top`.
fn person(top: Pt, color: Color) -> Vec<Primitive> {
    let r = ICON * 0.22;
    let head = Pt::new(top.x, top.y + r);
    let (w, y0) = (ICON * 0.45, top.y + 2.0 * r + 2.0);
    let shoulders = vec![
        Segment::Move(Pt::new(top.x - w, top.y + ICON)),
        Segment::Line(Pt::new(top.x - w, y0 + w * 0.5)),
        Segment::Arc {
            radii: Pt::new(w, w * 0.5),
            large: false,
            sweep: true,
            to: Pt::new(top.x + w, y0 + w * 0.5),
        },
        Segment::Line(Pt::new(top.x + w, top.y + ICON)),
        Segment::Close,
    ];
    vec![
        Primitive::Path {
            segments: shapes::ellipse(head, r, r),
            fill: Some(color),
            stroke: None,
        },
        Primitive::Path {
            segments: shoulders,
            fill: Some(color),
            stroke: None,
        },
    ]
}
