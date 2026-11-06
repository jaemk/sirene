//! Terminal text rendering of flowcharts (ASCIIR-*).

mod canvas;

use unicode_width::UnicodeWidthStr;

use crate::flowchart::{Arrow, Edge, Flowchart, Label, Measure, Node, Shape, Stroke, Subgraph};
use crate::layout::{self, Config, Point, Rect, Size, Spacing};
use canvas::{Canvas, DOWN, Glyph, LEFT, Line, RIGHT, UP, glyph};

/// Characters to draw with (ASCIIR-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Charset {
    /// Box-drawing characters.
    #[default]
    Unicode,
    /// Only printable ASCII.
    Ascii,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Options {
    pub charset: Charset,
}

/// Spacing in cells (ASCIIR-8).
pub const CONFIG: Config = Config {
    vertical: Spacing {
        node_gap: 3,
        edge_gap: 0,
        rank_gap: 1,
        track_gap: 1,
        cluster_cross_pad: 2,
        cluster_main_pad: 1,
        port_inset: 1,
        loop_size: 2,
        loop_inset: 0,
    },
    horizontal: Spacing {
        node_gap: 1,
        edge_gap: 0,
        rank_gap: 2,
        track_gap: 1,
        cluster_cross_pad: 1,
        cluster_main_pad: 2,
        port_inset: 1,
        loop_size: 2,
        loop_inset: 0,
    },
};

/// Renders `chart` as text (ASCIIR-1).
pub fn render(chart: &Flowchart, options: &Options) -> String {
    let graph = chart.layout_graph(&Cells);
    let layout = layout::layout(&graph, &CONFIG);
    let mut canvas = Canvas::new(layout.size.w, layout.size.h);

    for r in &layout.clusters {
        draw_box(&mut canvas, *r, Line::Light);
    }
    for (n, r) in chart.nodes.iter().zip(&layout.nodes) {
        draw_node_frame(&mut canvas, n, *r);
    }
    let rect_of = |id: &str| -> Rect {
        match chart.subgraphs.iter().position(|s| s.id == id) {
            Some(i) => layout.clusters[i],
            None => {
                let i = chart.nodes.iter().position(|n| n.id == id);
                layout.nodes[i.expect("edge endpoint exists")]
            }
        }
    };
    for (e, route) in chart.edges.iter().zip(&layout.edges) {
        if let Some(route) = route {
            if e.stroke != Stroke::Invisible {
                draw_edge(
                    &mut canvas,
                    e,
                    &route.points,
                    rect_of(&e.from),
                    rect_of(&e.to),
                    route.label,
                );
            }
        }
    }
    for (n, r) in chart.nodes.iter().zip(&layout.nodes) {
        draw_node_text(&mut canvas, n, *r);
    }
    for (e, route) in chart.edges.iter().zip(&layout.edges) {
        let label = route.as_ref().and_then(|r| r.label).zip(e.label.as_ref());
        if let Some((r, l)) = label {
            if e.stroke != Stroke::Invisible {
                draw_lines(&mut canvas, &lines(l), r.x, r.w, r.y);
            }
        }
    }
    for (s, r) in chart.subgraphs.iter().zip(&layout.titles) {
        if let Some(r) = r {
            canvas.text(r.x, r.y, &format!(" {} ", title(s)));
        }
    }

    let body = canvas.render(options.charset);
    match &chart.title {
        Some(t) => {
            let t = clean(t);
            let pad = ((layout.size.w - width(&t)).max(0) / 2) as usize;
            format!("{}{t}\n\n{body}", " ".repeat(pad))
        }
        None => body,
    }
}

/// Sizes in terminal cells.
struct Cells;

impl Measure for Cells {
    fn node(&self, node: &Node) -> Size {
        let lines = lines(&node.label);
        let w = lines.iter().map(|l| width(l)).max().unwrap_or(0);
        let h = lines.len() as i32;
        match frame(node.shape) {
            Frame::Text => Size::new(w.max(1), h),
            Frame::Framed => Size::new(w + 6, h + 2),
            Frame::Cylinder => Size::new(w + 4, h + 3),
            _ => Size::new(w + 4, h + 2),
        }
    }

    fn edge_label(&self, label: &Label) -> Option<Size> {
        let lines = lines(label);
        let w = lines.iter().map(|l| width(l)).max().unwrap_or(0);
        (w > 0).then(|| Size::new(w, lines.len() as i32))
    }

    /// The title and a space on each side, on the top border.
    fn subgraph_title(&self, subgraph: &Subgraph) -> Size {
        let t = title(subgraph);
        if t.is_empty() {
            Size::default()
        } else {
            Size::new(width(&t) + 2, 1)
        }
    }
}

fn width(s: &str) -> i32 {
    UnicodeWidthStr::width(s) as i32
}

/// Replaces control characters, which have no width, with spaces.
fn clean(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// Display lines of a label, with markdown emphasis markers dropped (ASCIIR-3).
fn lines(label: &Label) -> Vec<String> {
    let text = if label.markdown {
        label.text.replace("**", "").replace('*', "")
    } else {
        label.text.clone()
    };
    text.split('\n').map(clean).collect()
}

fn title(s: &Subgraph) -> String {
    lines(&s.title).join(" ").trim().to_owned()
}

/// Writes `lines` centered in the columns `x..x+w`, from row `y`.
fn draw_lines(canvas: &mut Canvas, lines: &[String], x: i32, w: i32, y: i32) {
    for (i, l) in lines.iter().enumerate() {
        canvas.text(x + (w - width(l)) / 2, y + i as i32, l);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Frame {
    Plain,
    Rounded,
    Round,
    Double,
    Diamond,
    Hexagon,
    Odd,
    LeanRight,
    LeanLeft,
    TrapBottom,
    TrapTop,
    Framed,
    Cylinder,
    Text,
}

fn frame(shape: Shape) -> Frame {
    match shape {
        Shape::Rounded => Frame::Rounded,
        Shape::Stadium
        | Shape::Circle
        | Shape::SmallCircle
        | Shape::FilledCircle
        | Shape::FramedCircle
        | Shape::CrossedCircle
        | Shape::Ellipse => Frame::Round,
        Shape::DoubleCircle => Frame::Double,
        Shape::Diamond => Frame::Diamond,
        Shape::Hexagon => Frame::Hexagon,
        Shape::Odd => Frame::Odd,
        Shape::LeanRight => Frame::LeanRight,
        Shape::LeanLeft => Frame::LeanLeft,
        Shape::TrapezoidBottom => Frame::TrapBottom,
        Shape::TrapezoidTop => Frame::TrapTop,
        Shape::FramedRect => Frame::Framed,
        Shape::Cylinder | Shape::HorizontalCylinder | Shape::LinedCylinder => Frame::Cylinder,
        Shape::Text => Frame::Text,
        _ => Frame::Plain,
    }
}

fn draw_box(canvas: &mut Canvas, r: Rect, line: Line) {
    canvas.hline(r.x, r.right(), r.y, line);
    canvas.hline(r.x, r.right(), r.bottom(), line);
    canvas.vline(r.x, r.y, r.bottom(), line);
    canvas.vline(r.right(), r.y, r.bottom(), line);
}

const ROUND: [Glyph; 4] = [
    glyph('╭', '.'),
    glyph('╮', '.'),
    glyph('╰', '\''),
    glyph('╯', '\''),
];
const SLANT: [Glyph; 4] = [
    glyph('╱', '/'),
    glyph('╲', '\\'),
    glyph('╲', '\\'),
    glyph('╱', '/'),
];
const FORWARD: Glyph = glyph('╱', '/');
const BACK: Glyph = glyph('╲', '\\');

fn draw_node_frame(canvas: &mut Canvas, node: &Node, r: Rect) {
    let f = frame(node.shape);
    if f == Frame::Text {
        return;
    }
    draw_box(
        canvas,
        r,
        if f == Frame::Double {
            Line::Double
        } else {
            Line::Light
        },
    );
    let corners = match f {
        Frame::Rounded | Frame::Round | Frame::Cylinder => Some(ROUND),
        Frame::Diamond | Frame::Hexagon => Some(SLANT),
        _ => None,
    };
    if let Some([tl, tr, bl, br]) = corners {
        canvas.glyph(r.x, r.y, tl);
        canvas.glyph(r.right(), r.y, tr);
        canvas.glyph(r.x, r.bottom(), bl);
        canvas.glyph(r.right(), r.bottom(), br);
    }
    let sides = match f {
        Frame::Round => Some((glyph('(', '('), glyph(')', ')'))),
        Frame::Hexagon => Some((glyph('<', '<'), glyph('>', '>'))),
        Frame::Odd => Some((glyph('>', '>'), glyph('│', '|'))),
        Frame::LeanRight => Some((FORWARD, FORWARD)),
        Frame::LeanLeft => Some((BACK, BACK)),
        Frame::TrapBottom => Some((FORWARD, BACK)),
        Frame::TrapTop => Some((BACK, FORWARD)),
        _ => None,
    };
    if let Some((left, right)) = sides {
        for y in r.y + 1..r.bottom() {
            canvas.glyph(r.x, y, left);
            canvas.glyph(r.right(), y, right);
        }
    }
    match f {
        Frame::Framed => {
            canvas.vline(r.x + 1, r.y, r.bottom(), Line::Light);
            canvas.vline(r.right() - 1, r.y, r.bottom(), Line::Light);
        }
        Frame::Cylinder => canvas.hline(r.x, r.right(), r.y + 1, Line::Light),
        _ => {}
    }
}

fn draw_node_text(canvas: &mut Canvas, node: &Node, r: Rect) {
    let lines = lines(&node.label);
    // The interior: columns `x..x+w`, rows `y..y+h`.
    let (x, w, y, h) = match frame(node.shape) {
        Frame::Text => (r.x, r.w, r.y, r.h),
        Frame::Framed => (r.x + 2, r.w - 4, r.y + 1, r.h - 2),
        Frame::Cylinder => (r.x + 1, r.w - 2, r.y + 2, r.h - 3),
        _ => (r.x + 1, r.w - 2, r.y + 1, r.h - 2),
    };
    draw_lines(canvas, &lines, x, w, y + (h - lines.len() as i32) / 2);
}

/// Every cell along an orthogonal path, in order.
fn cells(points: &[Point]) -> Vec<Point> {
    let mut out = Vec::new();
    for (i, &p) in points.iter().enumerate() {
        if i == 0 {
            out.push(p);
            continue;
        }
        let mut at = points[i - 1];
        while at != p {
            at.x += (p.x - at.x).signum();
            if at.x == p.x {
                at.y += (p.y - at.y).signum();
            }
            out.push(at);
        }
    }
    out
}

/// The direction bits from `a` to the adjacent cell `b`, and back.
fn bits(a: Point, b: Point) -> (u8, u8) {
    match (b.x - a.x, b.y - a.y) {
        (1, _) => (RIGHT, LEFT),
        (-1, _) => (LEFT, RIGHT),
        (_, 1) => (DOWN, UP),
        _ => (UP, DOWN),
    }
}

fn arrow_glyph(arrow: Arrow, toward: u8) -> Glyph {
    match arrow {
        Arrow::Cross => glyph('x', 'x'),
        Arrow::Circle => glyph('o', 'o'),
        Arrow::Point => match toward {
            DOWN => glyph('▼', 'v'),
            UP => glyph('▲', '^'),
            RIGHT => glyph('►', '>'),
            _ => glyph('◄', '<'),
        },
    }
}

/// Draws an edge, clipped to the outside of its endpoints and label (ASCIIR-4).
fn draw_edge(
    canvas: &mut Canvas,
    edge: &Edge,
    points: &[Point],
    from: Rect,
    to: Rect,
    label: Option<Rect>,
) {
    let line = match edge.stroke {
        Stroke::Thick => Line::Heavy,
        Stroke::Dotted => Line::Dotted,
        _ => Line::Light,
    };
    let path = cells(points);
    let Some(first) = path.iter().position(|&p| !from.contains(p)) else {
        return;
    };
    let Some(last) = path.iter().rposition(|&p| !to.contains(p)) else {
        return;
    };
    if first > last {
        return;
    }
    let in_label = |p: Point| label.is_some_and(|r| r.contains(p));
    for i in first..last {
        let (a, b) = (path[i], path[i + 1]);
        let (ab, ba) = bits(a, b);
        if !in_label(a) {
            canvas.add(a.x, a.y, ab, line);
        }
        if !in_label(b) {
            canvas.add(b.x, b.y, ba, line);
        }
    }
    // Each end: the endpoint cell next to the outermost drawn cell, if the path reaches one.
    let ends = [
        (first.checked_sub(1), first, edge.start),
        ((last + 1 < path.len()).then_some(last + 1), last, edge.end),
    ];
    for (inside, outside, arrow) in ends {
        let (Some(i), o) = (inside.map(|i| path[i]), path[outside]) else {
            continue;
        };
        match arrow {
            None => {
                let (oi, io) = bits(o, i);
                canvas.add(o.x, o.y, oi, line);
                canvas.add(i.x, i.y, io, line);
            }
            Some(a) => canvas.arrow(o.x, o.y, arrow_glyph(a, bits(o, i).0)),
        }
    }
}

#[cfg(test)]
mod tests;
