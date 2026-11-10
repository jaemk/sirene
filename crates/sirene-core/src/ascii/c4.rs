//! C4 diagram as text (MC4-4): elements as boxes holding their type, label, technology, and
//! description, boundaries as boxes titled on the top border, placed as in the drawing, and the
//! relations listed below.

use super::{Cells, Options, clean, draw, width};
use crate::c4::layout::{self, Sizes};
use crate::c4::{C4, Rel};
use crate::flowchart::{Arrow, Edge, Label, Measure};
use crate::layout::{Layout, Rect, Size};

/// The widest a description runs before wrapping.
const MAX_TEXT: i32 = 30;

/// `s` broken at spaces into lines at most `max` wide; a longer word keeps a line of its own.
fn wrap(s: &str, max: i32) -> Vec<String> {
    let mut out = Vec::new();
    for para in s.lines() {
        let mut line = String::new();
        for word in para.split_whitespace() {
            if !line.is_empty() && width(&line) + 1 + width(word) > max {
                out.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        out.push(line);
    }
    out
}

pub fn render(c: &C4, options: &Options) -> String {
    let mut chart = c.chart.clone();
    chart.edges.clear();
    for (n, e) in chart.nodes.iter_mut().zip(&c.elements) {
        let mut lines = vec![format!("<<{}>>", e.type_name())];
        lines.extend(wrap(&n.label.text, MAX_TEXT));
        if let Some(t) = &e.techn {
            lines.extend(wrap(&format!("[{t}]"), MAX_TEXT));
        }
        if let Some(d) = &e.descr {
            lines.extend(wrap(d, MAX_TEXT));
        }
        n.label = Label::plain(lines.join("\n"));
    }
    for (s, b) in chart.subgraphs.iter_mut().zip(&c.boundaries) {
        let mut t = s.title.text.replace('\n', " ");
        if let Some(k) = &b.kind {
            t = format!("{t} [{k}]");
        }
        s.title = Label::plain(t);
    }
    let node = |i: usize| {
        let s = Cells.node(&chart.nodes[i]);
        (f64::from(s.w), f64::from(s.h))
    };
    let title = |g: usize| {
        let s = Cells.subgraph_title(&chart.subgraphs[g]);
        (f64::from(s.w + 2), 0.0)
    };
    let placed = layout::layout(
        c,
        &Sizes {
            node: &node,
            title: &title,
            gap_x: 2.0,
            gap_y: 1.0,
            pad: 2.0,
        },
    );
    let rect = |r: layout::Rect| Rect {
        x: r.x.round() as i32,
        y: r.y.round() as i32,
        w: r.w.round() as i32,
        h: r.h.round() as i32,
    };
    let clusters: Vec<Rect> = placed.boundaries.iter().map(|&r| rect(r)).collect();
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
    let layout = Layout {
        size: Size::new(placed.width.round() as i32, placed.height.round() as i32),
        nodes: placed.nodes.iter().map(|&r| rect(r)).collect(),
        clusters,
        titles,
        edges: Vec::new(),
        notes: Vec::new(),
    };
    let mut out = draw(&chart, &layout, options);
    if !c.chart.edges.is_empty() {
        out.push('\n');
        for (e, r) in c.chart.edges.iter().zip(&c.rels) {
            out.push_str(&rel(e, r));
            out.push('\n');
        }
    }
    out
}

/// A relation as a line: `a --> b: label [techn]`, with `<-->` both ways.
fn rel(e: &Edge, r: &Rel) -> String {
    let arrow = if e.start == Some(Arrow::Point) {
        "<-->"
    } else {
        "-->"
    };
    let mut text: Vec<String> = Vec::new();
    if let Some(l) = &e.label {
        text.push(clean(&l.text.replace('\n', " ")));
    }
    if let Some(t) = &r.techn {
        text.push(format!("[{}]", clean(t)));
    }
    let mut out = format!("{} {arrow} {}", e.from, e.to);
    if !text.is_empty() {
        out.push_str(": ");
        out.push_str(&text.join(" "));
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::diagram::{self, Diagram};

    fn ascii(src: &str) -> String {
        let Diagram::C4(c) = diagram::lower(src).diagram else {
            panic!("not a C4 diagram")
        };
        super::render(&c, &super::Options::default())
    }

    #[test]
    fn elements_and_relations() {
        let out = ascii(
            "C4Context\nPerson(a, \"A\")\nSystem(b, \"B\", \"Does things\")\nRel(a, b, \"Uses\", \"HTTP\")\nBiRel(b, a, \"Syncs\")\n",
        );
        assert_eq!(
            out,
            "╭────────────╮  ╭─────────────╮\n\
             │ <<person>> │  │ <<system>>  │\n\
             │     A      │  │      B      │\n\
             ╰────────────╯  │ Does things │\n\
             \x20               ╰─────────────╯\n\
             \n\
             a --> b: Uses [HTTP]\n\
             b <--> a: Syncs\n"
        );
    }
}
