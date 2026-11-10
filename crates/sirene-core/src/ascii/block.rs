//! Block diagram as text (MBLOCK-4): the grid of boxes, groups with their titles on the top
//! border, and the edges listed below, since edges go straight between blocks.

use super::canvas::{Canvas, Line, glyph};
use super::{Cells, Options, clean, draw_box, draw_lines, draw_node_frame, draw_node_text, lines};
use super::{title as subgraph_title, width};
use crate::block::layout::{self, Sizes};
use crate::block::{Block, Heading};
use crate::flowchart::{Arrow, Edge, Measure, Stroke};
use crate::layout::Rect;

pub fn render(b: &Block, options: &Options) -> String {
    let chart = &b.chart;
    let node = |i: usize| -> (f64, f64) {
        let s = Cells.node(&chart.nodes[i]);
        match &b.arrows[i] {
            // A box with the heads on its sides.
            Some(_) => {
                let w = lines(&chart.nodes[i].label)
                    .iter()
                    .map(|l| width(l))
                    .max()
                    .unwrap_or(0);
                (f64::from(w + 4), f64::from(s.h.max(3)))
            }
            None => (f64::from(s.w), f64::from(s.h)),
        }
    };
    let title = |g: usize| -> f64 { if g == 0 { 0.0 } else { 1.0 } };
    let placed = layout::layout(
        b,
        &Sizes {
            node: &node,
            title: &title,
            pad_x: 2.0,
            pad_y: 1.0,
        },
    );
    let rect = |r: layout::Rect| Rect {
        x: r.x.round() as i32,
        y: r.y.round() as i32,
        w: r.w.round() as i32,
        h: r.h.round() as i32,
    };
    let mut canvas = Canvas::new(placed.width.round() as i32, placed.height.round() as i32);

    for (g, group) in b.groups.iter().enumerate().skip(1) {
        let r = rect(placed.groups[g]);
        let line = match group.subgraph {
            Some(s) if chart.subgraphs[s].region => Line::Dotted,
            _ => Line::Light,
        };
        draw_box(&mut canvas, r, line);
    }
    for (i, n) in chart.nodes.iter().enumerate() {
        let Some(r) = placed.nodes[i].map(rect) else {
            continue;
        };
        match &b.arrows[i] {
            Some(dirs) => {
                draw_box(&mut canvas, r, Line::Light);
                draw_heads(&mut canvas, r, dirs);
                let l = lines(&n.label);
                let y = r.y + (r.h - l.len() as i32) / 2;
                draw_lines(&mut canvas, &l, r.x + 1, r.w - 2, y);
            }
            None => {
                draw_node_frame(&mut canvas, n, r);
                draw_node_text(&mut canvas, n, r);
            }
        }
    }
    for group in b.groups.iter().skip(1) {
        let Some(s) = group.subgraph else { continue };
        let t = subgraph_title(&chart.subgraphs[s]);
        let g = b.group_of(s).expect("a subgraph's group");
        let r = rect(placed.groups[g]);
        if !t.is_empty() {
            canvas.text(r.x + 2, r.y, &format!(" {t} "));
        }
    }

    let mut out = String::new();
    if let Some(t) = &chart.title {
        let t = clean(t);
        let pad = ((placed.width.round() as i32 - width(&t)).max(0) / 2) as usize;
        out.push_str(&format!("{}{t}\n\n", " ".repeat(pad)));
    }
    out.push_str(&canvas.render(options.charset));
    let edges: Vec<String> = chart
        .edges
        .iter()
        .filter(|e| e.stroke != Stroke::Invisible)
        .map(edge)
        .collect();
    if !edges.is_empty() {
        out.push('\n');
        for e in edges {
            out.push_str(&e);
            out.push('\n');
        }
    }
    out
}

/// The arrowheads of a block arrow, on the middle of the sides it points out of.
fn draw_heads(canvas: &mut Canvas, r: Rect, dirs: &[Heading]) {
    let has = |h: Heading| dirs.contains(&h);
    let (cx, cy) = (r.x + (r.w - 1) / 2, r.y + (r.h - 1) / 2);
    let x = has(Heading::X);
    let y = has(Heading::Y);
    let (left, right) = (x || has(Heading::Left), x || has(Heading::Right));
    let (up, down) = (y || has(Heading::Up), y || has(Heading::Down));
    if left {
        canvas.arrow(r.x, cy, glyph('◄', '<'));
    }
    if right || !(left || up || down) {
        canvas.arrow(r.right(), cy, glyph('►', '>'));
    }
    if up {
        canvas.arrow(cx, r.y, glyph('▲', '^'));
    }
    if down {
        canvas.arrow(cx, r.bottom(), glyph('▼', 'v'));
    }
}

/// An edge as Mermaid writes it: `a --> b`, `a -- label --> b`.
fn edge(e: &Edge) -> String {
    let head = |a: Option<Arrow>, point: &'static str| match a {
        Some(Arrow::Point) => point,
        Some(Arrow::Cross) => "x",
        Some(Arrow::Circle) => "o",
        _ => "",
    };
    let (start, end) = (head(e.start, "<"), head(e.end, ">"));
    let body = match (e.stroke, end.is_empty()) {
        (Stroke::Thick, true) => "===",
        (Stroke::Thick, false) => "==",
        (Stroke::Dotted, _) => "-.-",
        (_, true) => "---",
        (_, false) => "--",
    };
    let link = format!("{start}{body}{end}");
    match e.label.as_ref().map(|l| lines(l).join(" ")) {
        Some(l) if !l.is_empty() => format!("{} {link} {}: {l}", e.from, e.to),
        _ => format!("{} {link} {}", e.from, e.to),
    }
}

#[cfg(test)]
mod tests {
    use crate::diagram::{self, Diagram};

    fn ascii(src: &str) -> String {
        let Diagram::Block(b) = diagram::lower(src).diagram else {
            panic!("not a block diagram")
        };
        super::render(&b, &super::Options::default())
    }

    #[test]
    fn edges_list_below_the_grid() {
        let out = ascii("block-beta\na b\na --> b\nb -- \"go\" --- a\n");
        assert!(out.ends_with("\na --> b\nb --- a: go\n"), "{out}");
    }
}
