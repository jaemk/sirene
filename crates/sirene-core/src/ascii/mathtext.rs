//! Formulas inside label lines as text (LEMB-4): a line with a formula is as many rows as the
//! formula has, text and formulas side by side with their baselines on one row, and every row
//! of the line the same width so the rows stay aligned when the line is centered.

#[cfg(feature = "math")]
use std::cell::RefCell;
#[cfg(feature = "math")]
use std::collections::HashMap;
#[cfg(feature = "math")]
use std::rc::Rc;

#[cfg(feature = "math")]
use unicode_width::UnicodeWidthChar;

#[cfg(feature = "math")]
use crate::flowchart::Span;
use crate::flowchart::{Delim, Label};

/// The formulas of one render as rows, set once each, keyed on the formula span's text (which
/// holds the delimiters, so says the language and style too).
#[cfg(feature = "math")]
#[derive(Default)]
pub(super) struct Rows(RefCell<HashMap<String, Rc<Block>>>);

#[cfg(not(feature = "math"))]
#[derive(Default)]
pub(super) struct Rows(());

#[cfg(feature = "math")]
impl Rows {
    /// The block of `span`: a formula's rows when it typesets (cached), else its text.
    fn block(&self, span: &Span) -> Rc<Block> {
        if !span.math {
            return Rc::new(Block::text(&span.text));
        }
        if let Some(hit) = self.0.borrow().get(&span.text) {
            return hit.clone();
        }
        let made = Rc::new(Block::of(span));
        self.0.borrow_mut().insert(span.text.clone(), made.clone());
        made
    }
}

/// The rows of `label` when some line holds a formula, else `None` (the label is its plain
/// lines).
#[cfg(feature = "math")]
pub(super) fn rows(rows: &Rows, label: &Label) -> Option<Vec<String>> {
    if !label.has_formula() {
        return None;
    }
    let mut out = Vec::new();
    for line in label.runs() {
        out.extend(line_rows(rows, &line));
    }
    Some(out)
}

/// Without the `math` feature a formula is its source, and not read as markdown.
#[cfg(not(feature = "math"))]
pub(super) fn rows(_: &Rows, label: &Label) -> Option<Vec<String>> {
    if !label.has_formula() {
        return None;
    }
    let line = |l: Vec<crate::flowchart::Span>| l.iter().map(|s| s.text.as_str()).collect();
    Some(label.runs().into_iter().map(line).collect())
}

/// `lines` of source text as rows, each line with a formula as many rows as the formula has.
/// Without a formula, or without the `math` feature, the lines are unchanged.
#[cfg(feature = "math")]
pub(super) fn text_rows(lines: &[String], delim: Delim) -> Vec<String> {
    use crate::flowchart::formula;
    if !lines.iter().any(|l| formula::has(l, delim)) {
        return lines.to_vec();
    }
    let cache = Rows::default();
    lines
        .iter()
        .flat_map(|l| line_rows(&cache, &formula::runs(l, delim)))
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
impl Block {
    fn text(text: &str) -> Block {
        Block {
            rows: vec![text.to_owned()],
            base: 0,
            width: width(text),
        }
    }

    /// The rows of a formula span as the scene draws it: the rows of a formula that typesets
    /// (none, for a formula with nothing to draw, which is no width and one empty row), and the
    /// span's text only when it does not typeset.
    fn of(span: &Span) -> Block {
        match span.ascii_rows() {
            Some(Ok((rows, _))) if rows.is_empty() => Block::text(""),
            Some(Ok((rows, base))) => Block {
                width: rows.iter().map(|r| width(r)).max().unwrap_or(0),
                rows,
                base,
            },
            _ => Block::text(&span.text),
        }
    }
}

/// One line as rows: the blocks side by side on a common baseline.
#[cfg(feature = "math")]
fn line_rows(cache: &Rows, line: &[Span]) -> Vec<String> {
    let blocks: Vec<Rc<Block>> = line.iter().map(|s| cache.block(s)).collect();
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

#[cfg(all(test, feature = "math"))]
mod tests {
    use super::*;

    fn formula(text: &str) -> Span {
        Span {
            text: text.to_owned(),
            math: true,
            ..Span::default()
        }
    }

    fn label(text: &str) -> Label {
        Label::plain(text)
    }

    #[test]
    fn a_formula_with_nothing_to_draw_is_no_width_not_its_source() {
        for src in ["$$$$", r"$$\phantom{x}$$", r"$$\,$$"] {
            let b = Block::of(&formula(src));
            assert_eq!(b.rows, [""], "{src}");
            assert_eq!(b.width, 0, "{src}");
            let got = rows(&Rows::default(), &label(&format!("a {src} b"))).unwrap();
            assert_eq!(got, ["a  b"], "{src}");
        }
    }

    #[test]
    fn only_a_formula_that_does_not_typeset_is_drawn_as_source() {
        let b = Block::of(&formula(r"$$\foo$$"));
        assert_eq!(b.rows, [r"$$\foo$$"]);
        let got = rows(&Rows::default(), &label(r"a $$\foo$$ b")).unwrap();
        assert_eq!(got, [r"a $$\foo$$ b"]);
    }

    #[test]
    fn a_formula_that_typesets_is_its_rows() {
        let got = rows(&Rows::default(), &label(r"x $$\frac{1}{2}$$ y")).unwrap();
        assert_eq!(got.len(), 3, "{got:?}");
        assert!(got.iter().all(|r| !r.contains('$')), "{got:?}");
        let w = |r: &String| width(r);
        assert!(got.iter().all(|r| w(r) == w(&got[0])), "{got:?}");
    }

    #[test]
    fn a_formula_is_set_once_per_cache() {
        let cache = Rows::default();
        let l = label("$$x^2$$ and $$x^2$$ and $$y$$");
        let first = rows(&cache, &l).unwrap();
        for _ in 0..5 {
            assert_eq!(rows(&cache, &l).unwrap(), first);
        }
        assert_eq!(cache.0.borrow().len(), 2);
    }

    /// A flowchart of formula labels is set once per formula in a render, whatever the shape.
    #[test]
    fn a_flowchart_sets_each_formula_once_per_render() {
        let n = 100;
        let mut src = String::from("flowchart TD\n");
        for i in 0..n {
            let (open, close) = if i % 2 == 0 { ('[', ']') } else { ('{', '}') };
            src += &format!("    N{i}{open}\"$$x^{{{i}}}$$\"{close}\n");
        }
        let chart = crate::flowchart::parse(&src).lower().flowchart;
        crate::equation::count::reset();
        let out = super::super::render(&chart, &super::super::Options::default()).unwrap();
        assert_eq!(crate::equation::count::get(), n);
        assert!(!out.contains("$$"), "{out}");
    }

    #[test]
    fn an_empty_formula_label_draws_no_source() {
        for src in [r#"A["$$$$"]"#, r#"A["$$\phantom{x}$$"]"#, r#"A["$$\,$$"]"#] {
            let chart = crate::flowchart::parse(&format!("flowchart TD\n    {src}\n"))
                .lower()
                .flowchart;
            let out = super::super::render(&chart, &super::super::Options::default()).unwrap();
            assert!(
                !out.contains('$') && !out.contains("phantom"),
                "{src}: {out}"
            );
        }
    }

    #[test]
    fn text_spans_are_not_cached() {
        let cache = Rows::default();
        let b = cache.block(&Span::plain("$$x$$"));
        assert_eq!(b.rows, ["$$x$$"]);
        assert_eq!(cache.0.borrow().len(), 0);
    }
}
