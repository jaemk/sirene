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

use super::Charset;
#[cfg(feature = "math")]
use crate::flowchart::Span;
use crate::flowchart::{Delim, Label};

/// The formulas of one render as rows, set once each, keyed on the formula span's text (which
/// holds the delimiters, so says the language and style too).
#[cfg(feature = "math")]
#[derive(Default)]
pub(super) struct Rows {
    cache: RefCell<HashMap<String, Rc<Block>>>,
    /// The characters the formulas are drawn with.
    charset: Charset,
}

#[cfg(not(feature = "math"))]
#[derive(Default)]
pub(super) struct Rows(());

#[cfg(feature = "math")]
impl Rows {
    /// An empty set of formulas, to be drawn in `charset`.
    pub(super) fn new(charset: Charset) -> Rows {
        Rows {
            charset,
            ..Rows::default()
        }
    }

    /// The block of `span`: a formula's rows when it typesets (cached), else its text.
    fn block(&self, span: &Span) -> Rc<Block> {
        if !span.math {
            return Rc::new(Block::text(&span.text));
        }
        if let Some(hit) = self.cache.borrow().get(&span.text) {
            return hit.clone();
        }
        let made = Rc::new(Block::of(span, self.charset));
        self.cache
            .borrow_mut()
            .insert(span.text.clone(), made.clone());
        made
    }
}

#[cfg(not(feature = "math"))]
impl Rows {
    pub(super) fn new(_: Charset) -> Rows {
        Rows::default()
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
pub(super) fn text_rows(lines: &[String], delim: Delim, charset: Charset) -> Vec<String> {
    use crate::flowchart::formula;
    if !lines.iter().any(|l| formula::has(l, delim)) {
        return lines.to_vec();
    }
    let cache = Rows::new(charset);
    lines
        .iter()
        .flat_map(|l| line_rows(&cache, &formula::runs(l, delim)))
        .collect()
}

#[cfg(not(feature = "math"))]
pub(super) fn text_rows(lines: &[String], _: Delim, _: Charset) -> Vec<String> {
    lines.to_vec()
}

/// The rows of a compartment of a PlantUML node (LANGS-3) when a cell holds a formula, else
/// `None` (the compartment is its text rows). Each row of the compartment is as many lines as its
/// tallest formula; its cells are padded into columns by their widths, on a common baseline.
#[cfg(feature = "math")]
pub(super) fn section_rows(
    cache: &Rows,
    rows: &[crate::flowchart::Row],
) -> Option<Vec<Vec<String>>> {
    use crate::flowchart::formula;
    let cells: Vec<Vec<Vec<Span>>> = rows
        .iter()
        .map(|r| {
            r.cells
                .iter()
                .map(|c| formula::runs(&crate::flowchart::clean(c), Delim::Latex))
                .collect()
        })
        .collect();
    if !cells.iter().flatten().flatten().any(|s| s.math) {
        return None;
    }
    let cell_width = |cell: &[Span]| -> i32 { cell.iter().map(|s| cache.block(s).width).sum() };
    let mut cols: Vec<i32> = Vec::new();
    for row in &cells {
        for (k, cell) in row.iter().enumerate() {
            let w = cell_width(cell);
            match cols.get_mut(k) {
                Some(c) => *c = (*c).max(w),
                None => cols.push(w),
            }
        }
    }
    Some(
        cells
            .iter()
            .map(|row| {
                let mut line: Vec<Span> = Vec::new();
                for (k, cell) in row.iter().enumerate() {
                    if cols[k] == 0 {
                        continue;
                    }
                    if !line.is_empty() {
                        line.push(Span::plain(" "));
                    }
                    line.extend(cell.iter().cloned());
                    let pad = (cols[k] - cell_width(cell)).max(0) as usize;
                    line.push(Span::plain(" ".repeat(pad)));
                }
                line_rows(cache, &line)
                    .into_iter()
                    .map(|r| r.trim_end().to_owned())
                    .collect()
            })
            .collect(),
    )
}

/// Without the `math` feature a formula is its source: the compartment is its text rows.
#[cfg(not(feature = "math"))]
pub(super) fn section_rows(_: &Rows, _: &[crate::flowchart::Row]) -> Option<Vec<Vec<String>>> {
    None
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
    fn of(span: &Span, charset: Charset) -> Block {
        match span.ascii_rows(charset) {
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
    fn every_row_of_a_line_is_as_wide_as_the_block_in_every_set() {
        for set in [Charset::Unicode, Charset::Ascii, Charset::UnicodeMath] {
            for src in [
                r"$$\sqrt[3]{x^2+1}$$",
                r"$$\frac{\alpha}{\sum_{i=1}^n x_i}$$",
                r"$$\left(\frac{a}{b}\right)$$",
                r"$$\hat{x}\xrightarrow{f}\overline{yz}\pm\infty$$",
            ] {
                let cache = Rows::new(set);
                let rows = line_rows(&cache, &[formula(src)]);
                let b = Block::of(&formula(src), set);
                assert!(rows.len() > 1, "{src}");
                for r in &rows {
                    assert_eq!(width(r), b.width, "{src} {set:?} {r:?}");
                }
            }
        }
    }

    #[test]
    fn a_formula_with_nothing_to_draw_is_no_width_not_its_source() {
        for src in ["$$$$", r"$$\phantom{x}$$", r"$$\,$$"] {
            let b = Block::of(&formula(src), Charset::Unicode);
            assert_eq!(b.rows, [""], "{src}");
            assert_eq!(b.width, 0, "{src}");
            let got = rows(&Rows::default(), &label(&format!("a {src} b"))).unwrap();
            assert_eq!(got, ["a  b"], "{src}");
        }
    }

    #[test]
    fn only_a_formula_that_does_not_typeset_is_drawn_as_source() {
        let b = Block::of(&formula(r"$$\foo$$"), Charset::Unicode);
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
        assert_eq!(cache.cache.borrow().len(), 2);
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

    /// Lowers `src`, renders it as text, and returns the text and how many formulas were set.
    fn counted(src: &str) -> (String, usize) {
        use crate::diagram::{self, Diagram};
        let options = super::super::Options::default();
        crate::equation::count::reset();
        let out = match diagram::lower(src).diagram {
            Diagram::Block(b) => super::super::block::render(&b, &options),
            Diagram::C4(c) => super::super::c4::render(&c, &options),
            Diagram::Architecture(a) => super::super::architecture::render(&a, &options),
            _ => String::new(),
        };
        (out, crate::equation::count::get())
    }

    /// The block, C4, and architecture renderers measure and draw with one cache, so a formula is
    /// set once per render, not once per call.
    #[test]
    fn block_c4_and_architecture_set_each_formula_once_per_render() {
        for src in [
            "block-beta\n    a[\"$$x^2$$\"] b\n    a --> b\n",
            "C4Context\n    Person(p, \"$$y^2$$\")\n    System(s, \"S\")\n    Rel(p, s, \"uses\")\n",
            "architecture-beta\n    service a(server)[\"$$z^2$$\"]\n    service b[B]\n    a:R --> L:b\n",
        ] {
            let (out, sets) = counted(src);
            assert_eq!(sets, 1, "{src}\n{out}");
            assert!(!out.contains("$$"), "{src}\n{out}");
        }
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
        assert_eq!(cache.cache.borrow().len(), 0);
    }
}
