//! A diagram with its `header`, `footer`, `caption`, and `legend` as text (PCOM-1): the diagram's
//! own drawing with the decorations in rows around it.
//!
//! From the top: the header, a top legend, the diagram, a bottom legend, the caption, the
//! footer, each set apart from the next by a blank row. Bold, italic, and color are not
//! drawn; formulas are rows (LEMB-4).

use super::mathtext::run_rows;
use super::{Charset, Options, width};
use crate::decor::{self, Decorated, HAlign, Run, VAlign};
use crate::flowchart::Span;

/// Rows of text, set against the side `align` says of a drawing.
struct Part {
    rows: Vec<String>,
    width: i32,
    align: HAlign,
}

/// The text of `d`: its diagram with the decorations around it.
pub fn decorated(d: &Decorated, options: &Options) -> crate::Result<String> {
    let inner = d.diagram.ascii(options)?;
    let charset = options.charset;
    let bullet = if charset == Charset::Ascii {
        "*"
    } else {
        "\u{2022}"
    };
    let banner = |b: &Option<decor::Banner>| {
        b.as_ref()
            .map(|b| block(&b.lines, b.align, b.align, charset, bullet))
    };
    let legend = d.decor.legend.as_ref().map(|l| {
        let part = block(&l.lines, HAlign::Left, l.horizontal, charset, bullet);
        boxed(part, charset)
    });
    let top_legend = d
        .decor
        .legend
        .as_ref()
        .is_some_and(|l| l.vertical == VAlign::Top);
    let (above, below) = if top_legend {
        (
            vec![banner(&d.decor.header), legend],
            vec![banner(&d.decor.caption), banner(&d.decor.footer)],
        )
    } else {
        (
            vec![banner(&d.decor.header)],
            vec![legend, banner(&d.decor.caption), banner(&d.decor.footer)],
        )
    };
    let above: Vec<Part> = above.into_iter().flatten().collect();
    let below: Vec<Part> = below.into_iter().flatten().collect();

    let body: Vec<&str> = inner.trim_end_matches('\n').lines().collect();
    let body_width = body.iter().map(|l| width(l)).max().unwrap_or(0);
    let total = above
        .iter()
        .chain(&below)
        .map(|p| p.width)
        .fold(body_width, i32::max);
    let indent = |n: i32| " ".repeat(n.max(0) as usize);
    let set = |p: &Part| -> Vec<String> {
        let pad = match p.align {
            HAlign::Left => 0,
            HAlign::Center => (total - p.width) / 2,
            HAlign::Right => total - p.width,
        };
        p.rows
            .iter()
            .map(|r| format!("{}{r}", indent(pad)))
            .collect()
    };
    let mut out: Vec<String> = Vec::new();
    for p in &above {
        out.extend(set(p));
        out.push(String::new());
    }
    let lead = (total - body_width) / 2;
    out.extend(body.iter().map(|l| format!("{}{l}", indent(lead))));
    for p in &below {
        out.push(String::new());
        out.extend(set(p));
    }
    let mut text = out.join("\n");
    if inner.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}

/// The rows of `runs`, formulas typeset to rows, trailing space cut.
fn text_rows(runs: &[Run], prefix: String, charset: Charset) -> Vec<String> {
    let mut spans = vec![Span::plain(prefix)];
    spans.extend(runs.iter().map(|r| r.span.clone()));
    run_rows(&[spans], charset)
        .into_iter()
        .map(|r| r.trim_end().to_owned())
        .collect()
}

/// What a block row is before the block's width is known.
enum Row {
    Text(String),
    Rule(char),
}

/// `rows` of cells as a grid of borders and cell text, each cell set against its left.
fn table(rows: &[Vec<Vec<String>>], charset: Charset) -> Vec<String> {
    let count = rows.iter().map(Vec::len).max().unwrap_or(0);
    let cols: Vec<i32> = (0..count)
        .map(|c| {
            let cells = rows.iter().filter_map(|r| r.get(c));
            cells
                .flat_map(|cell| cell.iter().map(|l| width(l)))
                .max()
                .unwrap_or(0)
        })
        .collect();
    // Corners and joins: top, middle, and bottom rows of the grid, and the bar and vertical.
    let (h, v, tl, tm, tr, ml, mm, mr, bl, bm, br) = if charset == Charset::Ascii {
        ('-', '|', '+', '+', '+', '+', '+', '+', '+', '+', '+')
    } else {
        (
            '\u{2500}', '\u{2502}', '\u{250c}', '\u{252c}', '\u{2510}', '\u{251c}', '\u{253c}',
            '\u{2524}', '\u{2514}', '\u{2534}', '\u{2518}',
        )
    };
    let border = |l: char, m: char, r: char| {
        let bars: Vec<String> = cols
            .iter()
            .map(|w| h.to_string().repeat((w + 2) as usize))
            .collect();
        format!("{l}{}{r}", bars.join(&m.to_string()))
    };
    let mut out = vec![border(tl, tm, tr)];
    for (i, row) in rows.iter().enumerate() {
        let height = row.iter().map(Vec::len).max().unwrap_or(1).max(1);
        for line in 0..height {
            let mut text = v.to_string();
            for (c, w) in cols.iter().enumerate() {
                let cell = row
                    .get(c)
                    .and_then(|cell| cell.get(line))
                    .map_or("", String::as_str);
                let pad = " ".repeat((w - width(cell)).max(0) as usize);
                text.push_str(&format!(" {cell}{pad} {v}"));
            }
            out.push(text);
        }
        out.push(if i + 1 == rows.len() {
            border(bl, bm, br)
        } else {
            border(ml, mm, mr)
        });
    }
    out
}

/// `lines` as rows, each set against the side `within` says of the widest, the block against
/// the side `align` says of the drawing. A rule is as wide as the block; a table is a grid.
fn block(lines: &[String], within: HAlign, align: HAlign, charset: Charset, bullet: &str) -> Part {
    let mut rows: Vec<Row> = Vec::new();
    let mut tables: Vec<Vec<Vec<String>>> = Vec::new();
    let flush = |tables: &mut Vec<Vec<Vec<String>>>, rows: &mut Vec<Row>| {
        if !tables.is_empty() {
            rows.extend(table(tables, charset).into_iter().map(Row::Text));
            tables.clear();
        }
    };
    for line in decor::lines(lines) {
        if !line.cells.is_empty() {
            let cells = line
                .cells
                .iter()
                .map(|c| text_rows(c, String::new(), charset))
                .collect();
            tables.push(cells);
            continue;
        }
        flush(&mut tables, &mut rows);
        if let Some(rule) = line.rule {
            rows.push(Row::Rule(rule.ch()));
        } else {
            let prefix = line.marker.prefix(bullet);
            rows.extend(
                text_rows(&line.runs, prefix, charset)
                    .into_iter()
                    .map(Row::Text),
            );
        }
    }
    flush(&mut tables, &mut rows);
    let w = rows
        .iter()
        .map(|r| match r {
            Row::Text(t) => width(t),
            Row::Rule(_) => 0,
        })
        .max()
        .unwrap_or(0);
    // A rule is at least the four characters it is written with.
    let w = if rows.iter().any(|r| matches!(r, Row::Rule(_))) {
        w.max(4)
    } else {
        w
    };
    let rows = rows
        .into_iter()
        .map(|r| match r {
            Row::Rule(c) => c.to_string().repeat(w as usize),
            Row::Text(r) => {
                let pad = match within {
                    HAlign::Left => 0,
                    HAlign::Center => (w - width(&r)) / 2,
                    HAlign::Right => w - width(&r),
                };
                format!("{}{r}", " ".repeat(pad.max(0) as usize))
            }
        })
        .collect();
    Part {
        rows,
        width: w.max(0),
        align,
    }
}

/// `part` in a box, with a space between the text and each side.
fn boxed(part: Part, charset: Charset) -> Part {
    let (tl, h, tr, v, bl, br) = if charset == Charset::Ascii {
        ('+', '-', '+', '|', '+', '+')
    } else {
        (
            '\u{250c}', '\u{2500}', '\u{2510}', '\u{2502}', '\u{2514}', '\u{2518}',
        )
    };
    let inner = part.width + 2;
    let bar = h.to_string().repeat(inner.max(0) as usize);
    let mut rows = vec![format!("{tl}{bar}{tr}")];
    for r in &part.rows {
        let pad = " ".repeat((part.width - width(r)).max(0) as usize);
        rows.push(format!("{v} {r}{pad} {v}"));
    }
    rows.push(format!("{bl}{bar}{br}"));
    Part {
        rows,
        width: inner + 2,
        align: part.align,
    }
}
