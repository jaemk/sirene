//! Tables and line breaks: `array`, the matrices, `cases`, the aligned environments, `\\`
//! outside a table, and equation tags.
//!
//! Adapted from KaTeX's `src/environments/array.ts` (the HTML builder), `src/functions/cr.ts`,
//! and the tag handling of `src/buildHTML.ts` (MIT, Copyright (c) 2013-2020 Khan Academy and
//! other contributors).
//!
//! Where KaTeX leaves a job to CSS, this builds boxes instead:
//! - A column holds its cells at the left edge of a stack, shifted by half the slack for `c` and
//!   all of it for `r`.
//! - A dashed rule (`\hdashline`, `:` between columns) is a run of short rules, each three times
//!   as long as the rule is thick, with gaps of the same length.
//! - An equation tag follows the table (or the formula) on the baseline's side, where KaTeX's
//!   style sheet pins it to the right edge of the page and `leqno` moves it to the left. It
//!   follows with no space between; moving it to the edge waits for the formula's own type,
//!   which will carry it apart. Numbers made by `{align}` and kin are CSS counters, so a row numbered
//!   automatically has an empty tag box of the row's size.
//! - The extra space `\\[1em]` puts above a line break at the top level is a margin, which a
//!   box does not have, so that break is an empty box.
//! - `{CD}` is an array of this tree whose baseline skip is `3ex`; its arrows and labels are
//!   built by `arrow` and `cd`.

use std::ops::Range;

use super::{Atom, Built, Class, Grouping, build, expression};
use crate::math::{
    MathError,
    ast::{AlignSpec, Array, ColSeparation, Measurement, Node, RowTag},
    boxes::{Bx, Part, Position},
    options::{Em, Options},
    style::Style,
    symbols::Mode,
};

/// A row of the table once its cells are built: its size, and where its baseline is below the
/// top of the table.
struct Row<'t> {
    cells: Vec<Option<Bx<'t>>>,
    height: f64,
    depth: f64,
    pos: f64,
}

/// A table: the rows and columns of `array`, laid out as KaTeX's HTML builder does.
pub(super) fn array<'t>(
    array: &'t Array<'_>,
    _mode: Mode,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let metrics = options.metrics();
    // From LaTeX's `\arrayrulewidth`, which is 0.04em.
    let rule_thickness = metrics.array_rule_width.max(options.min_rule_thickness.0);

    // Horizontal spacing.
    let pt = 1.0 / metrics.pt_per_em;
    // `\arraycolsep` of article.cls.
    let mut arraycolsep = 5.0 * pt;
    if array.col_separation == Some(ColSeparation::Small) {
        // `{smallmatrix}` separates by `\thickspace`, 5/18em, less what LaTeX applies as
        // `\scriptstyle` to the whole table and this applies only inside each cell.
        let local = options.having_style(Style::Script).size_multiplier();
        arraycolsep = 0.2778 * (local / options.size_multiplier());
    }

    // Vertical spacing, from size10.clo: `\baselineskip`, `\jot`, and the strut of each row.
    let baselineskip = if array.col_separation == Some(ColSeparation::Cd) {
        // A commutative diagram's rows are `3ex` apart.
        let skip = Measurement {
            number: 3.0,
            unit: "ex".to_string(),
        };
        options.length(&skip, span.clone())?.0
    } else {
        12.0 * pt
    };
    let jot = 3.0 * pt;
    let arrayskip = array.arraystretch * baselineskip;
    let strut_height = 0.7 * arrayskip;
    let strut_depth = 0.3 * arrayskip;

    let mut total = 0.0;
    // The `\hline`s as (distance below the top of the table, dashed).
    let mut hlines: Vec<(f64, bool)> = Vec::new();
    place_hlines(&mut total, array.hlines_before_row.first(), &mut hlines);

    let mut rows: Vec<Row<'t>> = Vec::with_capacity(array.body.len());
    let mut columns = 0;
    for (r, in_row) in array.body.iter().enumerate() {
        let mut height = strut_height;
        let mut depth = strut_depth;
        columns = columns.max(in_row.len());
        let mut cells = Vec::with_capacity(in_row.len());
        for node in in_row {
            let cell = build(node, options)?.into_box();
            depth = depth.max(cell.depth.0);
            height = height.max(cell.height.0);
            cells.push(Some(cell));
        }

        let mut gap = 0.0;
        if let Some(Some(row_gap)) = array.row_gaps.get(r) {
            gap = options.length(row_gap, span.clone())?.0;
            // `\@argarraycr`: space below the row is a minimum depth.
            if gap > 0.0 {
                gap += strut_depth;
                depth = depth.max(gap);
                gap = 0.0;
            }
        }
        // The lines of an AMS multiline environment are `\jot` further apart, which is added as
        // depth to each row but the last.
        if array.add_jot && r + 1 < array.body.len() {
            depth += jot;
        }

        total += height;
        let pos = total;
        total += depth + gap;
        rows.push(Row {
            cells,
            height,
            depth,
            pos,
        });
        place_hlines(&mut total, array.hlines_before_row.get(r + 1), &mut hlines);
    }

    let offset = total / 2.0 + metrics.axis_height;

    // The tag column: one box per row, as tall as the row, when any row has a tag.
    let has_tags = array.tags.as_ref().is_some_and(|tags| {
        tags.iter().any(|tag| match tag {
            RowTag::Numbered(numbered) => *numbered,
            RowTag::Custom(_) => true,
        })
    });
    let tag_column = if has_tags {
        let mut shifted = Vec::with_capacity(rows.len());
        for (r, row) in rows.iter().enumerate() {
            let mut tag = match array.tags.as_deref().and_then(|tags| tags.get(r)) {
                Some(RowTag::Custom(nodes)) => {
                    let atoms = expression(nodes, options, Grouping::Group)?;
                    Bx::hlist(atoms.into_iter().map(|a| a.bx).collect())
                }
                _ => Bx::space(Em::ZERO, Em::ZERO, Em::ZERO),
            };
            tag.height = Em(row.height);
            tag.depth = Em(row.depth);
            shifted.push((tag, Em(row.pos - offset), Em::ZERO));
        }
        Some(Bx::vlist_shifted(shifted))
    } else {
        None
    };

    let descriptions: &[AlignSpec] = array.cols.as_deref().unwrap_or(&[]);
    let mut cols: Vec<Bx<'t>> = Vec::new();
    let (mut c, mut d) = (0, 0);
    // Continue while either columns or column descriptions remain, so that trailing separators
    // are kept.
    while c < columns || d < descriptions.len() {
        let mut description = descriptions.get(d);
        let mut first = true;
        while let Some(AlignSpec::Separator(separator)) = description {
            // Two separators in a row have a space between them.
            if !first {
                cols.push(Bx::kern(Em(metrics.double_rule_sep)));
            }
            let dashed = match separator {
                '|' => false,
                ':' => true,
                other => {
                    return Err(MathError::new(
                        format!("Invalid separator type: {other}"),
                        span,
                    ));
                }
            };
            cols.push(vertical_rule(
                dashed,
                (rule_thickness, offset, total),
                options,
            ));
            d += 1;
            description = descriptions.get(d);
            first = false;
        }

        if c < columns {
            let (align, pregap, postgap) = match description {
                Some(AlignSpec::Align {
                    align,
                    pregap,
                    postgap,
                }) => (*align, *pregap, *postgap),
                _ => ('c', None, None),
            };

            if c > 0 || array.hskip_before_and_after == Some(true) {
                let gap = pregap.unwrap_or(arraycolsep);
                if gap != 0.0 {
                    cols.push(Bx::kern(Em(gap)));
                }
            }

            let mut cells = Vec::new();
            for row in &mut rows {
                let Some(mut cell) = row.cells.get_mut(c).and_then(Option::take) else {
                    continue;
                };
                // The cell takes the size of its row.
                cell.depth = Em(row.depth);
                cell.height = Em(row.height);
                cells.push((cell, Em(row.pos - offset)));
            }
            let width = cells
                .iter()
                .fold(0.0_f64, |w, (cell, _)| w.max(cell.width.0));
            let placed = cells
                .into_iter()
                .map(|(cell, shift)| {
                    let slack = width - cell.width.0;
                    let dx = match align {
                        'l' => 0.0,
                        'r' => slack,
                        _ => slack / 2.0,
                    };
                    (cell, shift, Em(dx))
                })
                .collect();
            cols.push(Bx::vlist_shifted(placed));

            if c + 1 < columns || array.hskip_before_and_after == Some(true) {
                let gap = postgap.unwrap_or(arraycolsep);
                if gap != 0.0 {
                    cols.push(Bx::kern(Em(gap)));
                }
            }
        }
        c += 1;
        d += 1;
    }

    let mut table = Bx::hlist(cols);

    // The `\hline`s, each a line as wide as the table at its place in the stack.
    if !hlines.is_empty() {
        let width = table.width;
        let mut children = vec![(table, Em::ZERO, Em::ZERO)];
        while let Some((pos, dashed)) = hlines.pop() {
            let line = if dashed {
                dashed_line(width, rule_thickness, options)
            } else {
                Bx::line(width, Em(rule_thickness), options)
            };
            children.push((line, Em(pos - offset), Em::ZERO));
        }
        table = Bx::vlist_shifted(children);
    }

    match tag_column {
        None => Ok(Built::One(Atom::new(table, Some(Class::Ord), options))),
        Some(tags) => Ok(Built::Many(vec![
            Atom::new(table, None, options),
            Atom::new(tags, None, options),
        ])),
    }
}

/// Gives each `\hline` in a gap a place: the first at the current height, the others a quarter
/// em lower each.
fn place_hlines(total: &mut f64, gap: Option<&Vec<bool>>, hlines: &mut Vec<(f64, bool)>) {
    for (i, dashed) in gap.into_iter().flatten().enumerate() {
        if i > 0 {
            *total += 0.25;
        }
        hlines.push((*total, *dashed));
    }
}

/// A rule between columns: `thickness` wide, centered on the column edge so that it takes no
/// width, from `offset` above the baseline to `total - offset` below it.
fn vertical_rule<'t>(
    dashed: bool,
    (thickness, offset, total): (f64, f64, f64),
    options: &Options<'t>,
) -> Bx<'t> {
    let line = if dashed {
        // The stack runs bottom to top, so the runs are laid from the bottom.
        let parts = dash_runs(total, thickness)
            .into_iter()
            .rev()
            .map(|(length, dash)| {
                if dash {
                    Part::of(Bx::rule(Em(thickness), Em(length), Em::ZERO, options))
                } else {
                    Part::Kern(Em(length))
                }
            })
            .collect();
        Bx::vlist(Position::Bottom(Em(total - offset)), parts)
    } else {
        Bx::rule(Em(thickness), Em(offset), Em(total - offset), options)
    };
    let half = Em(-thickness / 2.0);
    Bx::hlist(vec![Bx::kern(half), line, Bx::kern(half)])
}

/// The runs of a dashed rule of a thickness and a length, from its start: `(length, is_dash)`.
/// A dash and the gap after it are each three times the thickness, and the rule ends in a dash
/// that takes up what is left. A rule longer than `MAX_DASHES` steps takes longer dashes instead
/// of more of them (KaTeX's CSS dashes cost the same at any length), and a length that is not
/// finite is one run.
fn dash_runs(length: f64, thickness: f64) -> Vec<(f64, bool)> {
    const MAX_DASHES: f64 = 1000.0;
    let mut left = if length.is_finite() {
        length.max(0.0)
    } else {
        0.0
    };
    let step = (3.0 * thickness).max(left / (2.0 * MAX_DASHES));
    let mut runs = Vec::new();
    while left > 2.0 * step && step > 0.0 {
        runs.push((step, true));
        runs.push((step, false));
        left -= 2.0 * step;
    }
    runs.push((left, true));
    runs
}

/// `\hdashline`: short rules across `width`.
fn dashed_line<'t>(width: Em, thickness: f64, options: &Options<'t>) -> Bx<'t> {
    let parts = dash_runs(width.0, thickness)
        .into_iter()
        .map(|(length, dash)| {
            if dash {
                Bx::rule(Em(length), Em(thickness), Em::ZERO, options)
            } else {
                Bx::kern(Em(length))
            }
        })
        .collect();
    Bx::hlist(parts)
}

/// `\\` outside a table: a line break, with `size` of extra space below it.
///
/// KaTeX builds an empty `mspace` here, which its style sheet gives the line break and the
/// margin, so this is an empty space that spacing passes over.
pub(super) fn cr<'t>(
    new_line: bool,
    size: Option<&'t Measurement>,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    // KaTeX measures the size when it is a newline, so an unknown unit fails here too.
    if let (true, Some(size)) = (new_line, size) {
        options.length(size, span)?;
    }
    let mut atom = Atom::space(Bx::space(Em::ZERO, Em::ZERO, Em::ZERO), options);
    atom.newline = new_line;
    Ok(Built::One(atom))
}

/// A display formula with an equation tag at its right. A tag node is only ever the whole
/// formula (KaTeX's `buildHTML` takes it apart there), so its body is built as the root. The
/// tag follows the formula with no space between; KaTeX's style sheet pins it to the right
/// edge of the page, which is the caller's to do.
pub(super) fn tag<'t>(
    body: &'t [Node<'_>],
    tag: &'t [Node<'_>],
    options: &Options<'t>,
    _span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let mut boxes: Vec<Bx<'t>> = expression(body, options, Grouping::Root)?
        .into_iter()
        .map(|atom| atom.bx)
        .collect();
    let tag = expression(tag, options, Grouping::Group)?
        .into_iter()
        .map(|atom| atom.bx)
        .collect();
    boxes.push(Bx::hlist(tag));
    Ok(Built::One(Atom::new(Bx::hlist(boxes), None, options)))
}

#[cfg(test)]
mod tests {
    use crate::math::boxes::{Bx, Item};
    use crate::math::layout::layout;
    use crate::math::parser::{Settings, parse_with};

    fn laid_out(src: &str, display: bool) -> Bx<'static> {
        // The box borrows the source and the nodes; leaking them keeps it usable after the call.
        let src: &'static str = Box::leak(src.to_string().into_boxed_str());
        let nodes = parse_with(
            src,
            Settings {
                display_mode: display,
            },
        )
        .unwrap();
        let nodes: &'static [_] = Box::leak(nodes.into_boxed_slice());
        layout(nodes, display).unwrap()
    }

    fn hd(src: &str, display: bool) -> (f64, f64) {
        let bx = laid_out(src, display);
        (bx.height.0, bx.depth.0)
    }

    fn width(src: &str) -> f64 {
        laid_out(src, false).width.0
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-4
    }

    /// Height and depth are KaTeX's, from the oracle fixture.
    fn agrees(src: &str, display: bool, expected: (f64, f64)) {
        let got = hd(src, display);
        assert!(
            close(got.0, expected.0) && close(got.1, expected.1),
            "{src:?}: {got:?} vs {expected:?}"
        );
    }

    #[test]
    fn matrix_rows_are_a_strut_apart() {
        agrees(
            "\\begin{matrix}a&b\\\\c&d\\end{matrix}",
            false,
            (1.45, 0.95),
        );
        agrees(
            "\\begin{array}{cc}a&b\\\\c&d\\end{array}",
            false,
            (1.45, 0.95),
        );
        agrees("\\begin{matrix}\\end{matrix}", false, (0.0, 0.0));
    }

    #[test]
    fn matrix_columns_are_a_colsep_apart() {
        // Each of two columns takes the width of its widest cell, with 5pt on each side of the
        // edge between them and none at the outside.
        let w = width("\\begin{matrix}a&b\\\\ccc&d\\end{matrix}");
        assert!(close(w, width("ccc") + 1.0 + width("d")), "{w}");
        // `{array}` also has them at the outside.
        let w = width("\\begin{array}{cc}a&b\\\\ccc&d\\end{array}");
        assert!(close(w, width("ccc") + 2.0 + width("d")), "{w}");
    }

    #[test]
    fn columns_align_left_center_and_right() {
        let x_of = |align: char| {
            let src = format!("\\begin{{array}}{{{align}}}a\\\\bbb\\end{{array}}");
            let bx = laid_out(&src, false);
            let items = bx.items();
            match items.first() {
                Some(Item::Glyph { x, .. }) => *x,
                other => panic!("a glyph first: {other:?}"),
            }
        };
        let slack = width("bbb") - width("a");
        assert!(close(x_of('l'), 0.5), "{}", x_of('l'));
        assert!(close(x_of('c'), 0.5 + slack / 2.0));
        assert!(close(x_of('r'), 0.5 + slack));
    }

    #[test]
    fn hlines_add_their_thickness_above() {
        agrees(
            "\\begin{matrix}\\hline a&b\\\\ \\hline c&d\\end{matrix}",
            false,
            (1.49, 0.95),
        );
        agrees(
            "\\begin{matrix}\\hdashline a&b\\\\ \\hdashline c&d\\end{matrix}",
            false,
            (1.49, 0.95),
        );
        // The line is as wide as the table: a rule of the table's width.
        let bx = laid_out("\\begin{matrix}a&b\\\\ \\hline c&d\\end{matrix}", false);
        let widths: Vec<f64> = bx
            .items()
            .iter()
            .filter_map(|item| match item {
                Item::Rule { width, height, .. } => Some((*width, *height)),
                _ => None,
            })
            .map(|(w, h)| {
                assert!(close(h, 0.04), "{h}");
                w
            })
            .collect();
        assert_eq!(widths.len(), 1);
        assert!(close(widths[0], bx.width.0), "{widths:?}");
    }

    #[test]
    fn dashed_lines_are_runs_of_short_rules() {
        let bx = laid_out("\\begin{matrix}a&b\\\\ \\hdashline c&d\\end{matrix}", false);
        let rules: Vec<(f64, f64)> = bx
            .items()
            .iter()
            .filter_map(|item| match item {
                Item::Rule { x, width, .. } => Some((*x, *width)),
                _ => None,
            })
            .collect();
        assert!(rules.len() > 3, "{rules:?}");
        let end = rules.iter().map(|(x, w)| x + w).fold(0.0, f64::max);
        assert!(close(end, bx.width.0), "{end}");
    }

    #[test]
    fn long_dashed_rules_have_a_bounded_number_of_dashes() {
        let runs = super::dash_runs(1e9, 0.04);
        assert!(runs.len() <= 2001, "{}", runs.len());
        let total: f64 = runs.iter().map(|(length, _)| length).sum();
        assert!((total - 1e9).abs() < 1e-3, "{total}");
        assert_eq!(super::dash_runs(f64::INFINITY, 0.04), vec![(0.0, true)]);
        assert_eq!(super::dash_runs(f64::NAN, 0.04), vec![(0.0, true)]);
    }

    #[test]
    fn vertical_rules_take_no_width() {
        let plain = width("\\begin{array}{cc}a&b\\end{array}");
        assert!(close(width("\\begin{array}{c|c}a&b\\end{array}"), plain));
        assert!(close(width("\\begin{array}{c:c}a&b\\end{array}"), plain));
        // A rule is as tall as the table.
        let bx = laid_out("\\begin{array}{c|c}a&b\\\\c&d\\end{array}", false);
        let tall = bx.items().into_iter().any(|item| match item {
            Item::Rule { height, width, .. } => close(height, 2.4) && close(width, 0.04),
            _ => false,
        });
        assert!(tall);
        // Two in a row are a `\doublerulesep` apart.
        let double = width("\\begin{array}{c||c}a&b\\end{array}");
        assert!(close(double, plain + 0.2), "{double} {plain}");
    }

    #[test]
    fn aligned_rows_are_a_jot_further_apart() {
        agrees("\\begin{aligned}\\end{aligned}", false, (0.85, 0.35));
        agrees(
            "\\begin{aligned}a&=b&c&=d\\\\e&=f\\end{aligned}",
            false,
            (1.6, 1.1),
        );
        agrees(
            "\\begin{alignedat}{3}a &= 1 & b &= 2 &\\quad c &= 3\\\\3a &= 3 & 17b &= 34 &\\quad 400c &= 1200\\end{alignedat}",
            false,
            (1.6, 1.1),
        );
    }

    #[test]
    fn smallmatrix_is_script_sized() {
        agrees(
            "\\begin{smallmatrix} a & b \\\\ c & d \\end{smallmatrix}",
            false,
            (0.916108, 0.416108),
        );
        // Columns are 0.2778em apart in the script size's em (0.7), twice, at the text size.
        let w = width("\\begin{smallmatrix}a&b\\end{smallmatrix}");
        let letters = 0.7 * (width("a") + width("b"));
        let sep = 2.0 * 0.2778 * (0.7 / 1.0);
        assert!(close(w, letters + sep), "{w} {letters} {sep}");
    }

    #[test]
    fn row_gaps_add_depth_below_a_row() {
        // Worked from array.ts: the row is 0.84 high and takes 1em + 0.36 as depth, so the table
        // is 0.84 + 1.36 + 0.84 + 0.36 = 3.4 high and its axis offset 1.7 + 0.25.
        agrees(
            "\\begin{matrix}a\\\\[1em]b\\end{matrix}",
            false,
            (1.95, 1.45),
        );
        // A gap adds to the strut's depth (0.36) and is the row's depth when that is more.
        agrees(
            "\\begin{matrix}a\\\\[0.1em]b\\end{matrix}",
            false,
            (1.5, 1.0),
        );
        agrees(
            "\\begin{matrix}a\\\\[0pt]b\\end{matrix}",
            false,
            (1.45, 0.95),
        );
        // A negative one pulls the next row up.
        agrees(
            "\\begin{matrix}a\\\\[-1em]b\\end{matrix}",
            false,
            (0.95, 0.45),
        );
    }

    #[test]
    fn rows_of_unequal_length_leave_cells_out() {
        agrees(
            "\\begin{matrix}0&1\\over2&3\\\\4&5&6\\end{matrix}",
            false,
            (1.452554, 0.952554),
        );
    }

    #[test]
    fn numbered_rows_have_a_tag_column() {
        agrees(
            "\\begin{gather} a=\\frac 1 2 \\\\ e=b+c \\end{gather}",
            true,
            (2.00372, 1.50372),
        );
        // A custom tag is set after the table, as wide as it is.
        let plain = laid_out("\\begin{gather*} a \\\\ b \\end{gather*}", true);
        let tagged = laid_out("\\begin{gather*} a \\tag{xyz} \\\\ b \\end{gather*}", true);
        assert!(close(
            tagged.width.0,
            plain.width.0 + width("\\text{(xyz)}")
        ));
        assert!(close(tagged.height.0, plain.height.0));
    }

    #[test]
    fn a_tag_follows_the_formula() {
        let (h, d) = hd("x\\tag{1}", true);
        // The tag is "(1)", whose parentheses are 0.75 high and 0.25 deep.
        assert!(close(h, 0.75) && close(d, 0.25), "{h} {d}");
        let bx = laid_out("x\\tag{12}", true);
        assert!(close(bx.width.0, width("x") + width("\\text{(12)}")));
        // The tag's own height counts.
        let (h, _) = hd("x\\tag{$\\frac12$}", true);
        assert!(h > 0.75, "{h}");
    }

    #[test]
    fn line_breaks_outside_a_table_are_empty() {
        let bx = laid_out("a\\\\b", false);
        assert!(close(bx.width.0, width("a") + width("b")));
        assert!(close(bx.height.0, 0.69444));
        // The size of `\\[1em]` is a margin, which a box does not have.
        let bx = laid_out("a\\\\[1em]b", false);
        assert!(close(bx.height.0, 0.69444) && close(bx.depth.0, 0.0));
    }

    #[test]
    fn a_line_break_starts_spacing_afresh() {
        // After a break, `-` starts a line: unary, with no space on either side.
        let bx = laid_out("a\\\\-b", false);
        assert!(close(bx.width.0, width("a") + width("-") + width("b")));
        // Before a break, `+` keeps its class and the space on its left, but none on its right.
        let medium = 4.0 / 18.0;
        let bx = laid_out("a+\\\\b", false);
        assert!(close(
            bx.width.0,
            width("a") + medium + width("-") + width("b")
        ));
        // Inside a group a break is a space like any other: `-` still has `a` on its left.
        let grouped = laid_out("{a\\\\-b}", false);
        assert!(close(
            grouped.width.0,
            width("a") + 2.0 * medium + width("-") + width("b")
        ));
    }
}
