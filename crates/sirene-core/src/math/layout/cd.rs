//! The labels of a commutative diagram's vertical arrows.
//!
//! Adapted from KaTeX's `src/environments/cd.ts` (the `cdlabel` and `cdlabelparent` builders; MIT,
//! Copyright (c) 2013-2020 Khan Academy and other contributors). The table itself is laid out by
//! `array`, with the rows `3ex` apart, and the horizontal arrows are `arrow`'s `\cd...` arrows.
//!
//! KaTeX leaves the place of a label to its style sheet: the arrow is an inline block, and each
//! label is taken out of the flow beside it, `0.3em` from the arrow's middle (in the label's
//! own em) and with its bottom edge `0.8em - depth` above the arrow's bottom edge. The label then
//! has no width, height, or depth, so the row is as tall as the arrow. This builds that: the
//! labels are placed against the arrow with no size of their own. A box has no edge but its
//! baseline and depth, so a label's bottom edge is its depth below its baseline.
//!
//! KaTeX's box leaves a label that hangs past the diagram's right edge outside it, as the label
//! overflows in the browser. `array` widens the diagram's box to take it in (nothing moves), so
//! that a formula's ink stays inside its box.

use super::{Atom, Built, build, build_in};
use crate::math::{
    MathError,
    ast::{CdSide, Node, NodeKind},
    boxes::Bx,
    options::{Em, Options},
};

/// The distance of a label from the middle of its arrow, in the label's em.
const LABEL_GAP: f64 = 0.3;

/// The height above an arrow's bottom edge at which a label's baseline sits when it has no
/// depth, in the label's em.
const LABEL_RISE: f64 = 0.8;

/// A label built in the style a superscript has, sized for `options`, with the size its own em
/// has in those of `options`.
fn label_box<'t>(label: &'t Node<'_>, options: &Options<'t>) -> Result<(Bx<'t>, f64), MathError> {
    let inner = options.having_style(options.style.sup());
    let scale = inner.size_multiplier() / options.size_multiplier();
    let bx = build_in(label, &inner, options)?.into_box();
    Ok((bx, scale))
}

/// The label alone, with no size: where a label has no arrow to sit beside, it takes no room.
pub(super) fn cd_label<'t>(
    label: &'t Node<'_>,
    options: &Options<'t>,
) -> Result<Built<'t>, MathError> {
    let (mut bx, _) = label_box(label, options)?;
    bx.width = Em::ZERO;
    bx.height = Em::ZERO;
    bx.depth = Em::ZERO;
    Ok(Built::One(Atom::new(bx, None, options)))
}

/// A vertical arrow with its labels, which sit beside it and add nothing to its size.
pub(super) fn cd_label_parent<'t>(
    fragment: &'t Node<'_>,
    options: &Options<'t>,
) -> Result<Built<'t>, MathError> {
    // The fragment is the left label, the arrow, and the right label; anything else in it
    // is the arrow.
    let nodes: &'t [Node<'_>] = match &fragment.kind {
        NodeKind::Group { body, .. } => body,
        _ => std::slice::from_ref(fragment),
    };
    let mut arrow = Vec::new();
    let mut labels = Vec::new();
    for node in nodes {
        match &node.kind {
            NodeKind::CdLabel { side, label } => labels.push((*side, label_box(label, options)?)),
            _ => arrow.push(build(node, options)?.into_box()),
        }
    }
    let arrow = Bx::hlist(arrow);
    let (width, height, depth) = (arrow.width, arrow.height, arrow.depth);

    let mut children = vec![(arrow, Em::ZERO, Em::ZERO)];
    for (side, (mut label, scale)) in labels {
        let label_width = label.width;
        // The baseline is `0.8em - depth` (the label's em) above the arrow's bottom edge, and the
        // label's own bottom edge is its depth below it.
        let raise = label.depth - depth + (Em(LABEL_RISE) - label.depth) * scale;
        let gap = Em(LABEL_GAP * scale);
        let dx = match side {
            CdSide::Left => width / 2.0 - gap - label_width,
            CdSide::Right => width / 2.0 + gap,
        };
        label.width = Em::ZERO;
        label.height = Em::ZERO;
        label.depth = Em::ZERO;
        children.push((label, -raise, dx));
    }
    let mut bx = Bx::vlist_shifted(children);
    // The labels are taken out of the flow: the arrow alone sizes the box.
    bx.width = width;
    bx.height = height;
    bx.depth = depth;
    Ok(Built::One(Atom::new(bx, None, options)))
}

#[cfg(test)]
mod tests {
    use crate::math::{
        boxes::{Bx, Item},
        layout::layout,
        parser::{Settings, parse_with},
    };

    fn laid_out(src: &str) -> Bx<'static> {
        // The box borrows the source and the nodes; leaking them keeps it usable after the call.
        let src: &'static str = Box::leak(src.to_string().into_boxed_str());
        let nodes = parse_with(src, Settings { display_mode: true }).unwrap();
        let nodes: &'static [_] = Box::leak(nodes.into_boxed_slice());
        layout(nodes, true).unwrap()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-4
    }

    /// Height and depth are KaTeX's, from the oracle fixture
    /// `tests/fixtures/math/katex-oracle.json`.
    #[test]
    fn a_diagram_is_as_tall_as_katex_makes_it() {
        let bx = laid_out(
            "\\begin{CD} A @<a<< B @>>b> C \\\\ @| @AcAA @VVdV \\\\ D @= E @>>> F \\end{CD}",
        );
        assert!(close(bx.height.0, 3.65926), "{}", bx.height.0);
        assert!(close(bx.depth.0, 1.86626), "{}", bx.depth.0);
    }

    fn extent(src: &str) -> f64 {
        let bx = laid_out(src);
        bx.height.0 + bx.depth.0
    }

    #[test]
    fn each_row_adds_three_ex_and_a_jot() {
        // A row is a strut of 3ex (0.431em each) and the jot of 3pt (0.3em) that the rows before
        // the last take; the cells are lower than the strut.
        // (A row of arrows between rows of cells, with a blank for each arrow.)
        let two = extent("\\begin{CD}A\\\\@.\\\\B\\end{CD}");
        let three = extent("\\begin{CD}A\\\\@.\\\\B\\\\@.\\\\C\\end{CD}");
        // Two more rows.
        assert!(
            close(three - two, 2.0 * (3.0 * 0.431 + 0.3)),
            "{two} {three} {}",
            three - two
        );
    }

    #[test]
    fn a_vertical_arrow_is_sized_by_the_arrow_alone() {
        let plain = laid_out("\\begin{CD}A\\\\@VVV\\\\B\\end{CD}");
        let labeled = laid_out("\\begin{CD}A\\\\@VabcdVxyzV\\\\B\\end{CD}");
        // (Only the diagram's right edge grows, to take in a label that overhangs it; see
        // `a_label_past_the_last_column_widens_the_diagram_but_moves_nothing`.)
        assert_eq!(plain.height, labeled.height);
        assert_eq!(plain.depth, labeled.depth);
    }

    /// The position of the first glyph that draws `text`.
    fn glyph_at(bx: &Bx<'_>, text: &str) -> (f64, f64) {
        let found = bx.items().into_iter().find_map(|item| match item {
            Item::Glyph { glyph, x, y, .. } if glyph.text == text => Some((x, y)),
            _ => None,
        });
        found.unwrap_or_else(|| panic!("no glyph {text}"))
    }

    #[test]
    fn a_label_past_the_last_column_widens_the_diagram_but_moves_nothing() {
        let plain = laid_out("\\begin{CD}A\\\\@VVV\\\\B\\end{CD}");
        let labeled = laid_out("\\begin{CD}A\\\\@VVdddV\\\\B\\end{CD}");
        // The label is 0.3em (script em) right of the arrow's middle and 3 d's wide.
        let (d, _) = glyph_at(&labeled, "d");
        let width = laid_out("\\scriptstyle ddd").width.0;
        assert!(
            (labeled.width.0 - (d + width)).abs() < 0.01,
            "{} {d} {width}",
            labeled.width.0
        );
        assert!(labeled.width.0 > plain.width.0 + 0.3);
        // The glyph of the cell is where it was.
        assert_eq!(glyph_at(&plain, "A"), glyph_at(&labeled, "A"));
        // A label inside the column adds nothing: a wide cell holds it.
        let held = laid_out("\\begin{CD}AAAAAAAA\\\\@VVdV\\\\B\\end{CD}");
        let bare = laid_out("\\begin{CD}AAAAAAAA\\\\@VVV\\\\B\\end{CD}");
        assert_eq!(held.width, bare.width);
    }

    #[test]
    fn labels_sit_either_side_of_the_arrow() {
        let bx = laid_out("\\begin{CD}A\\\\@VlVrV\\\\B\\end{CD}");
        let ((left, _), (right, _)) = (glyph_at(&bx, "l"), glyph_at(&bx, "r"));
        // The arrow is the middle of the one column, and a label is 0.3em of its own (script
        // size) em from it.
        let middle = laid_out("\\begin{CD}A\\\\@VVV\\\\B\\end{CD}").width.0 / 2.0;
        assert!(bx.width.0 >= middle * 2.0);
        let gap = 0.3 * 0.7;
        assert!(close(right, middle + gap), "{right} {middle}");
        // The left label ends there, on the other side.
        let width = laid_out("\\scriptstyle l").width.0;
        assert!(close(left + width, middle - gap), "{left} {width} {middle}");
    }

    #[test]
    fn a_label_rises_by_its_depth() {
        // Its bottom edge is `0.8em - depth` above the arrow's, so a label with a depth (`g`)
        // has its baseline higher than one without (`l`) by that depth, less the 0.7 of it that
        // the label's own em takes back.
        let bx = laid_out("\\begin{CD}A\\\\@VgVlV\\\\B\\end{CD}");
        let ((_, g), (_, l)) = (glyph_at(&bx, "g"), glyph_at(&bx, "l"));
        let depth = 0.19444 * 0.7;
        assert!(close(l - g, depth * (1.0 - 0.7)), "{g} {l}");
    }

    #[test]
    fn horizontal_arrows_are_at_least_three_em() {
        let bx = laid_out("\\begin{CD}A @>>> B\\end{CD}");
        let (a, b) = (laid_out("A").width.0, laid_out("B").width.0);
        // Half an enskip on each side of each of the two edges between the columns' contents.
        assert!(
            close(bx.width.0, a + b + 3.0 + 4.0 * 0.25),
            "{}",
            bx.width.0
        );
        // A label wider than that stretches the arrow.
        let wide = laid_out("\\begin{CD}A @>xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx>> B\\end{CD}");
        assert!(wide.width.0 > bx.width.0 + 1.0, "{}", wide.width.0);
    }

    #[test]
    fn odd_diagrams_are_laid_out() {
        for src in [
            "\\begin{CD}\\end{CD}",
            "\\begin{CD}@>>>\\end{CD}",
            "\\begin{CD}@VVV\\end{CD}",
            "\\begin{CD}@. @. @.\\\\@VVV\\end{CD}",
            "\\begin{CD}A @= B @| C @. D\\\\@VaVbV @AcAdA @VVV @. \\end{CD}",
        ] {
            let nodes = parse_with(src, Settings { display_mode: true }).unwrap();
            assert!(layout(&nodes, true).is_ok(), "{src}");
        }
    }

    #[test]
    fn a_huge_diagram_is_laid_out() {
        let row = "A @>>> B ".repeat(1000);
        let rest = "@VaVbV @. ".repeat(1000);
        let src = format!("\\begin{{CD}}{row}\\\\{rest}\\end{{CD}}");
        let bx = laid_out(&src);
        assert!(bx.width.0.is_finite() && bx.width.0 > 1000.0);
    }
}
