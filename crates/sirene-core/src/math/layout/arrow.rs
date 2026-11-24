//! Extensible arrows: `\xrightarrow[below]{above}` and the other `\x...` arrows.
//!
//! Adapted from KaTeX's `src/functions/arrow.ts` (MIT, Copyright (c) 2013-2020 Khan Academy and
//! other contributors), with `stretchy` for the arrow shapes. The arrow is as wide as the wider
//! of its labels with their padding, and at least the arrow's minimum width.

use std::ops::Range;

use super::{Atom, Built, Class, build_in, stretchy};
use crate::math::{
    MathError,
    ast::Node,
    boxes::Bx,
    options::{Em, Options},
};

/// The space a label is padded by on each side: `\xrightarrow` pads by 0.5em, and the arrows of
/// a commutative diagram by `\;` before and `\;\;` after.
fn padded<'t>(label: Bx<'t>, x_arrow: bool) -> Bx<'t> {
    let (left, right) = if x_arrow {
        (0.5, 0.5)
    } else {
        (0.27778, 0.55556)
    };
    Bx::hlist(vec![Bx::kern(Em(left)), label, Bx::kern(Em(right))])
}

pub(super) fn x_arrow<'t>(
    label: &'t str,
    body: &'t Node<'_>,
    below: Option<&'t Node<'_>>,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let style = options.style;
    let x_arrow = label.starts_with("\\x");

    // The labels are in script and scriptscript style (amsmath: `\scriptstyle\mkern#3mu{#6}`).
    let upper_options = options.having_style(style.sup());
    let upper = padded(build_in(body, &upper_options, options)?.into_box(), x_arrow);
    let lower = match below {
        Some(below) => {
            let lower_options = options.having_style(style.sub());
            Some(padded(
                build_in(below, &lower_options, options)?.into_box(),
                x_arrow,
            ))
        }
        None => None,
    };

    let label_width = lower
        .as_ref()
        .map_or(upper.width, |lower| upper.width.max(lower.width));
    let arrow = stretchy::svg_span(label, None, label_width, options, span)?;
    let (width, arrow_height) = (arrow.width, arrow.height);

    // The arrow is centered on the axis, and the labels sit 2mu off it.
    let axis = Em(options.metrics().axis_height);
    let arrow_shift = -axis + arrow_height * 0.5;
    let mut upper_shift = -axis - arrow_height * 0.5 - Em(0.111);
    if upper.depth > Em(0.25) || label == "\\xleftequilibrium" {
        upper_shift -= upper.depth;
    }
    // Each label is centered over the arrow.
    let centered = |bx: &Bx<'_>| (width - bx.width) / 2.0;
    let upper_dx = centered(&upper);
    let mut children = vec![
        (upper, upper_shift, upper_dx),
        (arrow, arrow_shift, Em::ZERO),
    ];
    if let Some(lower) = lower {
        let lower_shift = -axis + lower.height + arrow_height * 0.5 + Em(0.111);
        let dx = centered(&lower);
        children.push((lower, lower_shift, dx));
    }
    let bx = Bx::vlist_shifted(children);
    Ok(Built::One(Atom::new(bx, Some(Class::Rel), options)))
}

#[cfg(test)]
mod tests {
    use super::super::layout;
    use crate::math::{
        boxes::{Align, Fit, Item},
        parser::{Settings, parse_with},
        svg_data,
    };

    /// A formula's size, and the rectangle, view box, fit, and data of each path in it.
    type Drawn = (
        (f64, f64, f64),
        Vec<([f64; 4], [f64; 4], Fit, String)>,
        Vec<(String, f64)>,
    );

    fn drawn(src: &str, display: bool) -> Drawn {
        let nodes = parse_with(
            src,
            Settings {
                display_mode: display,
            },
        )
        .unwrap();
        let bx = layout(&nodes, display).unwrap();
        let mut paths = Vec::new();
        let mut glyphs = Vec::new();
        for item in bx.items() {
            match item {
                Item::Path {
                    x,
                    y,
                    width,
                    height,
                    path,
                    ..
                } => paths.push((
                    [x, y, width, height],
                    path.view_box,
                    path.fit,
                    path.data.into_owned(),
                )),
                Item::Glyph { glyph, x, .. } => glyphs.push((glyph.text.into_owned(), x)),
                Item::Rule { .. } => {}
            }
        }
        ((bx.width.0, bx.height.0, bx.depth.0), paths, glyphs)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    // Heights and depths are KaTeX's, from the oracle fixture
    // `tests/fixtures/math/katex-oracle.json`; widths and positions are KaTeX's formulas worked
    // through with the font metrics.

    #[test]
    fn a_short_arrow_has_its_minimum_width() {
        let ((w, h, d), paths, glyphs) = drawn("\\xrightarrow{x}", false);
        assert!(close(h, 0.923392) && close(d, 0.011), "{h} {d}");
        assert!(close(w, 1.469), "{w}");
        let [([x, y, width, height], view_box, fit, data)] = paths.as_slice() else {
            panic!("one path: {paths:?}");
        };
        // The arrow is centered on the axis: its bottom 0.011em below the baseline.
        assert!(close(*x, 0.0) && close(*y, -0.511), "{x} {y}");
        assert!(close(*width, 1.469) && close(*height, 0.522));
        assert_eq!(*view_box, [0.0, 0.0, 400_000.0, 522.0]);
        assert_eq!(*fit, Fit::Crop(Align::Max));
        assert_eq!(data, svg_data::path("rightarrow").unwrap());
        // The label is centered over it, in script size with 0.5em of padding.
        let label = 0.57153 * 0.7;
        let [(text, at)] = glyphs.as_slice() else {
            panic!("one glyph: {glyphs:?}");
        };
        assert_eq!(text, "x");
        assert!(close(*at, (1.469 - (1.0 + label)) / 2.0 + 0.5), "{at}");
    }

    #[test]
    fn a_long_label_stretches_the_arrow() {
        let ((w, h, d), paths, _) = drawn("\\xrightarrow{xxxxxxxx}", false);
        let width = 1.0 + 8.0 * 0.57153 * 0.7;
        assert!(close(w, width), "{w}");
        assert!(close(h, 0.923392) && close(d, 0.011), "{h} {d}");
        let [([_, _, path_width, _], view_box, ..)] = paths.as_slice() else {
            panic!("one path: {paths:?}");
        };
        assert!(close(*path_width, width));
        // The path is still the same 400em view box, cropped to the width.
        assert_eq!(*view_box, [0.0, 0.0, 400_000.0, 522.0]);
    }

    #[test]
    fn a_left_arrow_with_a_label_below() {
        let ((_, h, d), paths, glyphs) = drawn("\\xleftarrow[under]{over}", false);
        assert!(close(h, 0.923392) && close(d, 0.608108), "{h} {d}");
        let [([x, _, width, _], _, fit, data)] = paths.as_slice() else {
            panic!("one path: {paths:?}");
        };
        assert_eq!(*fit, Fit::Crop(Align::Min));
        assert_eq!(data, svg_data::path("leftarrow").unwrap());
        assert!(close(*x, 0.0));
        assert!(*width > 1.469, "{width}");
        // The lower label is centered as the upper one is.
        let first = |text: &str| glyphs.iter().find(|(t, _)| t == text).map(|&(_, x)| x);
        assert!(first("u").is_some() && first("o").is_some(), "{glyphs:?}");
    }

    #[test]
    fn an_arrow_of_two_halves() {
        let (_, paths, _) = drawn("\\xleftrightarrow{x}", false);
        let [
            (left, _, left_fit, left_data),
            (right, _, right_fit, right_data),
        ] = paths.as_slice()
        else {
            panic!("two paths: {paths:?}");
        };
        assert_eq!(
            (*left_fit, *right_fit),
            (Fit::Crop(Align::Min), Fit::Crop(Align::Max))
        );
        assert_eq!(left_data, svg_data::path("leftarrow").unwrap());
        assert_eq!(right_data, svg_data::path("rightarrow").unwrap());
        // Each half is just over half of the 1.75em, the right one flush right.
        assert!(close(left[0], 0.0) && close(left[2], 0.502 * 1.75));
        assert!(close(right[0], 0.498 * 1.75) && close(right[2], 0.502 * 1.75));
        assert!(close(left[3], 0.522) && close(right[3], 0.522));
    }

    #[test]
    fn the_arrow_is_a_relation_and_sets_its_own_height() {
        // A relation takes thick spaces around it.
        let (arrow, ..) = drawn("\\xrightarrow{x}", false);
        let (both, ..) = drawn("a\\xrightarrow{x}a", false);
        let a = 0.52859;
        assert!(
            close(both.0, arrow.0 + 2.0 * a + 2.0 * 5.0 / 18.0),
            "{}",
            both.0
        );
    }

    #[test]
    fn every_arrow_label_has_an_image() {
        for name in [
            "xleftarrow",
            "xrightarrow",
            "xLeftarrow",
            "xRightarrow",
            "xleftrightarrow",
            "xLeftrightarrow",
            "xhookleftarrow",
            "xhookrightarrow",
            "xmapsto",
            "xrightharpoondown",
            "xrightharpoonup",
            "xleftharpoondown",
            "xleftharpoonup",
            "xrightleftharpoons",
            "xleftrightharpoons",
            "xlongequal",
            "xtwoheadrightarrow",
            "xtwoheadleftarrow",
            "xtofrom",
            "xrightleftarrows",
            "xrightequilibrium",
            "xleftequilibrium",
        ] {
            let src = format!("\\{name}[b]{{a}}");
            let (_, paths, _) = drawn(&src, false);
            assert!(!paths.is_empty() && paths.len() <= 2, "{src}: {paths:?}");
        }
    }

    #[test]
    fn a_huge_label_gives_the_same_number_of_paths() {
        let body = "x".repeat(2000);
        let (_, paths, _) = drawn(&format!("\\xrightarrow{{{body}}}"), false);
        assert_eq!(paths.len(), 1);
        let (_, paths, _) = drawn(&format!("\\xleftrightarrow{{{body}}}"), false);
        assert_eq!(paths.len(), 2);
    }
}
