//! Roots: `\sqrt{x}` and `\sqrt[n]{x}`.
//!
//! Adapted from KaTeX's `src/functions/sqrt.ts` and `makeSqrtImage` in `src/delimiter.ts` (MIT,
//! Copyright (c) 2013-2020 Khan Academy and other contributors), with the radical paths of
//! `math::svg_paths`. TeX's rule 11 places the body under the vinculum; the radical sign is the
//! smallest of the font's signs that is tall enough, or one path drawn to any height.

use std::ops::Range;

use super::{
    Atom, Built, Class, build, build_in,
    delimiter::{Surd, surd_size},
};
use crate::math::{
    MathError,
    ast::Node,
    boxes::{Align, Bx, Fit, Part, Position},
    options::{Em, Options},
    style::Style,
    svg_paths::{SqrtSize, sqrt_path},
};

/// Every radical's view box has this much room above its vinculum, in view box units, which KaTeX
/// keeps so a browser's rounding of the height cannot pinch the line.
const VIEW_BOX_PAD: f64 = 80.0;

/// The same room in em, which the box height leaves out.
const EM_PAD: f64 = 0.08;

/// The height plus depth of each size font's radical sign.
const SIZE_TO_MAX_HEIGHT: [f64; 4] = [1.2, 1.8, 2.4, 3.0];

/// A radical sign as a box, and the sizes the body is placed by.
struct SqrtImage<'t> {
    /// The sign: as high as TeX's box for it, and as wide as its least width.
    sign: Bx<'t>,
    /// The thickness of the vinculum.
    rule_width: f64,
    /// How far right the body starts.
    advance_width: f64,
}

/// A radical sign for a body `height` em tall (KaTeX's `makeSqrtImage`).
fn sqrt_image<'t>(
    height: f64,
    options: &Options<'t>,
    span: &Range<usize>,
) -> Result<SqrtImage<'t>, MathError> {
    // A size change such as `\Huge` does not pick a different sign: it scales the one picked.
    let base = options.having_base_sizing();
    let surd = surd_size(height * base.size_multiplier(), &base, span)?;
    let mut size_multiplier = base.size_multiplier();

    // The standard signs have a vinculum 0.04em thick; a thicker minimum rule adds to it.
    let metrics = options.metrics();
    let extra = (options.min_rule_thickness.0 - metrics.sqrt_rule_thickness).max(0.0);

    // The height the sign has for placement (`tex`), and the height of its drawing, which has
    // `EM_PAD` more above the vinculum.
    let (size, view_height, tex_height, span_height, min_width, advance_width) = match surd {
        Surd::Small => {
            if height < 1.0 {
                size_multiplier = 1.0;
            } else if height < 1.4 {
                size_multiplier = 0.7;
            }
            (
                SqrtSize::Main,
                1000.0 + 1000.0 * extra + VIEW_BOX_PAD,
                (1.0 + extra) / size_multiplier,
                (1.0 + extra + EM_PAD) / size_multiplier,
                0.853,
                0.833 / size_multiplier,
            )
        }
        Surd::Large(n) => {
            let Some(&max_height) = usize::from(n)
                .checked_sub(1)
                .and_then(|at| SIZE_TO_MAX_HEIGHT.get(at))
            else {
                return Err(MathError::new(
                    format!("Unknown radical size {n}"),
                    span.clone(),
                ));
            };
            let size = match n {
                1 => SqrtSize::Size1,
                2 => SqrtSize::Size2,
                3 => SqrtSize::Size3,
                _ => SqrtSize::Size4,
            };
            (
                size,
                (1000.0 + VIEW_BOX_PAD) * max_height,
                (max_height + extra) / size_multiplier,
                (max_height + extra + EM_PAD) / size_multiplier,
                1.02,
                1.0 / size_multiplier,
            )
        }
        // Tall: where TeX stacks glyphs, one path stretches to the height.
        Surd::Tall => (
            SqrtSize::Tall,
            (1000.0 * height + extra).floor() + VIEW_BOX_PAD,
            height + extra,
            height + extra + EM_PAD,
            0.742,
            1.056,
        ),
    };

    // The drawing is `span_height` tall and its bottom is the box's. The box leaves out the room
    // at the top, so the view box does too, at the drawing's own scale.
    let pad = view_height * (span_height - tex_height) / span_height;
    let sign = Bx::path(
        sqrt_path(size, extra, view_height),
        [0.0, pad, 400_000.0, view_height - pad],
        Fit::Crop(Align::Min),
        (Em(min_width), Em(tex_height), Em::ZERO),
        options,
    );
    Ok(SqrtImage {
        sign,
        // The width of the line depends on the font chosen, and `\boldmath` should use the
        // thicker signs of the bold font; KaTeX does not either.
        rule_width: (metrics.sqrt_rule_thickness + extra) * size_multiplier,
        advance_width,
    })
}

/// A root, by TeX's rule 11. A root index is set in scriptscript style and raised by 60% of the
/// root's height less its depth, as TeX's `\r@@t` does.
pub(super) fn sqrt<'t>(
    body: &'t Node<'_>,
    index: Option<&'t Node<'_>>,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    // The body in a cramped style, and a body of no height as high as an x.
    let mut inner = build(body, &options.having_cramped_style())?.into_box();
    if inner.height.0 == 0.0 {
        inner.height = Em(options.metrics().x_height);
    }

    // The least height the sign must have.
    let metrics = options.metrics();
    let theta = metrics.default_rule_thickness;
    let phi = if options.style.size() == Style::Display.size() {
        metrics.x_height
    } else {
        theta
    };
    // The clearance between the body and the line.
    let mut line_clearance = theta + phi / 4.0;
    let body_size = inner.height.0 + inner.depth.0;
    let min_delimiter_height = body_size + line_clearance + theta;

    let SqrtImage {
        mut sign,
        rule_width,
        advance_width,
    } = sqrt_image(min_delimiter_height, options, &span)?;

    // A sign taller than needed shares the extra between above and below the body.
    let delim_depth = sign.height.0 - rule_width;
    if delim_depth > body_size + line_clearance {
        line_clearance = (line_clearance + delim_depth - body_size) / 2.0;
    }
    let sign_shift = sign.height.0 - inner.height.0 - line_clearance - rule_width;

    // The sign is as wide as the body with the room it leaves for the sign's hook.
    sign.width = sign.width.max(inner.width + Em(advance_width));
    let kern = Em(-(inner.height.0 + sign_shift));
    let body_box = Bx::vlist(
        Position::FirstBaseline,
        vec![
            Part::Box {
                child: inner,
                dx: Em(advance_width),
            },
            Part::Kern(kern),
            Part::of(sign),
            Part::Kern(Em(rule_width)),
        ],
    );

    let Some(index) = index else {
        return Ok(Built::One(Atom::new(body_box, Some(Class::Ord), options)));
    };
    // The index is always in scriptscript style.
    let index_options = options.having_style(Style::ScriptScript);
    let root = build_in(index, &index_options, options)?.into_box();
    let to_shift = 0.6 * (body_box.height.0 - body_box.depth.0);
    let raised = Bx::vlist(Position::Shift(Em(-to_shift)), vec![Part::of(root)]);
    // `\mkern 5mu` before the index and `\mkern -10mu` after it, from TeX's `\r@@t`.
    let bx = Bx::hlist(vec![
        Bx::kern(Em(5.0 / 18.0)),
        raised,
        Bx::kern(Em(-10.0 / 18.0)),
        body_box,
    ]);
    Ok(Built::One(Atom::new(bx, Some(Class::Ord), options)))
}

#[cfg(test)]
mod tests {
    use crate::math::{
        boxes::{Bx, Item},
        layout::layout,
        parser::{Settings, parse_with},
    };

    fn lay<R>(src: &str, display: bool, check: impl FnOnce(&Bx<'_>) -> R) -> R {
        let nodes = parse_with(
            src,
            Settings {
                display_mode: display,
            },
        )
        .unwrap();
        let bx = layout(&nodes, display).unwrap();
        check(&bx)
    }

    fn size(src: &str, display: bool) -> (f64, f64, f64) {
        lay(src, display, |bx| (bx.width.0, bx.height.0, bx.depth.0))
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    /// The radical sign of a layout as (x, y, width, height, view box), y from the top.
    fn signs(src: &str, display: bool) -> Vec<(f64, f64, f64, f64, [f64; 4])> {
        lay(src, display, |bx| {
            bx.items()
                .into_iter()
                .filter_map(|item| match item {
                    Item::Path {
                        x,
                        y,
                        width,
                        height,
                        path,
                        ..
                    } => Some((x, y, width, height, path.view_box)),
                    _ => None,
                })
                .collect()
        })
    }

    /// The glyphs of a layout as (text, x, y, scale).
    fn glyphs(src: &str, display: bool) -> Vec<(String, f64, f64, f64)> {
        lay(src, display, |bx| {
            bx.items()
                .into_iter()
                .filter_map(|item| match item {
                    Item::Glyph {
                        x, y, scale, glyph, ..
                    } => Some((glyph.text.to_string(), x, y, scale)),
                    _ => None,
                })
                .collect()
        })
    }

    #[test]
    fn a_root_of_one_letter() {
        // Worked from TeX's rule 11 in text style: x is 0.43056 high; theta is 0.04 and the
        // clearance 0.04 + 0.04 / 4 = 0.05; the main font's radical (1em, 0.96 of it below the
        // line) holds the 0.52056 needed, so the clearance grows to (0.05 + 0.96 - 0.43056) / 2
        // = 0.28972 and the sign hangs 1 - 0.43056 - 0.28972 - 0.04 = 0.23972 below the
        // baseline. The line is 0.04 thick, and KaTeX adds that to the height.
        let (w, h, d) = size("\\sqrt x", false);
        assert!(close(d, 0.23972), "{d}");
        assert!(close(h, 0.43056 + 0.28972 + 0.04 + 0.04), "{h}");
        // The body starts after the sign's 0.833em, which is wider than the body is.
        assert!(close(w, 0.833 + 0.57153), "{w}");
        let found = glyphs("\\sqrt x", false);
        assert_eq!(found.len(), 1);
        assert!(
            close(found[0].1, 0.833) && close(found[0].2, 0.0),
            "{found:?}"
        );
        // The sign is one path 1em high, from the baseline's 0.23972 below up to 0.76028 above,
        // as wide as the root.
        let sign = signs("\\sqrt x", false);
        assert_eq!(sign.len(), 1);
        let (x, y, width, height, view_box) = sign[0];
        assert!(
            close(x, 0.0) && close(y, -0.76028) && close(height, 1.0),
            "{sign:?}"
        );
        assert!(close(width, w), "{sign:?}");
        // The view box leaves out the 0.08em of room KaTeX draws above the line.
        let want = [0.0, 80.0, 400_000.0, 1000.0];
        assert!(view_box.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-6));
    }

    #[test]
    fn a_root_in_display_style_clears_by_an_x_height() {
        // Display style: phi is the x height (0.43097 here is 0.431), the clearance 0.04 +
        // 0.431 / 4. The sign's extra room is shared, as above.
        let phi = 0.431;
        let clearance = 0.04 + phi / 4.0;
        let split = (clearance + 0.96 - 0.43056) / 2.0;
        let shift = 1.0 - 0.43056 - split - 0.04;
        let (_, h, d) = size("\\sqrt x", true);
        assert!((d - shift).abs() < 1e-3, "{d} {shift}");
        assert!((h - (1.0 - shift + 0.04)).abs() < 1e-3, "{h}");
    }

    #[test]
    fn a_root_with_an_index() {
        // The index is in scriptscript style (half size here), raised by 0.6 of the root's height
        // less its depth: 0.6 * (0.80028 - 0.23972). TeX's `\r@@t` puts 5mu before it and -10mu
        // after, so the sign starts 5/18 + 0.25 - 10/18 from the left.
        let plain = size("\\sqrt x", false);
        let (w, h, d) = size("\\sqrt[3]x", false);
        assert!(close(h, plain.1) && close(d, plain.2), "{h} {d}");
        let shift = 5.0 / 18.0 + 0.25 - 10.0 / 18.0;
        assert!(close(w, plain.0 + shift), "{w}");
        let raised = 0.6 * (0.80028 - 0.23972);
        let found = glyphs("\\sqrt[3]x", false);
        assert_eq!(found.len(), 2);
        let (text, x, y, scale) = found[0].clone();
        assert_eq!(text, "3");
        assert!(
            close(x, 5.0 / 18.0) && close(y, -raised) && close(scale, 0.5),
            "{found:?}"
        );
        // The body is after the sign, which has moved with it.
        assert!(close(found[1].1, shift + 0.833), "{found:?}");
        let sign = signs("\\sqrt[3]x", false);
        assert!(close(sign[0].0, shift), "{sign:?}");
        assert!(close(found[1].3, 1.0), "{found:?}");
    }

    #[test]
    fn a_tall_root_is_one_path() {
        // A 3em body needs a sign of 3.09em, more than the biggest radical font's 3em: the
        // sign is drawn as one path of that height, with its 1.056em hook before the body. The
        // sign has no room to spare, so it reaches the baseline and the root is 0.04 higher.
        let (w, h, d) = size("\\sqrt{\\rule{1em}{3em}}", false);
        assert!(close(h, 3.09 + 0.04) && close(d, 0.0), "{h} {d}");
        assert!(close(w, 1.056 + 1.0), "{w}");
        let sign = signs("\\sqrt{\\rule{1em}{3em}}", false);
        assert_eq!(sign.len(), 1);
        let (x, y, width, height, view_box) = sign[0];
        assert!(
            close(x, 0.0) && close(y, -3.09) && close(height, 3.09),
            "{sign:?}"
        );
        assert!(close(width, w), "{sign:?}");
        let want = [0.0, 80.0, 400_000.0, 3090.0];
        assert!(view_box.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-6));
        // Under 3em it is a glyph-sized sign: Size1's for a body that is a little too tall for
        // the main font's.
        let sign = signs("\\sqrt{\\rule{1em}{1em}}", false);
        assert!(close(sign[0].3, 1.2), "{sign:?}");
    }

    #[test]
    fn a_huge_body_is_still_one_path() {
        let sign = signs("\\sqrt{\\rule{1em}{900em}}", false);
        assert_eq!(sign.len(), 1);
        assert!(close(sign[0].3, 900.09), "{sign:?}");
    }

    #[test]
    fn a_body_without_height_is_as_high_as_an_x() {
        // `\sqrt{}` and a body of a kern have no height; KaTeX gives them the font's x height
        // (0.431), so the sign hangs 1 - 0.431 - (0.05 + 0.96 - 0.431) / 2 - 0.04 = 0.2395 below
        // the baseline, and the root is as wide as the sign's least width.
        let (w, h, d) = size("\\sqrt{}", false);
        assert!(close(d, 0.2395) && close(h, 1.0 - 0.2395 + 0.04), "{h} {d}");
        assert!(close(w, 0.853), "{w}");
    }

    #[test]
    fn heights_and_depths_agree_with_katex() {
        // KaTeX's answers (the baseline oracle's) for inputs with roots, which cover the main
        // font's sign, the size fonts', and the size changes.
        for (src, height, depth) in [
            ("\\sqrt{2}", 0.90722, 0.13278),
            ("\\sqrt[3]{2}", 0.90722, 0.13278),
            ("\\sqrt2^3", 1.111228, 0.13278),
            ("\\sqrt \\frac x y", 1.092142, 0.747858),
            ("\\sqrt \\text x", 0.8002800000000001, 0.23972),
            (
                "\\Large\\sqrt[3]{x}",
                1.1524032000000002,
                0.34519679999999997,
            ),
            (
                "{\\Huge x}{\\LARGE y}{\\normalsize z}{\\scriptsize w}\\sqrt[\\small 3]{x+1}\n",
                1.07123328,
                0.33599232,
            ),
            (
                "\\left( X^{\\smash 2} \\right) \\sqrt{\\smash[b]{y=}}",
                0.8002800000000001,
                0.25,
            ),
        ] {
            let (_, h, d) = size(src, false);
            assert!(
                (h - height).abs() < 1e-4 && (d - depth).abs() < 1e-4,
                "{src}: {h} {d}"
            );
        }
    }
}
