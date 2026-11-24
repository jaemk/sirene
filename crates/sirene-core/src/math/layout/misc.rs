//! Nodes that change a box's size or place without new shapes: phantoms, `\smash`, the laps,
//! `\raisebox`, `\vcenter`, `\underline`, `\overline`, `\pmb`, `\reflectbox`, `\mathchoice`,
//! `\html@mathml`, `\hbox`, and `\verb`.
//!
//! Adapted from KaTeX's `src/functions/phantom.ts`, `smash.ts`, `lap.ts`, `raisebox.ts`,
//! `vcenter.ts`, `underline.ts`, `overline.ts`, `pmb.ts`, `reflectbox.ts`, `mathchoice.ts`,
//! `htmlmathml.ts`, `hbox.ts`, and `verb.ts` (MIT, Copyright (c) 2013-2020 Khan Academy and other
//! contributors). `Bx::shadowed` and `Bx::mirrored` draw `\pmb` and `\reflectbox`. A phantom
//! builds its body with `Options::with_phantom`, which draws it transparent; its size is the
//! body's.

use std::borrow::Cow;
use std::ops::Range;

use super::{Atom, Built, Class, Grouping, build, expression};
use crate::math::{
    MathError,
    ast::{MathClass, Measurement, Node},
    boxes::{Bx, Part, Position},
    metrics_data::Face,
    options::{Em, Options},
    symbols::{self, Mode, Name},
};

/// `\phantom`: the body's atoms, drawn transparent.
pub(super) fn phantom<'t>(
    body: &'t [Node<'_>],
    options: &Options<'t>,
    _span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let atoms = expression(body, &options.with_phantom(), Grouping::Partial)?;
    Ok(Built::Many(atoms))
}

/// `\vphantom`: the body's height and depth and no width.
pub(super) fn vphantom<'t>(
    body: &'t Node<'_>,
    options: &Options<'t>,
    _span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let inner = build(body, &options.with_phantom())?.into_box();
    let strut = Bx::space(Em::ZERO, inner.height, inner.depth);
    Ok(Built::One(Atom::new(strut, Some(Class::Ord), options)))
}

/// `\smash`: `height` and `depth` say which of the two are set to zero.
pub(super) fn smash<'t>(
    body: &'t Node<'_>,
    (height, depth): (bool, bool),
    options: &Options<'t>,
    _span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let mut node = Bx::hlist(vec![build(body, options)?.into_box()]);
    if !height && !depth {
        return Ok(Built::One(Atom::new(node, None, options)));
    }
    if height {
        node.height = Em::ZERO;
    }
    if depth {
        node.depth = Em::ZERO;
    }
    // A symmetric smash stays in the line; an asymmetric one goes through a vertical stack, which
    // takes the reset height or depth of its one box.
    let bx = if height && depth {
        node
    } else {
        Bx::vlist(Position::FirstBaseline, vec![Part::of(node)])
    };
    Ok(Built::One(Atom::new(bx, Some(Class::Ord), options)))
}

/// `\mathllap`, `\mathrlap`, `\mathclap` (and the text forms): `alignment` is `llap`, `rlap`,
/// or `clap`. The box has no width: the body hangs to the left, to the right, or both ways.
pub(super) fn lap<'t>(
    alignment: &'t str,
    body: &'t Node<'_>,
    options: &Options<'t>,
    _span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let inner = build(body, options)?.into_box();
    let hang = match alignment {
        "llap" => inner.width,
        "clap" => inner.width / 2.0,
        _ => Em::ZERO,
    };
    let rest = inner.width - hang;
    let bx = Bx::hlist(vec![Bx::kern(-hang), inner, Bx::kern(-rest)]);
    Ok(Built::One(Atom::new(bx, Some(Class::Ord), options)))
}

pub(super) fn raise_box<'t>(
    dy: &'t Measurement,
    body: &'t Node<'_>,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let inner = build(body, options)?.into_box();
    let dy = options.length(dy, span)?;
    let bx = Bx::vlist(Position::Shift(-dy), vec![Part::of(inner)]);
    Ok(Built::One(Atom::new(bx, None, options)))
}

/// `\vcenter`: the body centered on the math axis.
pub(super) fn vcenter<'t>(
    body: &'t Node<'_>,
    options: &Options<'t>,
    _span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let inner = build(body, options)?.into_box();
    let axis = Em(options.metrics().axis_height);
    let dy = ((inner.height - axis) - (inner.depth + axis)) * 0.5;
    let bx = Bx::vlist(Position::Shift(dy), vec![Part::of(inner)]);
    Ok(Built::One(Atom::new(bx, None, options)))
}

/// TeXbook rule 10: a line below the body, a threefold rule thickness clear of it.
pub(super) fn underline<'t>(
    body: &'t Node<'_>,
    options: &Options<'t>,
    _span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let inner = build(body, options)?.into_box();
    let rule = Em(options.metrics().default_rule_thickness);
    let line = Bx::line(inner.width, rule, options);
    let bx = Bx::vlist(
        Position::Top(inner.height),
        vec![
            Part::Kern(rule),
            Part::of(line),
            Part::Kern(rule * 3.0),
            Part::of(inner),
        ],
    );
    Ok(Built::One(Atom::new(bx, Some(Class::Ord), options)))
}

/// TeXbook rule 9: a line over the body, which is set cramped.
pub(super) fn overline<'t>(
    body: &'t Node<'_>,
    options: &Options<'t>,
    _span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let inner = build(body, &options.having_cramped_style())?.into_box();
    let rule = Em(options.metrics().default_rule_thickness);
    let line = Bx::line(inner.width, rule, options);
    let bx = Bx::vlist(
        Position::FirstBaseline,
        vec![
            Part::of(inner),
            Part::Kern(rule * 3.0),
            Part::of(line),
            Part::Kern(rule),
        ],
    );
    Ok(Built::One(Atom::new(bx, Some(Class::Ord), options)))
}

/// `\pmb`: the body drawn twice, offset, as KaTeX's text shadow.
pub(super) fn pmb<'t>(
    class: MathClass,
    body: &'t [Node<'_>],
    options: &Options<'t>,
    _span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let atoms = expression(body, options, Grouping::Group)?;
    let inner = Bx::hlist(atoms.into_iter().map(|a| a.bx).collect());
    let bx = Bx::shadowed(inner);
    Ok(Built::One(Atom::new(bx, Some(class.into()), options)))
}

pub(super) fn reflect_box<'t>(
    body: &'t Node<'_>,
    options: &Options<'t>,
    _span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let bx = Bx::mirrored(build(body, options)?.into_box());
    Ok(Built::One(Atom::new(bx, Some(Class::Ord), options)))
}

/// `\mathchoice`: the four bodies in display, text, script, and scriptscript order. The one for
/// the current style joins the list around it.
pub(super) fn math_choice<'t>(
    bodies: [&'t [Node<'_>]; 4],
    options: &Options<'t>,
    _span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let [display, text, script, script_script] = bodies;
    let body = match options.style.size() {
        0 => display,
        1 => text,
        2 => script,
        _ => script_script,
    };
    Ok(Built::Many(expression(body, options, Grouping::Partial)?))
}

/// `\html@mathml`: the HTML branch is drawn.
pub(super) fn html_mathml<'t>(
    html: &'t [Node<'_>],
    options: &Options<'t>,
    _span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    Ok(Built::Many(expression(html, options, Grouping::Partial)?))
}

/// `\hbox`: its body in text, in no font of its own.
pub(super) fn hbox<'t>(
    body: &'t [Node<'_>],
    options: &Options<'t>,
    _span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let inner = Options {
        font: None,
        ..*options
    };
    Ok(Built::Many(expression(body, &inner, Grouping::Partial)?))
}

/// `\verb`: each character in the typewriter face at text size. A space is a no-break space, or
/// the visible space of `\verb*`.
pub(super) fn verb<'t>(
    body: &'t str,
    star: bool,
    mode: Mode,
    options: &Options<'t>,
    _span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let inner = options.having_style(options.style.text());
    let glyphs = body
        .chars()
        .map(|c| match c {
            ' ' if star => '\u{2423}',
            ' ' => '\u{a0}',
            c => c,
        })
        .map(|c| {
            let key = c.to_string();
            let name = if c == '~' {
                Name::Control("textasciitilde")
            } else {
                Name::Char(&key)
            };
            let text = match symbols::lookup(mode, name) {
                Some(symbol) => Cow::Borrowed(symbol.replace),
                None => Cow::Owned(key.clone()),
            };
            Bx::glyph(text, Face::TypewriterRegular, mode, &inner)
        })
        .collect();
    let factor = inner.size_multiplier() / options.size_multiplier();
    let bx = Bx::scaled(factor, Bx::hlist(glyphs));
    Ok(Built::One(Atom::new(bx, Some(Class::Ord), options)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{
        boxes::{BoxKind, Item},
        layout::layout,
        metrics,
        parser::{Settings, parse_with},
    };

    fn laid_out(src: &str) -> Bx<'static> {
        let src: &'static str = Box::leak(src.to_owned().into_boxed_str());
        let nodes = parse_with(
            src,
            Settings {
                display_mode: false,
            },
        )
        .unwrap();
        // The box borrows the parse tree; the tests only read it, so leaking it is harmless.
        let nodes: &'static [Node<'static>] = Box::leak(nodes.into_boxed_slice());
        layout(nodes, false).unwrap()
    }

    fn dims(src: &str) -> (f64, f64, f64) {
        let bx = laid_out(src);
        (bx.width.0, bx.height.0, bx.depth.0)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    /// Asserts a box's height and depth, which the KaTeX oracle (or its formulas) gives.
    fn assert_hd(src: &str, height: f64, depth: f64) {
        let (_, h, d) = dims(src);
        assert!(
            close(h, height) && close(d, depth),
            "{src}: ({h}, {d}) vs ({height}, {depth})"
        );
    }

    #[test]
    fn phantoms_take_the_size_and_draw_transparent() {
        // Oracle: \phantom{x^2} is 0.814108 high, 0 deep.
        assert_hd("\\phantom{x^2}", 0.8141079999999999, 0.0);
        assert_hd("\\phantom x", 0.43056, 0.0);
        let (w, ..) = dims("x^2");
        let (pw, ..) = dims("\\phantom{x^2}");
        assert!(close(w, pw));
        let colors: Vec<_> = laid_out("\\phantom{x}")
            .items()
            .into_iter()
            .map(|item| match item {
                Item::Glyph { glyph, .. } => glyph.color,
                _ => None,
            })
            .collect();
        assert_eq!(colors, [Some("transparent")]);
    }

    #[test]
    fn vphantom_has_height_and_depth_but_no_width() {
        let (w, h, d) = dims("\\vphantom{x_2}");
        assert!(
            close(w, 0.0) && close(h, 0.43056) && close(d, 0.15),
            "{w} {h} {d}"
        );
        assert!(laid_out("\\vphantom{x}").items().is_empty());
    }

    #[test]
    fn hphantom_has_width_but_no_height_or_depth() {
        // Oracle: \hphantom{x^2} is 0 by 0.
        assert_hd("\\hphantom{x^2}", 0.0, 0.0);
        let (w, ..) = dims("x^2");
        let (hw, ..) = dims("\\hphantom{x^2}");
        assert!(close(w, hw));
    }

    #[test]
    fn smash_zeroes_what_it_is_told() {
        // Oracle: \smash{x} 0/0, \smash[b]{x} 0.43056/0, \smash[]{x} 0.43056/0.
        assert_hd("\\smash{x}", 0.0, 0.0);
        assert_hd("\\smash[b]{x}", 0.43056, 0.0);
        assert_hd("\\smash[]{x}", 0.43056, 0.0);
        // Formula: \smash[t]{x_2} keeps its 0.15 depth and has no height.
        assert_hd("\\smash[t]{x_2}", 0.0, 0.15);
        assert_hd("\\smash[tb]{x_2}", 0.0, 0.0);
        let (w, ..) = dims("x_2");
        let (sw, ..) = dims("\\smash[t]{x_2}");
        assert!(close(w, sw));
        // The smashed body is still drawn.
        assert_eq!(laid_out("\\smash x").items().len(), 1);
    }

    #[test]
    fn laps_have_no_width_and_place_the_body() {
        // Oracle: {=}\mathllap{/\,} is 0.75 by 0.25.
        for src in ["{=}\\mathllap{/\\,}", "\\mathrlap{\\,/}{=}"] {
            assert_hd(src, 0.75, 0.25);
            let (w, ..) = dims(src);
            let (eq, ..) = dims("{=}");
            assert!(close(w, eq), "{src}: {w}");
        }
        let items = |src: &str| -> Vec<f64> {
            laid_out(src)
                .items()
                .iter()
                .map(|item| match item {
                    Item::Glyph { x, .. } => *x,
                    _ => f64::NAN,
                })
                .collect()
        };
        let (a, _, _) = dims("a");
        // rlap: the body starts at the lap's left edge, past the "a".
        assert!(close(items("a\\mathrlap{b}")[1], a));
        // llap: it ends there.
        let (b, _, _) = dims("b");
        assert!(close(items("a\\mathllap{b}")[1], a - b));
        // clap: centered on it.
        assert!(close(items("a\\mathclap{b}")[1], a - b / 2.0));
    }

    #[test]
    fn raisebox_shifts_by_its_length() {
        // Oracle.
        assert_hd("\\raisebox{5pt}{text}", 1.1150799999999998, 0.0);
        assert_hd("\\raisebox{-5pt}{text}", 0.11507999999999996, 0.5);
        let (w, ..) = dims("\\raisebox{5pt}{text}");
        let (tw, ..) = dims("\\text{text}");
        assert!(close(w, tw));
    }

    #[test]
    fn vcenter_centers_on_the_axis() {
        // Oracle.
        assert_hd("\\vcenter{\\frac a b}", 0.770196, 0.270196);
        // Formula: a 2em rule centered on the axis is 1em above it and 1em below.
        assert_hd("\\vcenter{\\rule{1em}{2em}}", 1.25, 0.75);
    }

    #[test]
    fn underline_sets_a_line_below() {
        // Formula (TeXbook rule 10, rule thickness 0.04): depth is 0.04 + 0.04 + 0.12 plus the
        // body's, height is the body's.
        let (w, h, d) = dims("\\underline{x}");
        assert!(close(h, 0.43056) && close(d, 0.2), "{h} {d}");
        assert!(close(w, dims("x").0));
        let (_, _, d) = dims("\\underline{x_2}");
        assert!(close(d, 0.35), "{d}");
        let items = laid_out("\\underline{x}").items();
        assert!(items.iter().any(|i| matches!(i, Item::Rule { .. })));
    }

    #[test]
    fn overline_sets_a_line_above_in_cramped_style() {
        // Oracle: \mathop{\overline{\mathrm{lim}}} reaches 0.89444 = 0.69444 + 0.2.
        let (w, h, d) = dims("\\overline{x}");
        assert!(close(h, 0.43056 + 0.2) && close(d, 0.0), "{h} {d}");
        assert!(close(w, dims("x").0));
        // Cramped, so a superscript rises less than uncramped (0.363 rather than 0.413).
        let (_, h, _) = dims("\\overline{x^2}");
        let (_, plain, _) = dims("x^2");
        assert!(h < plain + 0.2 - 1e-6, "{h} {plain}");
    }

    #[test]
    fn pmb_draws_a_shadowed_copy() {
        // Oracle.
        assert_hd("\\pmb{+}", 0.58333, 0.08333);
        assert_hd("\\pmb{\\mu}", 0.43056, 0.19444);
        assert_hd("\\pmb{}", 0.0, 0.0);
        let bx = laid_out("\\pmb{x}");
        assert_eq!(bx.items().len(), 2);
        let has_shadow = |bx: &Bx<'_>| match &bx.kind {
            BoxKind::HList(children) => children
                .iter()
                .any(|c| matches!(c.kind, BoxKind::Shadowed(_))),
            _ => false,
        };
        assert!(has_shadow(&bx));
    }

    #[test]
    fn reflectbox_mirrors_the_body() {
        // Oracle.
        assert_hd("\\reflectbox{abc}", 0.69444, 0.0);
        assert_hd("\\mathreflectbox{x^2}", 0.8141079999999999, 0.0);
        assert_eq!(dims("\\reflectbox{abc}").0, dims("\\text{abc}").0);
        let items = laid_out("\\mathreflectbox{x}").items();
        assert!(matches!(
            items.as_slice(),
            [Item::Glyph { mirrored: true, .. }]
        ));
    }

    #[test]
    fn mathchoice_picks_by_style() {
        let width = |src: &str| dims(src).0;
        let (d, t, s, ss) = (width("D"), width("T"), width("S"), width("SS"));
        let choice = "\\mathchoice{D}{T}{S}{SS}";
        assert!(close(width(&format!("\\displaystyle{choice}")), d));
        assert!(close(width(&format!("\\textstyle{choice}")), t));
        // A script is set at 0.7 of the size, a scriptscript at 0.5.
        assert!(close(width(&format!("\\scriptstyle{choice}")), s * 0.7));
        assert!(close(
            width(&format!("\\scriptscriptstyle{choice}")),
            ss * 0.5
        ));
        // Text style is the formula's default here.
        assert!(close(width(choice), t));
    }

    #[test]
    fn html_mathml_draws_the_html_branch() {
        assert_eq!(dims("\\html@mathml{x}{y}"), dims("x"));
    }

    #[test]
    fn hbox_sets_text_in_no_font() {
        assert_eq!(dims("\\mathbf{\\hbox{x}}"), dims("\\hbox{x}"));
        assert_ne!(dims("\\mathbf{x}").0, dims("\\hbox{x}").0);
        assert_eq!(dims("\\hbox{ab}"), dims("\\text{ab}"));
    }

    #[test]
    fn verb_sets_typewriter_text() {
        let tt = |c: char| metrics::glyph(c, Face::TypewriterRegular, Mode::Text).unwrap();
        let (w, h, d) = dims("\\verb|ab|");
        assert!(close(w, tt('a').width + tt('b').width), "{w}");
        assert!(close(h, tt('a').height.max(tt('b').height)), "{h}");
        assert!(close(d, tt('a').depth.max(tt('b').depth)), "{d}");
        // A space is a no-break space; \verb* draws the visible space.
        let text = |src: &str| -> Vec<String> {
            laid_out(src)
                .items()
                .into_iter()
                .filter_map(|item| match item {
                    Item::Glyph { glyph, .. } => Some(glyph.text.into_owned()),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(text("\\verb|a b|"), ["a", "\u{a0}", "b"]);
        assert_eq!(text("\\verb*|a b|"), ["a", "\u{2423}", "b"]);
        assert_eq!(text("\\verb|~|"), ["~"]);
        // Text size even in a script: the glyph is drawn at the formula's size.
        let (sw, ..) = dims("\\scriptstyle\\verb|a|");
        assert!(close(sw, tt('a').width), "{sw}");
        let items = laid_out("\\scriptstyle\\verb|a|").items();
        assert!(matches!(
            items.as_slice(),
            [Item::Glyph { scale, .. }] if close(*scale, 1.0)
        ));
        let items = laid_out("\\scriptstyle a").items();
        assert!(matches!(
            items.as_slice(),
            [Item::Glyph { scale, .. }] if close(*scale, 0.7)
        ));
    }
}
