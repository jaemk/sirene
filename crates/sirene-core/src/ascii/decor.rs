//! A diagram with its `header`, `footer`, `caption`, and `legend` as text (PCOM-1): the diagram's
//! own drawing with the decorations in rows around it.
//!
//! From the top: the header, a top legend, the diagram, a bottom legend, the caption, the
//! footer, each set apart from the next by a blank row. Bold, italic, and color are not
//! drawn; formulas are rows (LEMB-4).

use super::mathtext::run_rows;
use super::{Charset, Options, width};
use crate::decor::{self, Decorated, HAlign, VAlign};
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

/// `lines` as rows, each set against the side `within` says of the widest, the block against
/// the side `align` says of the drawing.
fn block(lines: &[String], within: HAlign, align: HAlign, charset: Charset, bullet: &str) -> Part {
    let runs: Vec<Vec<Span>> = decor::lines(lines)
        .into_iter()
        .map(|line| {
            let mut spans = vec![Span::plain(line.marker.prefix(bullet))];
            spans.extend(line.runs.into_iter().map(|r| r.span));
            spans
        })
        .collect();
    let rows: Vec<String> = run_rows(&runs, charset)
        .into_iter()
        .map(|r| r.trim_end().to_owned())
        .collect();
    let w = rows.iter().map(|r| width(r)).max().unwrap_or(0);
    let rows = rows
        .into_iter()
        .map(|r| {
            let pad = match within {
                HAlign::Left => 0,
                HAlign::Center => (w - width(&r)) / 2,
                HAlign::Right => w - width(&r),
            };
            format!("{}{r}", " ".repeat(pad.max(0) as usize))
        })
        .collect();
    Part {
        rows,
        width: w,
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
