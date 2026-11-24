//! Formulas inside label lines as text (LEMB-4): a line with a formula is as many rows as the
//! formula has, text and formulas side by side with their baselines on one row, and every row
//! of the line the same width so the rows stay aligned when the line is centered.

#[cfg(feature = "math")]
use unicode_width::UnicodeWidthChar;

#[cfg(feature = "math")]
use crate::flowchart::Span;
use crate::flowchart::{Delim, Label};

/// The rows of `label` when some line holds a formula that renders, else `None` (the label is
/// its plain lines).
#[cfg(feature = "math")]
pub(super) fn rows(label: &Label) -> Option<Vec<String>> {
    if !label.has_formula() {
        return None;
    }
    let mut out = Vec::new();
    for line in label.runs() {
        out.extend(line_rows(&line));
    }
    Some(out)
}

#[cfg(not(feature = "math"))]
pub(super) fn rows(_: &Label) -> Option<Vec<String>> {
    None
}

/// `lines` of source text as rows, each line with a formula as many rows as the formula has.
/// Without a formula, or without the `math` feature, the lines are unchanged.
#[cfg(feature = "math")]
pub(super) fn text_rows(lines: &[String], delim: Delim) -> Vec<String> {
    use crate::flowchart::formula;
    if !lines.iter().any(|l| formula::has(l, delim)) {
        return lines.to_vec();
    }
    lines
        .iter()
        .flat_map(|l| line_rows(&formula::runs(l, delim)))
        .collect()
}

#[cfg(not(feature = "math"))]
pub(super) fn text_rows(lines: &[String], _: Delim) -> Vec<String> {
    lines.to_vec()
}

/// A rectangle of text with the row its baseline is on.
#[cfg(feature = "math")]
struct Block {
    rows: Vec<String>,
    base: usize,
    width: i32,
}

#[cfg(feature = "math")]
fn block(span: &Span) -> Block {
    use crate::math::ascii_formula_rows;
    let formula = span
        .formula()
        .zip(span.delim())
        .and_then(|(f, d)| ascii_formula_rows(&f, d.style()).ok())
        .filter(|(rows, _)| !rows.is_empty());
    let (rows, base) = formula.unwrap_or_else(|| (vec![span.text.clone()], 0));
    let width = rows.iter().map(|r| width(r)).max().unwrap_or(0);
    Block { rows, base, width }
}

/// One line as rows: the blocks side by side on a common baseline.
#[cfg(feature = "math")]
fn line_rows(line: &[Span]) -> Vec<String> {
    let blocks: Vec<Block> = line.iter().map(block).collect();
    let above = blocks.iter().map(|b| b.base).max().unwrap_or(0);
    let below = blocks
        .iter()
        .map(|b| b.rows.len().saturating_sub(b.base + 1))
        .max()
        .unwrap_or(0);
    (0..above + below + 1)
        .map(|k| {
            let mut row = String::new();
            for b in &blocks {
                // The block's row on this line row, if it has one.
                let text = (k + b.base)
                    .checked_sub(above)
                    .and_then(|i| b.rows.get(i))
                    .map_or("", String::as_str);
                row.push_str(text);
                row.push_str(&" ".repeat((b.width - width(text)).max(0) as usize));
            }
            row
        })
        .collect()
}

#[cfg(feature = "math")]
fn width(s: &str) -> i32 {
    s.chars()
        .map(|c| UnicodeWidthChar::width(c).unwrap_or(0) as i32)
        .sum()
}
