//! Braces and brackets over or under a base: `\overbrace`, `\underbrace`, `\overbracket`,
//! `\underbracket`.
//!
//! Adapted from KaTeX's `src/functions/horizBrace.ts` (MIT, Copyright (c) 2013-2020 Khan
//! Academy and other contributors), with `stretchy` for the brace shapes. The brace is as wide
//! as its base, and at least the brace's minimum width.

use std::ops::Range;

use super::{Atom, Built, Class, build, build_in, stretchy};
use crate::math::{
    MathError,
    ast::{Node, NodeKind},
    boxes::{Bx, Part, Position},
    options::{Em, Options},
    style::Style,
};

/// Boxes stacked with each centered over the widest, as the `.mover` and `.munder` classes
/// center them.
fn centered<'t>(bx: Bx<'t>, width: Em) -> Part<'t> {
    Part::Box {
        dx: (width - bx.width) / 2.0,
        child: bx,
    }
}

/// The base with its brace over or under it.
fn brace_stack<'t>(
    label: &str,
    is_over: bool,
    base: &'t Node<'_>,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Bx<'t>, MathError> {
    // The base is always set in display style.
    let body = build(base, &options.having_base_style(Style::Display))?.into_box();
    let brace = stretchy::svg_span(label, None, body.width, options, span)?;
    let width = body.width.max(brace.width);
    let gap = Em(0.1);
    if is_over {
        Ok(Bx::vlist(
            Position::FirstBaseline,
            vec![centered(body, width), Part::Kern(gap), Part::of(brace)],
        ))
    } else {
        let depth = body.depth + gap + brace.height;
        Ok(Bx::vlist(
            Position::Bottom(depth),
            vec![Part::of(brace), Part::Kern(gap), centered(body, width)],
        ))
    }
}

pub(super) fn horiz_brace<'t>(
    label: &'t str,
    is_over: bool,
    base: &'t Node<'_>,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let bx = brace_stack(label, is_over, base, options, span)?;
    Ok(Built::One(Atom::new(bx, Some(Class::Inner), options)))
}

/// A brace's label: a superscript on an over brace or a subscript on an under brace, which
/// KaTeX's brace builder places itself (`htmlBuilderDelegate`); `None` for any other base.
pub(super) fn supsub<'t>(
    base: &'t Node<'_>,
    sup: Option<&'t Node<'_>>,
    sub: Option<&'t Node<'_>>,
    options: &Options<'t>,
) -> Result<Option<Built<'t>>, MathError> {
    let NodeKind::HorizBrace {
        label,
        is_over,
        base: inner,
    } = &base.kind
    else {
        return Ok(None);
    };
    // The brace takes the script on its own side: a superscript over, a subscript under.
    if sub.is_none() != *is_over {
        return Ok(None);
    }
    // The label is the superscript if there is one, as KaTeX picks it.
    let style = options.style;
    let note = match (sup, sub) {
        (Some(sup), _) => build_in(sup, &options.having_style(style.sup()), options)?,
        (None, Some(sub)) => build_in(sub, &options.having_style(style.sub()), options)?,
        (None, None) => return Ok(None),
    }
    .into_box();

    // The note can be wider than the base; the base controls the brace's width.
    let stack = brace_stack(label, *is_over, inner, options, base.span.clone())?;
    let width = stack.width.max(note.width);
    let gap = Em(0.2);
    let bx = if *is_over {
        Bx::vlist(
            Position::FirstBaseline,
            vec![
                centered(stack, width),
                Part::Kern(gap),
                centered(note, width),
            ],
        )
    } else {
        let depth = stack.depth + gap + note.height + note.depth;
        Bx::vlist(
            Position::Bottom(depth),
            vec![
                centered(note, width),
                Part::Kern(gap),
                centered(stack, width),
            ],
        )
    };
    Ok(Some(Built::One(Atom::new(bx, Some(Class::Inner), options))))
}

#[cfg(test)]
mod tests {
    use super::super::layout;
    use crate::math::{
        boxes::{Align, Fit, Item},
        metrics,
        metrics_data::Face,
        parser::{Settings, parse_with},
        svg_data,
        symbols::Mode,
    };

    /// A formula's width, height, and depth, the path items (rectangle, view box, fit, data), and
    /// the glyphs (text, x, y).
    type Drawn = (
        (f64, f64, f64),
        Vec<([f64; 4], [f64; 4], Fit, String)>,
        Vec<(String, f64, f64)>,
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
                Item::Glyph { glyph, x, y, .. } => glyphs.push((glyph.text.into_owned(), x, y)),
                Item::Rule { .. } => {}
            }
        }
        ((bx.width.0, bx.height.0, bx.depth.0), paths, glyphs)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    // Heights and depths are KaTeX's, from the oracle fixture
    // `tests/fixtures/math/katex-oracle.json`, or its formulas worked through with the font
    // metrics (a brace is 0.548em high, 0.1em off its base, and its note 0.2em off it).

    #[test]
    fn an_over_brace_is_three_paths_above_the_base() {
        let ((w, h, d), paths, glyphs) = drawn("\\overbrace{abc}", false);
        // The body is in display style: b is 0.69444 high.
        assert!(close(h, 0.69444 + 0.1 + 0.548) && close(d, 0.0), "{h} {d}");
        assert!(close(w, 1.6), "{w}");
        let [left, mid, right] = paths.as_slice() else {
            panic!("three paths: {paths:?}");
        };
        let top = -(0.69444 + 0.1 + 0.548);
        for (path, fit, name, x, width) in [
            (left, Fit::Crop(Align::Min), "leftbrace", 0.0, 0.251 * 1.6),
            (
                mid,
                Fit::Crop(Align::Mid),
                "midbrace",
                0.25 * 1.6,
                0.5 * 1.6,
            ),
            (
                right,
                Fit::Crop(Align::Max),
                "rightbrace",
                0.749 * 1.6,
                0.251 * 1.6,
            ),
        ] {
            let ([px, py, pw, ph], view_box, path_fit, data) = path;
            assert!(close(*px, x) && close(*py, top), "{name}: {px} {py}");
            assert!(close(*pw, width) && close(*ph, 0.548), "{name}: {pw} {ph}");
            assert_eq!(*view_box, [0.0, 0.0, 400_000.0, 548.0]);
            assert_eq!(
                (*path_fit, data.as_str()),
                (fit, svg_data::path(name).unwrap())
            );
        }
        // The base, narrower than the brace, is centered under it.
        let (_, a, _) = glyphs.iter().find(|(t, ..)| t == "a").unwrap();
        let width = |c: char| {
            let m = metrics::glyph(c, Face::MathItalic, Mode::Math).unwrap();
            m.width + m.italic.max(0.0)
        };
        let base = width('a') + width('b') + width('c');
        assert!(close(*a, (1.6 - base) / 2.0), "{a}");
    }

    #[test]
    fn an_over_brace_with_a_note_above() {
        let ((_, h, d), paths, glyphs) = drawn("\\overbrace{abc}^{n}", false);
        let note = 0.43056 * 0.7;
        assert!(
            close(h, 0.69444 + 0.1 + 0.548 + 0.2 + note) && close(d, 0.0),
            "{h} {d}"
        );
        assert_eq!(paths.len(), 3);
        // The note is above the brace and centered over it.
        let (_, nx, ny) = glyphs.iter().find(|(t, ..)| t == "n").unwrap();
        assert!(close(*ny, -(0.69444 + 0.1 + 0.548 + 0.2)), "{ny}");
        let m = metrics::glyph('n', Face::MathItalic, Mode::Math).unwrap();
        let n_width = (m.width + m.italic.max(0.0)) * 0.7;
        assert!(close(*nx, (1.6 - n_width) / 2.0), "{nx}");
    }

    #[test]
    fn an_under_brace_is_below_the_base() {
        let ((_, h, d), paths, glyphs) = drawn("\\underbrace{abc}", false);
        assert!(close(h, 0.69444) && close(d, 0.1 + 0.548), "{h} {d}");
        let [left, mid, right] = paths.as_slice() else {
            panic!("three paths: {paths:?}");
        };
        // The brace hangs below the baseline, its top 0.1em under the base's depth.
        assert!(close(left.0[1], 0.1) && close(mid.0[1], 0.1) && close(right.0[1], 0.1));
        assert_eq!(left.3, svg_data::path("leftbraceunder").unwrap());
        assert_eq!(mid.3, svg_data::path("midbraceunder").unwrap());
        assert_eq!(right.3, svg_data::path("rightbraceunder").unwrap());
        let (_, _, ay) = glyphs.iter().find(|(t, ..)| t == "a").unwrap();
        assert!(close(*ay, 0.0), "{ay}");
    }

    #[test]
    fn an_under_brace_with_a_note_below() {
        let ((_, h, d), paths, glyphs) = drawn("\\underbrace{abc}_{n}", false);
        let note = 0.43056 * 0.7;
        assert!(close(h, 0.69444), "{h}");
        assert!(close(d, 0.1 + 0.548 + 0.2 + note), "{d}");
        let [left, ..] = paths.as_slice() else {
            panic!("paths: {paths:?}");
        };
        assert!(close(left.0[1], 0.1));
        let (_, _, ny) = glyphs.iter().find(|(t, ..)| t == "n").unwrap();
        assert!(close(*ny, 0.1 + 0.548 + 0.2 + note), "{ny}");
    }

    #[test]
    fn a_note_on_the_far_side_is_an_ordinary_script() {
        // A subscript on an over brace and a superscript on an under brace are laid out as
        // scripts, KaTeX's delegate leaving them to the script builder (oracle values).
        let (_, h, d) = drawn("\\overbrace{x}_2", false).0;
        let (_, h2, d2) = drawn("\\overbrace{x}", false).0;
        assert!(h > 0.0 && d > d2 && close(h2, 1.07856), "{h} {d} {h2} {d2}");
        let (_, h, _) = drawn("\\underbrace{x}^2", false).0;
        assert!(h > 0.43056, "{h}");
    }

    #[test]
    fn matches_katex_on_the_oracle_heights() {
        let (_, h, d) = drawn("\\overbrace{x}", false).0;
        assert!(close(h, 1.07856) && close(d, 0.0));
        let (_, h, d) = drawn("\\overbrace{x}^2", false).0;
        assert!(close(h, 1.7296679999999998) && close(d, 0.0), "{h}");
        let (_, h, d) = drawn("\\underbrace{x}_2", false).0;
        assert!(close(h, 0.43056) && close(d, 1.299108), "{h} {d}");
        // The superscript is the note when both are given under a brace.
        let (_, h, d) = drawn("\\underbrace{x}_2^2", false).0;
        assert!(close(h, 0.43056) && close(d, 1.299108), "{h} {d}");
    }

    #[test]
    fn brackets_use_their_own_heights() {
        let (_, h, _) = drawn("\\overbracket{x}", false).0;
        assert!(close(h, 0.43056 + 0.1 + 0.440), "{h}");
        let (_, _, d) = drawn("\\underbracket{x}", false).0;
        assert!(close(d, 0.1 + 0.410), "{d}");
        let (_, paths, _) = drawn("\\overbracket{x}", false);
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn a_wide_base_gives_a_fixed_number_of_paths() {
        let body = "x".repeat(1000);
        let ((w, ..), paths, _) = drawn(&format!("\\overbrace{{{body}}}^{{n}}"), false);
        assert_eq!(paths.len(), 3);
        assert!(close(paths[1].0[2], 0.5 * w), "{w}");
    }
}
