//! Accents over and under a base: `\hat{x}`, `\widehat{xy}`, `\vec`, text accents, and
//! `\underrightarrow` and its kind.
//!
//! Adapted from KaTeX's `src/functions/accent.ts` and `src/functions/accentunder.ts` (MIT,
//! Copyright (c) 2013-2020 Khan Academy and other contributors), with `stretchy` for the wide
//! forms. An accent sits `min(base height, x-height)` into its base, moved right by the base's
//! skew; a base of one character with scripts has the scripts laid on the base alone.

use std::borrow::Cow;
use std::ops::Range;

use super::{Atom, Built, Class, build, scripts, stretchy, symbol};
use crate::math::{
    MathError,
    ast::{Node, NodeKind},
    boxes::{BoxKind, Bx, Fit, Part, Position},
    options::{Em, Options},
    svg_data,
    symbols::{Mode, Name},
};

/// An accent: what `NodeKind::Accent` holds, and where it is.
pub(super) struct Accent<'t, 'n> {
    pub label: &'t str,
    pub stretchy: bool,
    pub shifty: bool,
    pub base: &'t Node<'n>,
    pub mode: Mode,
    pub span: Range<usize>,
}

/// The width and height of the `\vec` arrow, in em (KaTeX's `svgData.vec`).
const VEC_SIZE: (f64, f64) = (0.471, 0.714);

/// How far `\textcircled` is moved down, in em.
const TEXTCIRCLED_DROP: f64 = 0.2;

/// How many characters a wide accent covers: the length of a group, else one.
fn char_count(base: &Node<'_>) -> usize {
    match &base.kind {
        NodeKind::Group { body, .. } => body.len(),
        _ => 1,
    }
}

/// The skew of the symbol a built base is (KaTeX's `getBaseSymbol`): the glyph itself, or the
/// only child of a box.
fn base_skew(bx: &Bx<'_>) -> Em {
    match &bx.kind {
        BoxKind::Glyph(glyph) => glyph.skew,
        BoxKind::HList(children) => match children.as_slice() {
            [only] => base_skew(only),
            _ => Em::ZERO,
        },
        _ => Em::ZERO,
    }
}

/// The glyph of a non-stretchy accent, without its italic correction (which only serves to shift
/// the accent over).
fn accent_glyph<'t>(accent: &Accent<'t, '_>, options: &Options<'t>) -> Result<Bx<'t>, MathError> {
    if accent.label == "\\vec" {
        // KaTeX draws `\vec` as an SVG, not as the combining glyph.
        let data = svg_data::path("vec").ok_or_else(|| {
            MathError::new("No SVG data for \"vec\".".to_string(), accent.span.clone())
        })?;
        let (width, height) = VEC_SIZE;
        return Ok(Bx::path(
            data,
            [0.0, 0.0, width * 1000.0, height * 1000.0],
            Fit::Stretch,
            (Em(width), Em(height), Em::ZERO),
            options,
        ));
    }
    let name = accent.label.strip_prefix('\\').unwrap_or(accent.label);
    let text = Cow::Borrowed(accent.label);
    let mut bx = symbol::ord(
        text,
        Some(Name::Control(name)),
        false,
        accent.mode,
        options,
        accent.span.clone(),
    )?
    .into_box();
    if let BoxKind::Glyph(glyph) = &mut bx.kind {
        bx.width -= glyph.italic.max(Em::ZERO);
        glyph.italic = Em::ZERO;
    }
    Ok(bx)
}

/// A base with its accent over it, as KaTeX's `accent` builder gives it.
fn accent_box<'t>(accent: &Accent<'t, '_>, options: &Options<'t>) -> Result<Bx<'t>, MathError> {
    let body = build(accent.base, &options.having_cramped_style())?.into_box();

    // An accent over one character moves right by the character's skew.
    let skew = if accent.shifty && accent.base.is_character_box() {
        base_skew(&body)
    } else {
        Em::ZERO
    };

    if accent.stretchy {
        let (width, dx) = if skew > Em::ZERO {
            ((body.width - skew * 2.0).max(Em::ZERO), skew * 2.0)
        } else {
            (body.width, Em::ZERO)
        };
        let chars = Some(char_count(accent.base));
        let svg = stretchy::svg_span(accent.label, chars, width, options, accent.span.clone())?;
        // KaTeX centers the rows of an accent (`.katex-accent > .vlist-t`), so a base narrower
        // than the accent's least width is centered under it.
        let full = body.width.max(svg.width + dx);
        let body_dx = (full - body.width) / 2.0;
        return Ok(Bx::vlist(
            Position::FirstBaseline,
            vec![
                Part::Box {
                    child: body,
                    dx: body_dx,
                },
                Part::Box { child: svg, dx },
            ],
        ));
    }

    let below = accent.label == "\\c";
    let mut clearance = if below {
        body.height + body.depth
    } else {
        body.height.min(Em(options.metrics().x_height))
    };
    let glyph = accent_glyph(accent, options)?;
    if below {
        clearance += glyph.depth;
    }
    let (width, height, depth) = (glyph.width, glyph.height, glyph.depth);

    // KaTeX centers the rows of an accent (`.katex-accent > .vlist-t`). A "full" accent is as
    // wide as the accent and sits on the base; any other has no width, so it sits at the base's
    // middle and is shifted by half its own width.
    let full = accent.label == "\\textcircled";
    let mut left = skew;
    let mut drop = Em::ZERO;
    if full {
        clearance = body.height;
        drop = Em(TEXTCIRCLED_DROP);
    } else {
        left -= width / 2.0;
    }
    let mut placed = Bx::vlist(
        Position::Shift(drop),
        vec![Part::Box {
            child: glyph,
            dx: left,
        }],
    );
    // The drop moves the drawing, not the accent's box.
    placed.width = if full { width } else { Em::ZERO };
    placed.height = height;
    placed.depth = depth;

    let row = body.width.max(placed.width);
    let body_dx = (row - body.width) / 2.0;
    let placed_dx = (row - placed.width) / 2.0;
    Ok(Bx::vlist(
        Position::FirstBaseline,
        vec![
            Part::Box {
                child: body,
                dx: body_dx,
            },
            Part::Kern(-clearance),
            Part::Box {
                child: placed,
                dx: placed_dx,
            },
        ],
    ))
}

pub(super) fn accent<'t>(
    accent: Accent<'t, '_>,
    options: &Options<'t>,
) -> Result<Built<'t>, MathError> {
    let bx = accent_box(&accent, options)?;
    // Accents are ordinary, even over an operator.
    Ok(Built::One(Atom::new(bx, Some(Class::Ord), options)))
}

pub(super) fn accent_under<'t>(
    label: &'t str,
    base: &'t Node<'_>,
    _mode: Mode,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    // Under accents are set much like underlines.
    let inner = build(base, options)?.into_box();
    let svg = stretchy::svg_span(label, Some(char_count(base)), inner.width, options, span)?;
    let kern = if label == "\\utilde" { 0.12 } else { 0.0 };
    let top = inner.height;
    let bx = Bx::vlist(
        Position::Top(top),
        vec![Part::of(svg), Part::Kern(Em(kern)), Part::of(inner)],
    );
    Ok(Built::One(Atom::new(bx, Some(Class::Ord), options)))
}

/// Scripts on an accented base whose base is one character, which KaTeX's accent builder
/// places itself (`htmlBuilderDelegate`); `None` for any other base.
///
/// The scripts go on the character alone, so the accent's height does not move them; the
/// accent then replaces the character in the result.
pub(super) fn supsub<'t>(
    base: &'t Node<'_>,
    sup: Option<&'t Node<'_>>,
    sub: Option<&'t Node<'_>>,
    options: &Options<'t>,
) -> Result<Option<Built<'t>>, MathError> {
    let NodeKind::Accent {
        label,
        stretchy,
        shifty,
        base: inner,
    } = &base.kind
    else {
        return Ok(None);
    };
    if !inner.is_character_box() || (sup.is_none() && sub.is_none()) {
        return Ok(None);
    }
    let scripted = scripts::supsub(Some(inner), sup, sub, options)?;
    let accented = accent_box(
        &Accent {
            label,
            stretchy: *stretchy,
            shifty: *shifty,
            base: inner,
            mode: base.mode,
            span: base.span.clone(),
        },
        options,
    )?;
    let Built::One(Atom {
        bx:
            Bx {
                kind: BoxKind::HList(mut children),
                depth,
                ..
            },
        ..
    }) = scripted
    else {
        return Ok(None);
    };
    let Some(first) = children.first_mut() else {
        return Ok(None);
    };
    *first = accented;
    // KaTeX recomputes only the height after swapping the accent in (`accent.ts`), so the
    // depth stays the scripted character's.
    let mut bx = Bx::hlist(children);
    bx.depth = depth;
    Ok(Some(Built::One(Atom::new(bx, Some(Class::Ord), options))))
}

#[cfg(test)]
mod tests {
    use super::super::layout;
    use crate::math::{
        boxes::{Fit, Item},
        metrics,
        metrics_data::Face,
        parser::{Settings, parse_with},
        svg_data,
        symbols::Mode,
    };

    /// A formula's width, height, and depth, the paths (rectangle, fit, data), and the glyphs
    /// (text, x, y).
    type Drawn = (
        (f64, f64, f64),
        Vec<([f64; 4], Fit, String)>,
        Vec<(String, f64, f64)>,
    );

    fn drawn(src: &str) -> Drawn {
        let nodes = parse_with(
            src,
            Settings {
                display_mode: false,
            },
        )
        .unwrap();
        let bx = layout(&nodes, false).unwrap();
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
                } => paths.push(([x, y, width, height], path.fit, path.data.into_owned())),
                Item::Glyph { glyph, x, y, .. } => glyphs.push((glyph.text.into_owned(), x, y)),
                Item::Rule { .. } => {}
            }
        }
        ((bx.width.0, bx.height.0, bx.depth.0), paths, glyphs)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    fn glyph_at<'a>(glyphs: &'a [(String, f64, f64)], text: &str) -> &'a (String, f64, f64) {
        glyphs.iter().find(|(t, ..)| t == text).unwrap()
    }

    // Heights and depths are KaTeX's, from the oracle fixture
    // `tests/fixtures/math/katex-oracle.json`, where it has the input; otherwise they are
    // worked from KaTeX's formulas with the font metrics (said at each).

    #[test]
    fn a_hat_moves_right_by_the_skew_of_its_base() {
        let ((w, h, d), _, glyphs) = drawn("\\hat x");
        let x = metrics::glyph('x', Face::MathItalic, Mode::Math).unwrap();
        let hat = metrics::glyph('^', Face::MainRegular, Mode::Math).unwrap();
        // The accent is centered on the base, then moved by the skew; its italic is dropped
        // (TeX's rule 12, and KaTeX's centered rows with the accent's `left`).
        let (_, hx, hy) = glyph_at(&glyphs, "^");
        assert!(close(*hx, w / 2.0 + x.skew - hat.width / 2.0), "{hx}");
        // The hat sits min(height, x-height) = 0.43056 into the base, which is all of it: its
        // baseline is on the base's baseline less the glyph's depth.
        assert!(close(*hy, hat.depth), "{hy}");
        assert!(close(h, hat.height + hat.depth), "{h}");
        assert!(close(d, 0.0) && close(w, x.width), "{w} {d}");
        let (_, bx, _) = glyph_at(&glyphs, "x");
        assert!(close(*bx, 0.0));
    }

    #[test]
    fn a_hat_over_a_group_has_no_skew() {
        let ((w, h, _), _, glyphs) = drawn("\\hat{xy}");
        let hat = metrics::glyph('^', Face::MainRegular, Mode::Math).unwrap();
        let (_, hx, _) = glyph_at(&glyphs, "^");
        let x = metrics::glyph('x', Face::MathItalic, Mode::Math).unwrap();
        let y = metrics::glyph('y', Face::MathItalic, Mode::Math).unwrap();
        let base = x.width + x.italic.max(0.0) + y.width + y.italic.max(0.0);
        assert!(close(w, base), "{w}");
        assert!(close(*hx, (w - hat.width) / 2.0), "{hx}");
        assert!(close(h, hat.height + hat.depth), "{h}");
    }

    #[test]
    fn a_vec_is_an_svg_arrow() {
        // 0.714 is KaTeX's oracle height for \vec{x}.
        for src in ["\\vec x", "\\vec{x}", "\\vec v"] {
            let ((_, h, d), paths, _) = drawn(src);
            assert!(close(h, 0.714) && close(d, 0.0), "{src}: {h} {d}");
            let [(rect, fit, data)] = paths.as_slice() else {
                panic!("one path: {paths:?}");
            };
            assert_eq!(*fit, Fit::Stretch);
            assert_eq!(data.as_str(), svg_data::path("vec").unwrap());
            assert!(close(rect[2], 0.471) && close(rect[3], 0.714), "{rect:?}");
        }
        let ((w, ..), paths, _) = drawn("\\vec x");
        let skew = metrics::glyph('x', Face::MathItalic, Mode::Math)
            .unwrap()
            .skew;
        assert!(
            close(paths[0].0[0], w / 2.0 + skew - 0.471 / 2.0),
            "{paths:?}"
        );
    }

    #[test]
    fn a_wide_hat_stretches_over_its_base() {
        // \widehat{abc}: three characters take the 0.3em image, which stands on the base.
        let ((w, h, d), paths, glyphs) = drawn("\\widehat{abc}");
        let (_, _, by) = glyph_at(&glyphs, "b");
        assert!(close(*by, 0.0));
        assert!(close(h, 0.69444 + 0.3) && close(d, 0.0), "{h} {d}");
        let [(rect, fit, data)] = paths.as_slice() else {
            panic!("one path: {paths:?}");
        };
        assert_eq!(*fit, Fit::Stretch);
        assert_eq!(data.as_str(), svg_data::path("widehat2").unwrap());
        assert!(close(rect[0], 0.0) && close(rect[2], w) && close(rect[3], 0.3));
        assert!(close(rect[1], -(0.69444 + 0.3)), "{rect:?}");
    }

    #[test]
    fn a_wide_hat_over_one_character_is_shifted_by_its_skew() {
        // KaTeX: 0.67056 high for \widehat{x} (oracle): the 0.24em image on x's 0.43056.
        let ((w, h, _), paths, _) = drawn("\\widehat{x}");
        assert!(close(h, 0.67056), "{h}");
        let skew = metrics::glyph('x', Face::MathItalic, Mode::Math)
            .unwrap()
            .skew;
        let [(rect, _, data)] = paths.as_slice() else {
            panic!("one path: {paths:?}");
        };
        assert_eq!(data.as_str(), svg_data::path("widehat1").unwrap());
        assert!(
            close(rect[0], 2.0 * skew) && close(rect[2], w - 2.0 * skew),
            "{rect:?}"
        );
    }

    #[test]
    fn an_arrow_accent_is_a_stretchy_arrow_over_the_base() {
        // 1.20533 is KaTeX's oracle height for \overrightarrow{AB}.
        let ((w, h, d), paths, _) = drawn("\\overrightarrow{AB}");
        assert!(close(h, 1.20533) && close(d, 0.0), "{h} {d}");
        let [(rect, fit, _)] = paths.as_slice() else {
            panic!("one path: {paths:?}");
        };
        assert!(matches!(fit, Fit::Crop(_)));
        assert!(
            close(rect[0], 0.0) && close(rect[2], w.max(0.888)),
            "{rect:?}"
        );
        assert!(close(rect[3], 0.522));
        // Narrower than the arrow's least width: the arrow is wider than the base.
        let ((w, ..), paths, glyphs) = drawn("\\overrightarrow{x}");
        assert!(
            close(paths[0].0[2], 0.888) && close(w, 0.888),
            "{w} {paths:?}"
        );
        // KaTeX centers the rows of an accent, so the x is centered under the arrow.
        let x = metrics::glyph('x', Face::MathItalic, Mode::Math).unwrap();
        let (_, bx, _) = glyph_at(&glyphs, "x");
        assert!(close(*bx, (0.888 - x.width) / 2.0), "{bx}");
    }

    #[test]
    fn a_character_accent_with_scripts_scripts_the_character() {
        // KaTeX (oracle): \vec{x}^2 is as high as x^2, and \vec{x}_2 as deep as x_2.
        let ((w, h, d), _, glyphs) = drawn("\\vec{x}^2");
        assert!(close(h, 0.8141079999999999) && close(d, 0.0), "{h} {d}");
        let ((w2, _, _), _, plain) = drawn("x^2");
        assert!(close(w, w2), "{w} {w2}");
        assert!(close(glyph_at(&glyphs, "2").2, glyph_at(&plain, "2").2));
        assert!(close(glyph_at(&glyphs, "2").1, glyph_at(&plain, "2").1));
        let ((_, h, d), paths, _) = drawn("\\vec{x}_2");
        assert!(close(h, 0.714) && close(d, 0.15), "{h} {d}");
        assert_eq!(paths.len(), 1);
        let ((_, h, d), _, _) = drawn("\\vec{x}_2^2");
        assert!(
            close(h, 0.8141079999999999) && close(d, 0.24810799999999997),
            "{h} {d}"
        );
        // A hat over a letter: the accent is above the delegated base.
        let ((_, h, _), _, glyphs) = drawn("\\hat x^2");
        assert!(h > 0.8141079999999999 - 1e-9);
        let (_, hx, _) = glyph_at(&glyphs, "^");
        let hat = metrics::glyph('^', Face::MainRegular, Mode::Math).unwrap();
        let x = metrics::glyph('x', Face::MathItalic, Mode::Math).unwrap();
        assert!(close(*hx, x.width / 2.0 + x.skew - hat.width / 2.0), "{hx}");
        // A color around the character is seen through, as KaTeX's `getBaseElem` does: the
        // script is placed as on a bare x.
        let ((_, h, _), _, colored) = drawn("\\hat{\\textcolor{red}{x}}^2");
        let ((_, h2, _), _, plain) = drawn("\\hat{x}^2");
        assert!(close(h, h2), "{h} {h2}");
        assert!(close(glyph_at(&colored, "2").2, glyph_at(&plain, "2").2));
        assert!(close(glyph_at(&colored, "^").1, glyph_at(&plain, "^").1));
    }

    #[test]
    fn scripts_on_an_accent_over_several_characters_are_ordinary() {
        // The accent's height raises the script (oracle: 1.187338 for \widehat{AB}^2).
        let ((_, h, d), _, _) = drawn("\\widehat{AB}^2");
        assert!(close(h, 1.187338) && close(d, 0.0), "{h} {d}");
        let ((_, h, d), _, _) = drawn("\\widehat{AB}_2^2");
        assert!(close(h, 1.187338) && close(d, 0.247), "{h} {d}");
        let ((_, h, _), _, _) = drawn("\\overrightarrow{AB}_2^2");
        assert!(close(h, 1.409338), "{h}");
    }

    #[test]
    fn an_under_arrow_hangs_below_the_base() {
        // Oracle: \underrightarrow{x} is 0.43056 high and 0.522 deep.
        let ((w, h, d), paths, glyphs) = drawn("\\underrightarrow{x}");
        assert!(close(h, 0.43056) && close(d, 0.522), "{h} {d}");
        let (_, _, y) = glyph_at(&glyphs, "x");
        assert!(close(*y, 0.0));
        let [(rect, ..)] = paths.as_slice() else {
            panic!("one path: {paths:?}");
        };
        assert!(
            close(rect[1], 0.0) && close(rect[2], w.max(0.888)),
            "{rect:?}"
        );
        // A wider base: the arrow is as wide as it.
        let ((w, h, d), paths, _) = drawn("\\underrightarrow{ab}");
        assert!(close(h, 0.69444) && close(d, 0.522), "{h} {d}");
        assert!(close(paths[0].0[2], w), "{w}");
        // A script goes on the accent as on any other base (oracle).
        let ((_, h, d), _, _) = drawn("\\underrightarrow{x}_2");
        assert!(close(h, 0.43056) && close(d, 0.5717), "{h} {d}");
    }

    #[test]
    fn a_wavy_underline_is_set_off_its_base() {
        // \utilde adds 0.12em between the base and the image (0.26em high for one character).
        let ((_, h, d), paths, _) = drawn("\\utilde{x}");
        assert!(close(h, 0.43056) && close(d, 0.12 + 0.26), "{h} {d}");
        assert_eq!(paths[0].2, svg_data::path("tilde1").unwrap());
    }

    #[test]
    fn a_text_accent_sits_on_the_letter() {
        let ((w, h, d), _, glyphs) = drawn("\\text{\\'e}");
        let e = metrics::glyph('e', Face::MainRegular, Mode::Text).unwrap();
        let acute = metrics::glyph('\u{2ca}', Face::MainRegular, Mode::Text).unwrap();
        let (_, ax, ay) = glyph_at(&glyphs, "\u{2ca}");
        assert!(
            close(*ax, e.width / 2.0 + e.skew - acute.width / 2.0),
            "{ax}"
        );
        let clearance = e.height.min(0.43056);
        assert!(close(*ay, -(e.height - clearance - acute.depth)), "{ay}");
        assert!(
            close(h, e.height - clearance + acute.height + acute.depth),
            "{h}"
        );
        assert!(close(d, e.depth.max(0.0)) && close(w, e.width), "{w} {d}");
    }

    #[test]
    fn a_circle_accent_is_full_width_and_dropped() {
        let ((w, _, _), _, glyphs) = drawn("\\text{\\textcircled{a}}");
        let a = metrics::glyph('a', Face::MainRegular, Mode::Text).unwrap();
        let circle = metrics::glyph('\u{25ef}', Face::MainRegular, Mode::Text).unwrap();
        let (_, cx, cy) = glyph_at(&glyphs, "\u{25ef}");
        // It overlays the character, 0.2em lower than its box, which is as high as the
        // character. The circle is wider, so the rows, centered, put the letter in its middle.
        let row = circle.width.max(a.width);
        assert!(close(w, row), "{w}");
        assert!(close(*cx, (row - circle.width) / 2.0 + a.skew), "{cx}");
        assert!(close(*cy, 0.2 - circle.depth), "{cy}");
        let (_, ax, _) = glyph_at(&glyphs, "a");
        assert!(close(*ax, (row - a.width) / 2.0), "{ax}");
    }

    #[test]
    fn a_cedilla_hangs_below_the_character() {
        let (_, _, d) = drawn("\\text{\\c c}").0;
        let c = metrics::glyph('c', Face::MainRegular, Mode::Text).unwrap();
        assert!(d >= c.depth, "{d}");
        let (_, _, plain) = drawn("\\text{c}").0;
        assert!(d >= plain);
    }

    #[test]
    fn an_accent_is_ordinary() {
        // A hat over a plus is an ordinary atom: no binary operator spaces around it.
        let accented = drawn("a\\hat{+}b").0.0;
        let grouped = drawn("a{+}b").0.0;
        let binary = drawn("a+b").0.0;
        assert!(
            close(accented, grouped) && accented < binary,
            "{accented} {binary}"
        );
        let one = drawn("a\\hat{b}").0.0;
        let two = drawn("a\\hat{b}c").0.0;
        assert!(two > one);
    }

    #[test]
    fn odd_input_does_not_panic() {
        for src in [
            "\\hat{}",
            "\\vec{}",
            "\\widehat{}",
            "\\widehat{}^2",
            "\\utilde{}",
            "\\underrightarrow{}",
            "\\hat\\hat x",
            "\\hat{\\hat x}^2_3",
            "\\overrightarrow{\\overrightarrow{x}}",
            "\\text{\\'{}}",
            "\\c{}",
            "\\hat{\\text{\\'e}}",
            "\\mathbb{R}\\hat{\\mathbb R}",
        ] {
            let Ok(nodes) = parse_with(
                src,
                Settings {
                    display_mode: false,
                },
            ) else {
                continue;
            };
            let _ = layout(&nodes, false);
        }
        let long = "x".repeat(5000);
        let ((w, ..), paths, _) = drawn(&format!("\\widehat{{{long}}}"));
        assert!(w > 0.0 && paths.len() == 1);
        let (_, paths, _) = drawn(&format!("\\underleftrightarrow{{{long}}}"));
        assert!(paths.len() <= 3);
    }
}
