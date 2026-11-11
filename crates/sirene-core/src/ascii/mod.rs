//! Terminal text rendering of flowcharts (ASCIIR-*).

mod architecture;
mod block;
mod c4;
mod canvas;
mod cynefin;
mod eventmodel;
mod gantt;
mod git;
mod info;
mod ishikawa;
mod journey;
mod kanban;
mod mindmap;
mod packet;
mod pie;
mod quadrant;
mod radar;
mod railroad;
mod sankey;
mod sequence;
mod timeline;
mod treemap;
mod treeview;
mod venn;
mod wardley;
mod xychart;

pub use architecture::render as architecture;
pub use block::render as block;
pub use c4::render as c4;
pub use cynefin::render as cynefin;
pub use eventmodel::render as eventmodel;
pub use gantt::render as gantt;
pub use git::render as git_graph;
pub use info::render as info;
pub use ishikawa::render as ishikawa;
pub use journey::render as journey;
pub use kanban::render as kanban;
pub use mindmap::render as mindmap;
pub use packet::render as packet;
pub use pie::render as pie;
pub use quadrant::render as quadrant;
pub use radar::render as radar;
pub use railroad::render as railroad;
pub use sankey::render as sankey;
pub use sequence::render as sequence;
pub use timeline::render as timeline;
pub use treemap::render as treemap;
pub use treeview::render as treeview;
pub use venn::render as venn;
pub use wardley::render as wardley;
pub use xychart::render as xychart;

use unicode_width::UnicodeWidthStr;

use crate::flowchart::{
    Arrow, Edge, Flowchart, Label, Measure, Node, Row, Shape, Stroke, Subgraph, clean,
};
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
    // Collapsed subgraphs (FLOWMO-15) are drawn folded into nodes.
    let folded = chart.collapsed();
    let chart = folded.as_ref().map_or(chart, |c| &c.chart);
    let graph = chart.layout_graph(&Cells);
    let layout = layout::layout(&graph, &CONFIG);
    draw(chart, &layout, options)
}

/// Draws `chart` placed by `layout`, in cells.
fn draw(chart: &Flowchart, layout: &layout::Layout, options: &Options) -> String {
    let mut canvas = Canvas::new(layout.size.w, layout.size.h);

    for (s, r) in chart.subgraphs.iter().zip(&layout.clusters) {
        let line = if s.region { Line::Dotted } else { Line::Light };
        draw_box(&mut canvas, *r, line);
    }
    for ((target, note), &r) in chart.placed_notes().zip(&layout.notes) {
        let n = match target {
            layout::Item::Node(i) => layout.nodes[i],
            layout::Item::Cluster(c) => layout.clusters[c],
        };
        let (cx, cy) = (n.x + n.w / 2, n.y + n.h / 2);
        if r.x > n.right() {
            canvas.hline(n.right() + 1, r.x - 1, cy, Line::Dotted);
        } else if r.right() < n.x {
            canvas.hline(r.right() + 1, n.x - 1, cy, Line::Dotted);
        } else if r.y > n.bottom() {
            canvas.vline(cx, n.bottom() + 1, r.y - 1, Line::Dotted);
        } else {
            canvas.vline(cx, r.bottom() + 1, n.y - 1, Line::Dotted);
        }
        draw_box(&mut canvas, r, Line::Light);
        canvas.glyph(r.right(), r.y, glyph('╮', '.'));
        draw_lines(&mut canvas, &lines(&note.label), r.x + 1, r.w - 2, r.y + 1);
    }
    for (n, r) in chart.nodes.iter().zip(&layout.nodes) {
        draw_node_frame(&mut canvas, n, *r);
    }
    // An endpoint's rect, and whether it is one character that edges stop short of.
    let rect_of = |id: &str| -> (Rect, bool) {
        match chart.subgraphs.iter().position(|s| s.id == id) {
            Some(i) => (layout.clusters[i], false),
            None => {
                let i = chart.nodes.iter().position(|n| n.id == id);
                let i = i.expect("edge endpoint exists");
                let glyph = matches!(frame_of(&chart.nodes[i]), Frame::Glyph(_));
                (layout.nodes[i], glyph)
            }
        }
    };
    for (e, route) in chart.edges.iter().zip(&layout.edges) {
        if let Some(route) = route
            && e.stroke != Stroke::Invisible
        {
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
    for (n, r) in chart.nodes.iter().zip(&layout.nodes) {
        draw_node_text(&mut canvas, n, *r);
    }
    for (e, route) in chart.edges.iter().zip(&layout.edges) {
        let label = route.as_ref().and_then(|r| r.label).zip(e.label.as_ref());
        if let Some((r, l)) = label
            && e.stroke != Stroke::Invisible
        {
            draw_lines(&mut canvas, &lines(l), r.x, r.w, r.y);
        }
    }
    for (s, r) in chart.subgraphs.iter().zip(&layout.titles) {
        if let Some(r) = r {
            canvas.text(r.x, r.y, &format!(" {} ", title(s)));
        }
    }

    let body = canvas.render(options.charset);
    let title = chart.title.as_deref().map(clean);
    match title.as_deref().map(str::trim) {
        Some(t) if !t.is_empty() => {
            let pad = ((layout.size.w - width(t)).max(0) / 2) as usize;
            format!("{}{t}\n\n{body}", " ".repeat(pad))
        }
        _ => body,
    }
}

/// Sizes in terminal cells.
struct Cells;

impl Measure for Cells {
    fn node(&self, node: &Node) -> Size {
        let lines = lines(&node.label);
        let w = lines.iter().map(|l| width(l)).max().unwrap_or(0);
        let h = lines.len() as i32;
        if !node.sections.is_empty() {
            let rows: Vec<Vec<String>> = node.sections.iter().map(|s| section_lines(s)).collect();
            let rw = rows.iter().flatten().map(|l| width(l)).max().unwrap_or(0);
            let rh: i32 = rows.iter().map(|s| s.len() as i32 + 1).sum();
            return Size::new(w.max(rw) + 4, h + 2 + rh);
        }
        match frame_of(node) {
            Frame::Glyph(_) => Size::new(1, 1),
            Frame::Bar => Size::new(5, 1),
            Frame::Text => Size::new(w.max(1), h),
            Frame::Framed => Size::new(w + 6, h + 2),
            Frame::Cylinder => Size::new(w + 4, h + 3),
            Frame::Collapsed => Size::new((w + 4).max(7), h + 4),
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

    fn note(&self, label: &Label) -> Size {
        let lines = lines(label);
        let w = lines.iter().map(|l| width(l)).max().unwrap_or(0);
        Size::new(w + 4, lines.len() as i32 + 2)
    }
}

fn width(s: &str) -> i32 {
    UnicodeWidthStr::width(s) as i32
}

/// Display lines of a label, with markdown emphasis markers dropped (ASCIIR-3).
fn lines(label: &Label) -> Vec<String> {
    label.lines()
}

fn title(s: &Subgraph) -> String {
    lines(&s.title).join(" ").trim().to_owned()
}

/// A compartment's rows with their cells padded into columns (LANGS-3).
fn section_lines(rows: &[Row]) -> Vec<String> {
    let mut cols: Vec<i32> = Vec::new();
    for row in rows {
        for (k, c) in row.cells.iter().enumerate() {
            let w = width(&clean(c));
            match cols.get_mut(k) {
                Some(x) => *x = (*x).max(w),
                None => cols.push(w),
            }
        }
    }
    rows.iter()
        .map(|row| {
            let mut line = String::new();
            for (k, c) in row.cells.iter().enumerate() {
                let c = clean(c);
                if cols[k] == 0 {
                    continue;
                }
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(&c);
                line.push_str(&" ".repeat((cols[k] - width(&c)).max(0) as usize));
            }
            line.trim_end().to_owned()
        })
        .collect()
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
    /// A collapsed subgraph: rounded, with a dotted rule and three dots under the label.
    Collapsed,
    Text,
    /// One character and no label.
    Glyph(Glyph),
    /// A heavy bar along the rect's long side, and no label.
    Bar,
}

/// A node's frame: its shape's, except that an unlabeled diamond (a state diagram's choice) is
/// one character.
fn frame_of(node: &Node) -> Frame {
    if node.shape == Shape::Diamond && node.label.text.is_empty() {
        Frame::Glyph(glyph('◇', '?'))
    } else {
        frame(node.shape)
    }
}

fn frame(shape: Shape) -> Frame {
    match shape {
        Shape::Rounded => Frame::Rounded,
        Shape::SmallCircle | Shape::FilledCircle => Frame::Glyph(glyph('●', '*')),
        Shape::FramedCircle => Frame::Glyph(glyph('◉', '@')),
        Shape::Fork => Frame::Bar,
        Shape::Stadium | Shape::Circle | Shape::CrossedCircle | Shape::Ellipse | Shape::UseCase => {
            Frame::Round
        }
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
        Shape::Collapsed => Frame::Collapsed,
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
    let f = frame_of(node);
    match f {
        Frame::Text => return,
        Frame::Glyph(g) => {
            // Over any edge end that reaches the cell.
            canvas.arrow(r.x + r.w / 2, r.y + r.h / 2, g);
            return;
        }
        Frame::Bar => {
            if r.w >= r.h {
                canvas.hline(r.x, r.right(), r.y + r.h / 2, Line::Heavy);
            } else {
                canvas.vline(r.x + r.w / 2, r.y, r.bottom(), Line::Heavy);
            }
            return;
        }
        _ => {}
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
        Frame::Rounded | Frame::Round | Frame::Cylinder | Frame::Collapsed => Some(ROUND),
        Frame::Diamond | Frame::Hexagon => Some(SLANT),
        _ => None,
    };
    if let Some([tl, tr, bl, br]) = corners {
        canvas.glyph(r.x, r.y, tl);
        canvas.glyph(r.right(), r.y, tr);
        canvas.glyph(r.x, r.bottom(), bl);
        canvas.glyph(r.right(), r.bottom(), br);
    }
    // A note's folded corner, as notes beside nodes draw it.
    if node.shape == Shape::Note {
        canvas.glyph(r.right(), r.y, glyph('╮', '.'));
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
        Frame::Collapsed => {
            canvas.hline(r.x + 1, r.right() - 1, r.bottom() - 2, Line::Dotted);
            let dots = glyph('·', '.');
            for dx in -1..=1 {
                canvas.glyph(r.x + r.w / 2 + dx, r.bottom() - 1, dots);
            }
        }
        _ => {}
    }
    // A rule above each compartment, joining the sides.
    let mut y = r.y + 1 + lines(&node.label).len() as i32;
    for section in &node.sections {
        canvas.hline(r.x, r.right(), y, Line::Light);
        y += 1 + section.len() as i32;
    }
}

fn draw_node_text(canvas: &mut Canvas, node: &Node, r: Rect) {
    let lines = lines(&node.label);
    if !node.sections.is_empty() {
        draw_lines(canvas, &lines, r.x + 1, r.w - 2, r.y + 1);
        let mut y = r.y + 2 + lines.len() as i32;
        for section in &node.sections {
            for row in section_lines(section) {
                canvas.text(r.x + 2, y, &row);
                y += 1;
            }
            y += 1;
        }
        return;
    }
    // The interior: columns `x..x+w`, rows `y..y+h`.
    let (x, w, y, h) = match frame_of(node) {
        Frame::Glyph(_) | Frame::Bar => return,
        Frame::Text => (r.x, r.w, r.y, r.h),
        Frame::Framed => (r.x + 2, r.w - 4, r.y + 1, r.h - 2),
        Frame::Cylinder => (r.x + 1, r.w - 2, r.y + 2, r.h - 3),
        Frame::Collapsed => (r.x + 1, r.w - 2, r.y + 1, r.h - 4),
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
        Arrow::Triangle => match toward {
            DOWN => glyph('▽', 'v'),
            UP => glyph('△', '^'),
            RIGHT => glyph('▷', '>'),
            _ => glyph('◁', '<'),
        },
        Arrow::Diamond => glyph('◆', '*'),
        Arrow::HollowDiamond => glyph('◇', 'o'),
        // Crow's feet as multiplicities.
        Arrow::One => glyph('1', '1'),
        Arrow::ZeroOne => glyph('?', '?'),
        Arrow::ZeroMany => glyph('*', '*'),
        Arrow::OneMany => glyph('+', '+'),
    }
}

/// Draws an edge, clipped to the outside of its endpoints and label (ASCIIR-4). A line without
/// an arrowhead joins its endpoint's border, unless that endpoint is a glyph.
fn draw_edge(
    canvas: &mut Canvas,
    edge: &Edge,
    points: &[Point],
    (from, from_glyph): (Rect, bool),
    (to, to_glyph): (Rect, bool),
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
        (first.checked_sub(1), first, edge.start, from_glyph),
        (
            (last + 1 < path.len()).then_some(last + 1),
            last,
            edge.end,
            to_glyph,
        ),
    ];
    for (inside, outside, arrow, glyph) in ends {
        let (Some(i), o) = (inside.map(|i| path[i]), path[outside]) else {
            continue;
        };
        match arrow {
            None if glyph => {}
            None => {
                let (oi, io) = bits(o, i);
                canvas.add(o.x, o.y, oi, line);
                canvas.add(i.x, i.y, io, line);
            }
            Some(a) => canvas.arrow(o.x, o.y, arrow_glyph(a, bits(o, i).0)),
        }
    }
    // End labels beside the line's outermost cells: right of a vertical run, above a
    // horizontal one (LANGS-3).
    for (label, at, next) in [
        (&edge.start_label, first, first + 1),
        (&edge.end_label, last, last.wrapping_sub(1)),
    ] {
        let (Some(l), Some(&p)) = (label, path.get(at)) else {
            continue;
        };
        let text = lines(l).join(" ");
        let vertical = path.get(next).is_none_or(|q| q.x == p.x);
        if vertical {
            canvas.text(p.x + 1, p.y, &text);
        } else {
            let x = if path.get(next).is_some_and(|q| q.x < p.x) {
                p.x - width(&text) + 1
            } else {
                p.x
            };
            canvas.text(x, p.y - 1, &text);
        }
    }
}

#[cfg(test)]
mod tests;
