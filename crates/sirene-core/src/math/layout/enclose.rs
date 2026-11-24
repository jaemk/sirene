//! Enclosures: `\fbox`, `\colorbox`, `\fcolorbox`, `\boxed`, `\cancel`, `\bcancel`,
//! `\xcancel`, `\sout`, `\angl`, and `\phase`.
//!
//! Adapted from KaTeX's `src/functions/enclose.ts` (MIT, Copyright (c) 2013-2020 Khan Academy
//! and other contributors), with `stretchy::enclose_span` (KaTeX's `stretchyEnclose`) for the
//! lines and backgrounds and `svg_paths::phase_path` for the phase angle.
//!
//! KaTeX pads the body with CSS (`.boxpad`, `.cancel-pad`, `.anglpad`) and stacks the image of
//! the enclosure with the body in a vlist; here the padding is kerns around the body and the
//! stack is `Bx::vlist_shifted`, with the same shifts. The vlist of a cancel keeps the body's
//! height and depth, and a cancel of more than one character takes back its padding as negative
//! margins (`.cancel-lap`).

use std::ops::Range;

use super::{Atom, Built, Class, build, stretchy};
use crate::math::{
    MathError,
    ast::{Measurement, Node},
    boxes::{Align, Bx, Fit},
    options::{Em, Options},
    svg_paths::phase_path,
    symbols::Mode,
};

/// The padding at each side of a box (`.boxpad`), in em.
const BOX_PAD: f64 = 0.3;
/// The padding at each side of a cancel of more than one character (`.cancel-pad`), in em.
const CANCEL_PAD: f64 = 0.2;
/// The padding at each side of an actuarial angle (`.anglpad`, 1mu), in em.
const ANGL_PAD: f64 = 0.03889;
/// The thickness of the line of `\sout` (`.katex-sout`), in em.
const SOUT_THICKNESS: f64 = 0.08;
/// The width of the view box of the phase angle's path.
const PHASE_VIEW_WIDTH: f64 = 400_000.0;

/// An enclosure: what `NodeKind::Enclose` holds, and where it is.
pub(super) struct Enclosure<'t, 'n> {
    pub label: &'t str,
    pub background_color: Option<&'t str>,
    pub border_color: Option<&'t str>,
    pub body: &'t Node<'n>,
    pub mode: Mode,
    pub span: Range<usize>,
}

/// A box with kerns of `left` and `right` around it.
fn padded<'t>(bx: Bx<'t>, left: f64, right: f64) -> Bx<'t> {
    if left == 0.0 && right == 0.0 {
        return bx;
    }
    Bx::hlist(vec![Bx::kern(Em(left)), bx, Bx::kern(Em(right))])
}

/// The box a label draws, how far it hangs below the baseline, and the body it encloses (with
/// its padding).
type Drawn<'t> = (Bx<'t>, Em, Bx<'t>);

/// The line of `\sout`, through the middle of the x-height.
fn sout<'t>(inner: Bx<'t>, options: &Options<'t>) -> Drawn<'t> {
    let metrics = options.metrics();
    let scale = options.size_multiplier();
    // KaTeX sizes the line `defaultRuleThickness / scale` high but draws a border of 0.08em;
    // the line here is drawn with that thickness and sized as KaTeX sizes it, from the top down.
    let layout = Em(metrics.default_rule_thickness / scale);
    let below = Em(SOUT_THICKNESS).max(layout) - layout;
    let line = Bx::rule(inner.width, layout, below, options);
    (line, Em(-0.5 * metrics.x_height), inner)
}

/// The angle of `\phase`, which takes a left pad for the angle.
fn phase<'t>(
    inner: Bx<'t>,
    options: &Options<'t>,
    span: &Range<usize>,
) -> Result<Drawn<'t>, MathError> {
    // Two dimensions from the steinmetz package.
    let line_weight = options.length(
        &Measurement {
            number: 0.6,
            unit: "pt".to_owned(),
        },
        span.clone(),
    )?;
    let clearance = options.length(
        &Measurement {
            number: 0.35,
            unit: "ex".to_owned(),
        },
        span.clone(),
    )?;
    // Size changes like \Huge do not change the line's thickness.
    let scale = options.size_multiplier() / options.having_base_sizing().size_multiplier();
    let angle_height = inner.height + inner.depth + line_weight + clearance;
    // A negative pad is invalid CSS, which the browser drops.
    let left = (angle_height / 2.0 + line_weight).0.max(0.0);
    let shift = inner.depth + line_weight + clearance;
    let body = padded(inner, left, 0.0);

    let view_height = (1000.0 * angle_height.0 * scale).floor().max(1.0);
    let img = Bx::path(
        phase_path(view_height),
        [0.0, 0.0, PHASE_VIEW_WIDTH, view_height],
        Fit::Crop(Align::Min),
        (body.width, angle_height.max(Em::ZERO), Em::ZERO),
        options,
    );
    Ok((img, shift, body))
}

/// The cancels, boxes, and the angle: the image from `stretchy::enclose_span` over the body.
fn boxed<'t>(
    name: &str,
    enclosure: &Enclosure<'t, '_>,
    inner: Bx<'t>,
    single_char: bool,
    options: &Options<'t>,
) -> Drawn<'t> {
    let metrics = options.metrics();
    let is_cancel = matches!(name, "cancel" | "bcancel" | "xcancel");
    // Horizontal padding, then vertical padding and the border's thickness.
    let (pad, top_pad, bottom_pad, rule) = if is_cancel {
        let pad = if single_char { 0.0 } else { CANCEL_PAD };
        let vertical = if single_char { 0.2 } else { 0.0 };
        (pad, Em(vertical), Em(vertical), Em::ZERO)
    } else if name == "angl" {
        let rule = Em(metrics.default_rule_thickness).max(options.min_rule_thickness);
        // A gap of three lines, plus the line itself.
        let top = rule * 4.0;
        let bottom = (Em(0.25) - inner.depth).max(Em::ZERO);
        (ANGL_PAD, top, bottom, rule)
    } else {
        let rule = Em(metrics.fboxrule).max(options.min_rule_thickness);
        let top = Em(metrics.fboxsep) + if name == "colorbox" { Em::ZERO } else { rule };
        (BOX_PAD, top, top, rule)
    };
    let content = inner.height + inner.depth;
    let drop = inner.depth + bottom_pad;
    let body = padded(inner, pad, pad);
    let spec = stretchy::Enclosure {
        width: body.width,
        content,
        top_pad,
        bottom_pad,
        rule: if name == "colorbox" { Em::ZERO } else { rule },
        background: enclosure.background_color,
        border: enclosure.border_color,
    };
    (stretchy::enclose_span(name, &spec, options), drop, body)
}

pub(super) fn enclose<'t>(
    enclosure: Enclosure<'t, '_>,
    options: &Options<'t>,
) -> Result<Built<'t>, MathError> {
    let name = enclosure
        .label
        .strip_prefix('\\')
        .unwrap_or(enclosure.label);
    let _ = enclosure.mode;
    let inner = build(enclosure.body, options)?.into_box();
    let (height, depth) = (inner.height, inner.depth);
    // A group of one character gets the "tall" treatment of the cancel package.
    let single_char = enclosure.body.is_character_box();
    let is_cancel = matches!(name, "cancel" | "bcancel" | "xcancel");

    let (img, drop, body) = match name {
        "sout" => sout(inner, options),
        "phase" => phase(inner, options, &enclosure.span)?,
        "cancel" | "bcancel" | "xcancel" | "fbox" | "colorbox" | "fcolorbox" | "angl" => {
            boxed(name, &enclosure, inner, single_char, options)
        }
        _ => {
            return Err(MathError::new(
                format!("Unknown enclosure \"{name}\""),
                enclosure.span,
            ));
        }
    };

    // A background goes behind the body, a line over it.
    let children = if enclosure.background_color.is_some() {
        vec![(img, drop, Em::ZERO), (body, Em::ZERO, Em::ZERO)]
    } else {
        vec![(body, Em::ZERO, Em::ZERO), (img, drop, Em::ZERO)]
    };
    let mut stack = Bx::vlist_shifted(children);
    if is_cancel {
        // The lines do not add their height to the expression.
        stack.height = height;
        stack.depth = depth;
        if !single_char {
            // Nor does the padding add space.
            stack = Bx::hlist(vec![
                Bx::kern(Em(-CANCEL_PAD)),
                stack,
                Bx::kern(Em(-CANCEL_PAD)),
            ]);
        }
    }
    Ok(Built::One(Atom::new(stack, Some(Class::Ord), options)))
}

#[cfg(test)]
mod tests {
    use super::super::layout;
    use crate::math::{
        boxes::{Align, Fit, Item},
        parser::{Settings, parse_with},
        svg_paths::phase_path,
    };

    /// A formula's width, height, and depth, and its items.
    fn drawn(src: &str) -> ((f64, f64, f64), Vec<Item<'static>>) {
        let nodes = parse_with(
            src,
            Settings {
                display_mode: false,
            },
        )
        .unwrap();
        // The box borrows the nodes, which end here: keep only owned data.
        let bx = layout(&nodes, false).unwrap();
        let items = bx
            .items()
            .into_iter()
            .map(|item| match item {
                Item::Glyph {
                    x,
                    y,
                    scale,
                    glyph,
                    mirrored,
                } => Item::Glyph {
                    x,
                    y,
                    scale,
                    glyph: crate::math::boxes::Glyph {
                        text: glyph.text.into_owned().into(),
                        face: glyph.face,
                        italic: glyph.italic,
                        skew: glyph.skew,
                        color: None,
                    },
                    mirrored,
                },
                Item::Rule {
                    x,
                    y,
                    width,
                    height,
                    ..
                } => Item::Rule {
                    x,
                    y,
                    width,
                    height,
                    color: None,
                },
                Item::Path {
                    x,
                    y,
                    width,
                    height,
                    path,
                    mirrored,
                } => Item::Path {
                    x,
                    y,
                    width,
                    height,
                    path: crate::math::boxes::Path {
                        data: path.data,
                        view_box: path.view_box,
                        fit: path.fit,
                        color: None,
                    },
                    mirrored,
                },
            })
            .collect();
        ((bx.width.0, bx.height.0, bx.depth.0), items)
    }

    fn size(src: &str) -> (f64, f64, f64) {
        drawn(src).0
    }

    /// The colors of the paths and rules of a formula, in drawing order.
    fn colors(src: &str) -> Vec<Option<String>> {
        let nodes = parse_with(
            src,
            Settings {
                display_mode: false,
            },
        )
        .unwrap();
        let bx = layout(&nodes, false).unwrap();
        bx.items()
            .into_iter()
            .filter_map(|item| match item {
                Item::Path { path, .. } => Some(path.color.map(str::to_owned)),
                Item::Rule { color, .. } => Some(color.map(str::to_owned)),
                Item::Glyph { .. } => None,
            })
            .collect()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn paths(items: &[Item<'_>]) -> Vec<([f64; 4], [f64; 4], Fit, String)> {
        items
            .iter()
            .filter_map(|item| match item {
                Item::Path {
                    x,
                    y,
                    width,
                    height,
                    path,
                    ..
                } => Some((
                    [*x, *y, *width, *height],
                    path.view_box,
                    path.fit,
                    path.data.to_string(),
                )),
                _ => None,
            })
            .collect()
    }

    // Heights and depths are KaTeX's, from the oracle fixture
    // `tests/fixtures/math/katex-oracle.json`, except where noted as worked from its formulas.

    const X_HEIGHT: f64 = 0.43056;

    #[test]
    fn an_fbox_pads_and_borders_its_body() {
        // `\boxed{x}`, whose oracle height and depth these are: 0.3 + 0.04 around the body.
        let (w, h, d) = size("\\fbox{x}");
        // The body is text.
        let (xw, ..) = size("\\text{x}");
        assert!(close(h, X_HEIGHT + 0.34) && close(d, 0.34), "{h} {d}");
        assert!(close(w, xw + 0.6), "{w}");
        let (_, items) = drawn("\\fbox{x}");
        let ring = paths(&items);
        let [(rect, view_box, fit, data)] = ring.as_slice() else {
            panic!("one ring: {ring:?}");
        };
        // The ring spans the padded body, from 0.34 below the baseline to 0.34 above the body.
        assert!(close(rect[0], 0.0) && close(rect[2], xw + 0.6));
        assert!(close(rect[1], -(X_HEIGHT + 0.34)) && close(rect[3], X_HEIGHT + 0.68));
        assert_eq!(*fit, Fit::Stretch);
        assert!(close(view_box[3], X_HEIGHT + 0.68));
        assert_eq!(data.matches('M').count(), 2, "{data}");
        // The glyph is inside the padding.
        let glyph_x = items.iter().find_map(|item| match item {
            Item::Glyph { x, .. } => Some(*x),
            _ => None,
        });
        assert!(close(glyph_x.unwrap(), 0.3));
    }

    #[test]
    fn a_boxed_formula_matches_the_oracle() {
        // `\boxed{x^2}` is `\fbox{$\displaystyle x^2$}`.
        let (_, h, d) = size("\\boxed{x^2}");
        assert!(close(h, 1.2041079999999997) && close(d, 0.34), "{h} {d}");
        let (_, h, d) = size("\\boxed{x}_2");
        assert!(
            close(h, 0.7705600000000001) && close(d, 0.38969999999999994),
            "{h} {d}"
        );
    }

    #[test]
    fn a_colorbox_has_a_fill_behind_the_body() {
        // Oracle: `\colorbox{red} x`.
        let (w, h, d) = size("\\colorbox{red}{x}");
        let (xw, ..) = size("\\text{x}");
        assert!(close(h, 0.7305599999999999) && close(d, 0.3), "{h} {d}");
        assert!(close(w, xw + 0.6), "{w}");
        let (_, items) = drawn("\\colorbox{red}{x}");
        // One fill, no border, and the fill comes before the glyph.
        assert_eq!(paths(&items).len(), 1);
        assert!(matches!(items.first(), Some(Item::Path { .. })));
        assert_eq!(colors("\\colorbox{red}{x}"), [Some("red".to_owned())]);
    }

    #[test]
    fn an_fcolorbox_has_a_fill_and_a_border_in_their_colors() {
        // Oracle: `\fcolorbox{blue}{yellow} x`.
        let (_, h, d) = size("\\fcolorbox{red}{blue}{x}");
        assert!(close(h, 0.77056) && close(d, 0.34), "{h} {d}");
        // The first color is the border, the second the fill.
        assert_eq!(
            colors("\\fcolorbox{red}{blue}{x}"),
            [Some("blue".to_owned()), Some("red".to_owned())]
        );
        // An `\fbox` border takes the color around it.
        assert_eq!(colors("\\color{red}\\fbox{x}"), [Some("red".to_owned())]);
    }

    #[test]
    fn a_phase_of_a_negative_body_has_no_negative_pad() {
        // KaTeX's negative `padding-left` is invalid CSS, which the browser drops.
        let (w, ..) = size("\\phase{\\rule{0em}{-1em}}");
        assert!(w >= 0.0, "{w}");
    }

    #[test]
    fn a_cancel_of_one_character_is_padded_vertically() {
        // Oracle: `\cancel{x}`: the lines add no height. The image is 0.2 above and below.
        let (w, h, d) = size("\\cancel{x}");
        let (xw, ..) = size("x");
        assert!(close(h, X_HEIGHT) && close(d, 0.0), "{h} {d}");
        assert!(close(w, xw), "{w}");
        let (_, items) = drawn("\\cancel{x}");
        let lines = paths(&items);
        let [(rect, ..)] = lines.as_slice() else {
            panic!("one path: {lines:?}");
        };
        assert!(close(rect[0], 0.0) && close(rect[2], xw));
        assert!(close(rect[1], -(X_HEIGHT + 0.2)) && close(rect[3], X_HEIGHT + 0.4));
    }

    #[test]
    fn a_cancel_of_several_characters_is_padded_sideways_without_taking_room() {
        // Worked from KaTeX: height and depth of the body `xy`, width unchanged, and the image
        // 0.2 wider each side with no vertical padding.
        let (w, h, d) = size("\\cancel{xy}");
        let (bw, bh, bd) = size("xy");
        assert!(close(h, bh) && close(d, bd), "{h} {d}");
        assert!(close(w, bw), "{w}");
        let (_, items) = drawn("\\cancel{xy}");
        let lines = paths(&items);
        let [(rect, ..)] = lines.as_slice() else {
            panic!("one path: {lines:?}");
        };
        assert!(close(rect[0], -0.2) && close(rect[2], bw + 0.4), "{rect:?}");
        assert!(close(rect[1], -bh) && close(rect[3], bh + bd), "{rect:?}");
        // The glyphs are where the body is.
        let first_glyph = items.iter().find_map(|item| match item {
            Item::Glyph { x, .. } => Some(*x),
            _ => None,
        });
        assert!(close(first_glyph.unwrap(), 0.0));
    }

    #[test]
    fn an_xcancel_draws_both_lines() {
        // Oracle: the height of `\text{\sout{abc}}` is that of `abc`; `ab` is 0.69444 high.
        let (_, h, d) = size("\\xcancel{ab}");
        assert!(close(h, 0.69444) && close(d, 0.0), "{h} {d}");
        let (_, items) = drawn("\\xcancel{ab}");
        let lines = paths(&items);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].3.matches('z').count(), 2, "{}", lines[0].3);
        let (_, items) = drawn("\\bcancel{ab}");
        assert_eq!(paths(&items)[0].3.matches('z').count(), 1);
    }

    #[test]
    fn a_sout_is_a_line_through_the_middle() {
        let (w, h, d) = size("\\sout{ab}");
        let (bw, ..) = size("ab");
        assert!(close(h, 0.69444) && close(d, 0.0), "{h} {d}");
        assert!(close(w, bw), "{w}");
        let (_, items) = drawn("\\sout{ab}");
        let line = items
            .iter()
            .find_map(|item| match item {
                Item::Rule {
                    x,
                    y,
                    width,
                    height,
                    ..
                } => Some([*x, *y, *width, *height]),
                _ => None,
            })
            .unwrap();
        // 0.08 thick, half an x-height above the baseline (the x-height is 0.431).
        assert!(close(line[0], 0.0) && close(line[2], bw));
        assert!(close(line[3], 0.08), "{line:?}");
        assert!(close(line[1], -(0.5 * 0.431 + 0.04)), "{line:?}");
        // A line with nothing under it is as high as the line.
        let (_, h, _) = size("\\sout{.}");
        assert!(close(h, 0.5 * 0.431 + 0.04), "{h}");
    }

    #[test]
    fn an_angl_has_a_top_and_right_border_and_a_gap_below() {
        // Worked from KaTeX: four rule thicknesses (0.04) above `n`, and 0.25 below it.
        let (w, h, d) = size("\\angl{n}");
        let (nw, nh, _) = size("\\text{n}");
        assert!(close(h, nh + 0.16) && close(d, 0.25), "{h} {d}");
        assert!(close(w, nw + 2.0 * 0.03889), "{w}");
        let (_, items) = drawn("\\angl{n}");
        let image = paths(&items);
        let [(rect, _, _, data)] = image.as_slice() else {
            panic!("one path: {image:?}");
        };
        assert!(close(rect[0], 0.0) && close(rect[2], nw + 2.0 * 0.03889));
        assert!(close(rect[1], -(nh + 0.16)) && close(rect[3], nh + 0.16 + 0.25));
        assert!(data.starts_with("M0 0H"), "{data}");
        // The oracle's `a_{\angl{n}}` depth (0.39796) is the angle's depth in script style
        // plus the subscript's drop.
        let (_, _, d) = size("a_{\\angl{n}}");
        assert!(close(d, 0.39796), "{d}");
    }

    #[test]
    fn a_phase_angle_is_an_svg_left_of_the_body() {
        // Worked from KaTeX: 0.6pt line, 0.35ex clearance (the x-height is 0.431em).
        let line = 0.06;
        let clearance = 0.35 * 0.431;
        let (w, h, d) = size("\\phase{x}");
        let (xw, ..) = size("x");
        let angle = X_HEIGHT + line + clearance;
        assert!(close(h, X_HEIGHT), "{h}");
        assert!(close(d, line + clearance), "{d}");
        assert!(close(w, xw + angle / 2.0 + line), "{w}");
        let (_, items) = drawn("\\phase{x}");
        let image = paths(&items);
        let [(rect, view_box, fit, data)] = image.as_slice() else {
            panic!("one path: {image:?}");
        };
        assert!(close(rect[0], 0.0) && close(rect[2], xw + angle / 2.0 + line));
        assert!(close(rect[1], -X_HEIGHT) && close(rect[3], angle));
        let view_height = (1000.0 * angle).floor();
        assert_eq!(*view_box, [0.0, 0.0, 400_000.0, view_height]);
        assert_eq!(*fit, Fit::Crop(Align::Min));
        assert_eq!(*data, phase_path(view_height));
        // The body is right of the angle.
        let glyph_x = items.iter().find_map(|item| match item {
            Item::Glyph { x, .. } => Some(*x),
            _ => None,
        });
        assert!(close(glyph_x.unwrap(), angle / 2.0 + line));
    }

    #[test]
    fn matches_the_oracle_on_more_formulas() {
        let (_, h, d) = size("\\phase{-78.2^\\circ}");
        assert!(close(h, 0.674115) && close(d, 0.29418), "{h} {d}");
        let (_, h, d) = size("\\fcolorbox{blue}{yellow}{a b}");
        assert!(close(h, 1.03444) && close(d, 0.34), "{h} {d}");
        let (_, h, d) = size("\\cancel{x}_2");
        assert!(close(h, 0.43056) && close(d, 0.15), "{h} {d}");
        let (_, h, d) = size("\\sout{x}_2^2");
        assert!(
            close(h, 0.8141079999999999) && close(d, 0.24810799999999997),
            "{h} {d}"
        );
    }

    #[test]
    fn the_class_is_ordinary() {
        // `a \cancel{b}`: no spaces around an ordinary atom, as with `ab`.
        let (w, ..) = size("a\\cancel{b}");
        let (aw, ..) = size("ab");
        assert!(close(w, aw), "{w} {aw}");
    }

    #[test]
    fn a_wide_or_deep_body_makes_a_fixed_number_of_items() {
        let body = "x".repeat(2000);
        for label in ["\\fbox", "\\cancel", "\\xcancel", "\\sout", "\\phase"] {
            let (_, items) = drawn(&format!("{label}{{{body}}}"));
            let extra = items
                .iter()
                .filter(|item| !matches!(item, Item::Glyph { .. }))
                .count();
            assert!(extra <= 3, "{label}: {extra}");
        }
    }
}
