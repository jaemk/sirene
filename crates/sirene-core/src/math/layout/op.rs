//! Large and named operators: `\sum`, `\int`, `\lim`, `\mathop{...}`, `\operatorname{...}`, and
//! their limits.
//!
//! Adapted from KaTeX's `src/functions/op.ts`, `src/functions/operatorname.ts`, and
//! `src/functions/utils/assembleSupSub.ts` (MIT, Copyright (c) 2013-2020 Khan Academy and other
//! contributors), with TeX's rule 13 for operators and the placement of limits from the
//! TeXbook, Appendix G.

use std::borrow::Cow;
use std::ops::Range;

use super::{
    Atom, Built, Class, Grouping, build, build_in, cancel_binary_operators, expression,
    insert_spaces, symbol,
};
use crate::math::{
    MathError,
    ast::{Node, NodeKind, OpContent},
    boxes::{BoxKind, Bx, Fit, Part, Position},
    metrics_data::Face,
    options::{Em, MathFont, Options},
    style::Style,
    svg_data,
    symbols::{Mode, Name},
};

/// An operator: what `NodeKind::Op` holds, and where it is.
pub(super) struct Operator<'t, 'n> {
    #[allow(dead_code, reason = "carried from the node; only the callers read it")]
    pub limits: bool,
    pub symbol: bool,
    pub suppress_base_shift: Option<bool>,
    pub content: &'t OpContent<'n>,
    pub mode: Mode,
    pub span: Range<usize>,
}

/// An operator's own box, with what its limits need: the slant of its symbol and how far the
/// symbol sits off the axis.
struct OperatorBase<'t> {
    bx: Bx<'t>,
    slant: Em,
    shift: Em,
}

/// The one large operator that stays small in display style.
const NO_SUCCESSOR: &str = "\\smallint";

fn operator_base<'t>(
    op: &Operator<'t, '_>,
    options: &Options<'t>,
) -> Result<OperatorBase<'t>, MathError> {
    // The slant of a contour integral, whose oval is laid over the glyph of its integral.
    let mut oval_slant = None;
    let bx = match op.content {
        OpContent::Name(name) if op.symbol => {
            // Most symbol operators get larger in display style (rule 13).
            let large = options.style.size() == Style::Display.size() && name != NO_SUCCESSOR;
            let face = if large {
                Face::Size2Regular
            } else {
                Face::Size1Regular
            };
            // There is no glyph with the oval yet, so KaTeX takes the one without and draws
            // the oval over it.
            let (glyph_name, oval) = match name.as_str() {
                "\\oiint" => ("\\iint", Some(("oiint", 0.957, 0.499, 1.472, 0.659))),
                "\\oiiint" => ("\\iiint", Some(("oiiint", 1.304, 0.499, 1.98, 0.659))),
                other => (other, None),
            };
            let symbol_name = Name::Control(glyph_name.strip_prefix('\\').unwrap_or(glyph_name));
            let text = symbol::drawn_text(
                Some(symbol_name),
                &symbol::source_text(symbol_name),
                Mode::Math,
            );
            let glyph = Bx::glyph(text, face, Mode::Math, options);
            match oval {
                Some((stem, small_w, small_h, large_w, large_h)) => {
                    if let BoxKind::Glyph(g) = &glyph.kind {
                        oval_slant = Some(g.italic);
                    }
                    let (key, w, h) = if large {
                        (format!("{stem}Size2"), large_w, large_h)
                    } else {
                        (format!("{stem}Size1"), small_w, small_h)
                    };
                    let data = svg_data::path(&key).ok_or_else(|| {
                        MathError::new(format!("No SVG data for \"{key}\"."), op.span.clone())
                    })?;
                    // The oval is as large as its view box, from its top left corner.
                    let oval = Bx::path(
                        data,
                        [0.0, 0.0, 1000.0 * w, 1000.0 * h],
                        Fit::Stretch,
                        (Em(w), Em(h), Em::ZERO),
                        options,
                    );
                    let drop = if large { Em(0.08) } else { Em::ZERO };
                    Bx::vlist_shifted(vec![(glyph, Em::ZERO, Em::ZERO), (oval, drop, Em::ZERO)])
                }
                None => glyph,
            }
        }
        OpContent::Body(body) => {
            let atoms = expression(body, options, Grouping::Group)?;
            let mut boxes: Vec<Bx<'t>> = atoms.into_iter().map(|atom| atom.bx).collect();
            if let [
                Bx {
                    kind: BoxKind::Glyph(_),
                    ..
                },
            ] = boxes.as_slice()
                && let Some(only) = boxes.pop()
            {
                only
            } else {
                Bx::hlist(boxes)
            }
        }
        OpContent::Name(name) => {
            // A text operator: the characters of the name after its backslash.
            let text = name.strip_prefix('\\').unwrap_or(name);
            let glyphs = text
                .char_indices()
                .map(|(at, c)| {
                    let one = &text[at..at + c.len_utf8()];
                    symbol::math_symbol(Name::Char(one), op.mode, options)
                })
                .collect();
            Bx::hlist(glyphs)
        }
    };
    // A single symbol is centered on the axis (rule 13), unless an `\overset` or `\underset`
    // keeps its base in place. Its slant is its italic correction.
    let axis = Em(options.metrics().axis_height);
    let (slant, shift) = match (&bx.kind, oval_slant) {
        _ if op.suppress_base_shift == Some(true) => (Em::ZERO, Em::ZERO),
        (BoxKind::Glyph(glyph), _) => (glyph.italic, (bx.height - bx.depth) / 2.0 - axis),
        (_, Some(slant)) => (slant, (bx.height - bx.depth) / 2.0 - axis),
        _ => (Em::ZERO, Em::ZERO),
    };
    Ok(OperatorBase { bx, slant, shift })
}

/// An operator without limits. KaTeX moves a lone symbol onto the axis with a relative offset
/// that leaves its box where it was (a few thousandths of an em for the large operator glyphs,
/// up to about 0.2em for a one-glyph `\mathop` body such as `\mathop{.}`); a box here has no
/// such offset, so the symbol is drawn on the baseline and a subscript still follows its italic
/// correction.
pub(super) fn op<'t>(op: Operator<'t, '_>, options: &Options<'t>) -> Result<Built<'t>, MathError> {
    let base = operator_base(&op, options)?;
    Ok(Built::One(Atom::new(base.bx, Some(Class::Op), options)))
}

/// The base of `\operatorname`: its body set upright, with the minus and asterisk drawn as the
/// hyphen and asterisk of `amsopn`.
fn operator_name_base<'t>(
    body: &'t [Node<'_>],
    options: &Options<'t>,
) -> Result<Bx<'t>, MathError> {
    let upright = options.with_font(MathFont::Rm);
    let mut atoms: Vec<Atom<'t>> = Vec::new();
    for node in body {
        // A symbol of any kind is set as an ordinary character.
        let built = match &node.kind {
            NodeKind::Symbol { name, .. } => symbol::ord(
                symbol::source_text(*name),
                Some(*name),
                false,
                node.mode,
                &upright,
                node.span.clone(),
            )?,
            _ => build(node, &upright)?,
        };
        match built {
            Built::One(atom) => atoms.push(atom),
            Built::Many(more) => atoms.extend(more),
        }
    }
    cancel_binary_operators(&mut atoms, false);
    let mut atoms = insert_spaces(atoms, &upright, false);
    for atom in &mut atoms {
        if let BoxKind::Glyph(glyph) = &mut atom.bx.kind {
            // The glyph keeps the size of the character it replaces, as KaTeX's.
            glyph.text = Cow::Owned(
                glyph
                    .text
                    .replacen('\u{2212}', "-", 1)
                    .replacen('\u{2217}', "*", 1),
            );
        }
    }
    Ok(Bx::hlist(atoms.into_iter().map(|atom| atom.bx).collect()))
}

pub(super) fn operator_name<'t>(
    body: &'t [Node<'_>],
    _mode: Mode,
    options: &Options<'t>,
    _span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let bx = operator_name_base(body, options)?;
    Ok(Built::One(Atom::new(bx, Some(Class::Op), options)))
}

/// A limit built in `style`, and the kern between it and the base.
fn limit<'t>(
    node: &'t Node<'_>,
    style: Style,
    options: &Options<'t>,
    kern: impl FnOnce(&Bx<'t>) -> f64,
) -> Result<(Bx<'t>, Em), MathError> {
    let inner = options.having_style(style);
    let elem = build_in(node, &inner, options)?.into_box();
    let kern = Em(kern(&elem));
    Ok((elem, kern))
}

/// The base, superscript, and subscript stacked as limits (KaTeX's `assembleSupSub`): the
/// subscript below the base and the superscript above, each centered, the symbol's slant moving
/// them apart.
fn assemble_limits<'t>(
    base: OperatorBase<'t>,
    sup: Option<&'t Node<'_>>,
    sub: Option<&'t Node<'_>>,
    options: &Options<'t>,
) -> Result<Built<'t>, MathError> {
    let metrics = options.metrics();
    let style = options.style;
    let OperatorBase { bx, slant, shift } = base;
    // The base goes in a box of its own, which is never below the baseline when its glyph is.
    let bx = Bx::hlist(vec![bx]);
    let sub_is_single_character = sub.is_some_and(Node::is_character_box);
    let sup = sup
        .map(|node| {
            limit(node, style.sup(), options, |elem| {
                metrics
                    .big_op_spacing1
                    .max(metrics.big_op_spacing3 - elem.depth.0)
            })
        })
        .transpose()?;
    let sub = sub
        .map(|node| {
            limit(node, style.sub(), options, |elem| {
                metrics
                    .big_op_spacing2
                    .max(metrics.big_op_spacing4 - elem.height.0)
            })
        })
        .transpose()?;
    let spacing5 = Em(metrics.big_op_spacing5);

    // Each row is centered in the stack (KaTeX's `.op-limits` `text-align: center`), and its
    // margin, the slant, moves it by half of that: a row is as wide as its box and margin.
    let mut width = bx.width;
    if let Some((sup, _)) = &sup {
        width = width.max(sup.width + slant);
    }
    if let Some((sub, _)) = &sub {
        width = width.max(sub.width - slant);
    }
    let centered = |row: &Bx<'_>, margin: Em| (width + margin - row.width) / 2.0;
    let base_dx = centered(&bx, Em::ZERO);
    let sup_dx = sup.as_ref().map(|(sup, _)| centered(sup, slant));
    let sub_dx = sub.as_ref().map(|(sub, _)| centered(sub, -slant));
    let (sup_dx, sub_dx) = (sup_dx.unwrap_or(Em::ZERO), sub_dx.unwrap_or(Em::ZERO));

    let stack = match (sup, sub.as_ref()) {
        (Some((sup, sup_kern)), Some((sub, sub_kern))) => {
            let bottom = spacing5 + sub.height + sub.depth + *sub_kern + bx.depth + shift;
            Bx::vlist(
                Position::Bottom(bottom),
                vec![
                    Part::Kern(spacing5),
                    Part::Box {
                        child: sub.clone(),
                        dx: sub_dx,
                    },
                    Part::Kern(*sub_kern),
                    Part::Box {
                        child: bx,
                        dx: base_dx,
                    },
                    Part::Kern(sup_kern),
                    Part::Box {
                        child: sup,
                        dx: sup_dx,
                    },
                    Part::Kern(spacing5),
                ],
            )
        }
        (None, Some((sub, sub_kern))) => {
            // The subscript hangs below the base, which keeps its place less its shift.
            let top = bx.height - shift;
            Bx::vlist(
                Position::Top(top),
                vec![
                    Part::Kern(spacing5),
                    Part::Box {
                        child: sub.clone(),
                        dx: sub_dx,
                    },
                    Part::Kern(*sub_kern),
                    Part::Box {
                        child: bx,
                        dx: base_dx,
                    },
                ],
            )
        }
        (Some((sup, sup_kern)), None) => {
            let bottom = bx.depth + shift;
            Bx::vlist(
                Position::Bottom(bottom),
                vec![
                    Part::Box {
                        child: bx,
                        dx: base_dx,
                    },
                    Part::Kern(sup_kern),
                    Part::Box {
                        child: sup,
                        dx: sup_dx,
                    },
                    Part::Kern(spacing5),
                ],
            )
        }
        (None, None) => bx,
    };
    // The lower limit moved left by the slant; a space of the slant keeps it off what comes
    // before, unless it is one character, which clears it.
    let bx = if sub.is_some() && slant != Em::ZERO && !sub_is_single_character {
        Bx::hlist(vec![Bx::kern(slant), stack])
    } else {
        stack
    };
    Ok(Built::One(Atom::new(bx, Some(Class::Op), options)))
}

/// Scripts on an operator base that takes them as limits (an `Op` with `limits` in display
/// style or `always_handle_supsub`, or an `OperatorName` likewise), as KaTeX's
/// `htmlBuilderDelegate`; `None` for any other base, which `scripts::supsub` lays out.
pub(super) fn supsub<'t>(
    base: &'t Node<'_>,
    sup: Option<&'t Node<'_>>,
    sub: Option<&'t Node<'_>>,
    options: &Options<'t>,
) -> Result<Option<Built<'t>>, MathError> {
    let display = options.style.size() == Style::Display.size();
    match &base.kind {
        NodeKind::Op {
            limits,
            always_handle_supsub,
            symbol,
            suppress_base_shift,
            content,
        } if *limits && (display || *always_handle_supsub) => {
            let operator = Operator {
                limits: *limits,
                symbol: *symbol,
                suppress_base_shift: *suppress_base_shift,
                content,
                mode: base.mode,
                span: base.span.clone(),
            };
            let base = operator_base(&operator, options)?;
            assemble_limits(base, sup, sub, options).map(Some)
        }
        NodeKind::OperatorName {
            body,
            always_handle_supsub,
            limits,
        } if *always_handle_supsub && (display || *limits) => {
            let base = OperatorBase {
                bx: operator_name_base(body, options)?,
                slant: Em::ZERO,
                shift: Em::ZERO,
            };
            assemble_limits(base, sup, sub, options).map(Some)
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::super::layout;
    use crate::math::{
        boxes::Item,
        metrics,
        metrics_data::Face,
        parser::{Settings, parse_with},
        symbols::Mode,
    };

    /// The box of a formula: its width, height, and depth, and the text of its glyphs.
    fn laid_out(src: &str, display: bool) -> ((f64, f64, f64), Vec<String>) {
        let nodes = parse_with(
            src,
            Settings {
                display_mode: display,
            },
        )
        .unwrap();
        let bx = layout(&nodes, display).unwrap();
        let glyphs = bx
            .items()
            .into_iter()
            .filter_map(|item| match item {
                Item::Glyph { glyph, .. } => Some(glyph.text.into_owned()),
                _ => None,
            })
            .collect();
        ((bx.width.0, bx.height.0, bx.depth.0), glyphs)
    }

    /// The x of each glyph, by its text, and the formula's width.
    fn placed(src: &str, display: bool) -> (Vec<(String, f64)>, f64) {
        let nodes = parse_with(
            src,
            Settings {
                display_mode: display,
            },
        )
        .unwrap();
        let bx = layout(&nodes, display).unwrap();
        let glyphs = bx
            .items()
            .into_iter()
            .filter_map(|item| match item {
                Item::Glyph { x, glyph, .. } => Some((glyph.text.into_owned(), x)),
                _ => None,
            })
            .collect();
        (glyphs, bx.width.0)
    }

    #[test]
    fn limits_are_centered_on_the_base() {
        // `lim` over a wider subscript sits in the middle of it (KaTeX's `text-align: center`).
        let (glyphs, width) = placed("\\lim_{xxxxxx}", true);
        let lim = 0.27778 + 0.27778 + 0.83334;
        let x_of = |glyphs: &[(String, f64)], text: &str| {
            glyphs
                .iter()
                .find(|(t, _)| t == text)
                .map(|&(_, x)| x)
                .unwrap()
        };
        let l = x_of(&glyphs, "l");
        assert!((l + lim / 2.0 - width / 2.0).abs() < 1e-4, "{l} in {width}");
        assert!(x_of(&glyphs, "x").abs() < 1e-9, "{glyphs:?}");
        // A lower limit under a slanted integral stays inside the box.
        let (glyphs, _) = placed("\\int\\limits_a", false);
        assert!(x_of(&glyphs, "a") > 0.0, "{glyphs:?}");
    }

    fn sized(src: &str, display: bool) -> (f64, f64, f64) {
        laid_out(src, display).0
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    /// The width of a glyph as a box has it (its italic correction included) and its height and
    /// depth, from the metrics.
    fn glyph(c: char, face: Face) -> (f64, f64, f64, f64) {
        let m = metrics::glyph(c, face, Mode::Math).unwrap();
        (m.width + m.italic.max(0.0), m.height, m.depth, m.italic)
    }

    // The expected heights and depths below are KaTeX's, from the oracle fixture
    // `tests/fixtures/math/katex-oracle.json`; widths and the limits are KaTeX's formulas worked
    // through with the font metrics.

    #[test]
    fn symbols_are_large_only_in_display_style() {
        let (w, h, d) = sized("\\int", false);
        let (iw, ih, id, _) = glyph('\u{222b}', Face::Size1Regular);
        assert!(
            close(w, iw) && close(h, 0.805) && close(d, 0.30612),
            "{w} {h} {d}"
        );
        assert!(close(ih, 0.805) && close(id, 0.30612));
        // Display style takes the Size2 face.
        let (w, h, d) = sized("\\int", true);
        let (iw, ih, id, _) = glyph('\u{222b}', Face::Size2Regular);
        assert!(close(w, iw) && close(h, ih) && close(d, id), "{w} {h} {d}");
        assert!(close(h, 1.36) && close(d, 0.86225));
        // `\smallint` stays small.
        let (_, h, _) = sized("\\smallint", true);
        assert!(close(h, glyph('\u{222b}', Face::Size1Regular).1));
    }

    #[test]
    fn mathop_of_a_symbol_is_the_symbol() {
        let (w, h, d) = sized("\\mathop{\\int}", false);
        assert!(close(w, sized("\\int", false).0));
        assert!(close(h, 0.805) && close(d, 0.30612), "{h} {d}");
        // Anything else is an operator box of its own, which a symbol is not.
        let (w, h, _) = sized("\\mathop{ab}", false);
        let (aw, ah, ..) = glyph('a', Face::MathItalic);
        let (bw, bh, ..) = glyph('b', Face::MathItalic);
        assert!(close(w, aw + bw) && close(h, ah.max(bh)), "{w} {h}");
    }

    #[test]
    fn text_operators_are_upright_names() {
        let ((w, h, d), glyphs) = laid_out("\\sin", false);
        let width: f64 = ['s', 'i', 'n']
            .iter()
            .map(|&c| glyph(c, Face::MainRegular).0)
            .sum();
        assert!(
            close(w, width) && close(h, 0.66786) && close(d, 0.0),
            "{w} {h} {d}"
        );
        assert_eq!(glyphs, ["s", "i", "n"]);
    }

    #[test]
    fn integrals_take_scripts_beside_them() {
        // Text style, and `\nolimits`: ordinary scripts, as the oracle has them.
        let (_, h, d) = sized("\\int_i^n", false);
        assert!(close(h, 0.859292) && close(d, 0.35582), "{h} {d}");
        let (_, h, d) = sized("\\int^2\\nolimits_2", false);
        assert!(close(h, 1.009008) && close(d, 0.35582), "{h} {d}");
        let (_, h, d) = sized("\\sum\\nolimits_3^4 4", false);
        assert!(close(h, 0.954008) && close(d, 0.29971), "{h} {d}");
    }

    #[test]
    fn limits_stack_above_and_below() {
        // `\int\limits` in text style, in a formula KaTeX gives these numbers for.
        let (_, h, d) = sized("\\int\\limits_2^2 3", false);
        assert!(close(h, 1.556668) && close(d, 1.022668), "{h} {d}");
        // A sum in display style, with the oracle's fraction after it.
        let (_, h, d) = sized("\\sum_{i=0}^\\infty \\frac{1}{i}", true);
        assert!(close(h, 1.651397) && close(d, 1.277669), "{h} {d}");
        // `\underset` keeps its arrow's box off the axis and clamps its depth at zero.
        let (_, h, d) = sized("\\underset{f}{\\rightarrow} Y", false);
        assert!(close(h, 0.68333) && close(d, 0.888216), "{h} {d}");
    }

    #[test]
    fn a_sum_with_limits_in_display_style() {
        // The base is centered on the axis (shift = (h - d) / 2 - 0.25); the superscript sits
        // `max(bigOpSpacing1, bigOpSpacing3 - depth)` above and the subscript `max(bigOpSpacing2,
        // bigOpSpacing4 - height)` below, with `bigOpSpacing5` beyond each, all in display style.
        let (bw, bh, bd, italic) = glyph('\u{2211}', Face::Size2Regular);
        let shift = (bh - bd) / 2.0 - 0.25;
        let (aw, ah, ..) = glyph('a', Face::MathItalic);
        let (bbw, bbh, ..) = glyph('b', Face::MathItalic);
        let (sub_h, sup_h) = (0.7 * ah, 0.7 * bbh);
        let sup_kern = 0.111_f64.max(0.2);
        let sub_kern = 0.166_f64.max(0.6 - sub_h);
        let (w, h, d) = sized("\\sum_a^b", true);
        assert!(close(h, bh - shift + sup_kern + sup_h + 0.1), "{h}");
        assert!(close(d, bd + shift + sub_kern + sub_h + 0.1), "{d}");
        // Single-character limits are centered on the symbol, shifted by its slant.
        assert!(
            close(w, bw.max(0.7 * aw - italic).max(0.7 * bbw + italic)),
            "{w}"
        );
    }

    #[test]
    fn named_limits_in_display_style() {
        // `\lim_x`: the base is the name's height; the subscript hangs below it by
        // `max(bigOpSpacing2, bigOpSpacing4 - height)`, with `bigOpSpacing5` above it.
        let ((w, h, d), glyphs) = laid_out("\\lim_x", true);
        let name: f64 = ['l', 'i', 'm']
            .iter()
            .map(|&c| glyph(c, Face::MainRegular).0)
            .sum();
        let x_h = 0.7 * glyph('x', Face::MathItalic).1;
        assert!(close(w, name), "{w}");
        assert!(close(h, 0.69444), "{h}");
        assert!(close(d, 0.1 + x_h + 0.166_f64.max(0.6 - x_h)), "{d}");
        // The stack lists the subscript first, as it is built bottom to top.
        assert_eq!(glyphs, ["x", "l", "i", "m"]);
        // A `\mathop` body takes limits with `\limits`; KaTeX's oracle has depth 0.7 for the
        // same stack with a wider subscript (`bigOpSpacing4` less the subscript's height).
        let (_, h, d) = sized("\\mathop{\\mathrm{lim}}\\limits_x", false);
        assert!(
            close(h, 0.69444) && close(d, 0.1 + x_h + 0.166_f64.max(0.6 - x_h)),
            "{h} {d}"
        );
        // In text style the limits are ordinary scripts.
        let (_, h, d) = sized("\\lim_x", false);
        assert!(close(h, 0.69444) && close(d, 0.15), "{h} {d}");
    }

    #[test]
    fn a_lower_limit_leaves_room_for_the_slant() {
        // A subscript wider than one character moves left by the slant, and a space of the slant
        // keeps it clear of what comes before.
        let (_, _, _, slant) = glyph('\u{222b}', Face::Size1Regular);
        assert!(slant > 0.0);
        let (iw, ..) = glyph('\u{222b}', Face::Size1Regular);
        let sub: f64 = ['a', 'b']
            .iter()
            .map(|&c| glyph(c, Face::MathItalic).0)
            .sum();
        let (w, ..) = sized("\\int\\limits_{ab}", false);
        assert!(close(w, slant + iw.max(0.7 * sub - slant)), "{w}");
        // One character does not.
        let (w, ..) = sized("\\int\\limits_a", false);
        let a = glyph('a', Face::MathItalic).0;
        assert!(close(w, iw.max(0.7 * a - slant)), "{w}");
    }

    #[test]
    fn operator_names_are_upright_and_take_limits_when_starred() {
        let (_, h, d) = sized("\\operatorname{f}", false);
        let (_, fh, fd, _) = glyph('f', Face::MainRegular);
        assert!(close(h, fh) && close(d, fd), "{h} {d}");
        // Scripts on the plain form are ordinary ones, on the starred form limits in display
        // style only.
        let (_, h, d) = sized("\\operatorname*{asin}_y x", false);
        let (_, _, plain_d) = sized("\\operatorname{asin}_y x", false);
        assert!(close(h, 0.66786) && close(d, plain_d), "{h} {d}");
        let (_, _, limits_d) = sized("\\operatorname*{asin}_y", true);
        let (_, _, scripted_d) = sized("\\operatorname{asin}_y", true);
        assert!(limits_d > scripted_d, "{limits_d} {scripted_d}");
        // `\limits` asks for them in any style.
        let (_, h, d) = sized("\\operatorname{sn}\\limits_{b>c}(b+c)", false);
        assert!(close(h, 0.75) && close(d, 0.25), "{h} {d}");
    }

    #[test]
    fn operator_names_use_hyphen_and_asterisk() {
        // The glyphs are the hyphen and asterisk; their widths are the minus sign's and
        // asterisk operator's.
        let ((w, ..), glyphs) = laid_out("\\operatorname{a-b*c}", false);
        assert_eq!(glyphs, ["a", "-", "b", "*", "c"]);
        let main = |c: char| glyph(c, Face::MainRegular).0;
        let minus = glyph('\u{2212}', Face::MainRegular).0;
        let ast = glyph('\u{2217}', Face::MainRegular).0;
        assert!(
            close(w, main('a') + minus + main('b') + ast + main('c')),
            "{w}"
        );
    }

    type Oval = ((f64, f64, f64, f64), [f64; 4], String);

    /// The path items of a formula: their rectangle (x, y, width, height), view box, and data.
    fn ovals(src: &str, display: bool) -> Vec<Oval> {
        let nodes = parse_with(
            src,
            Settings {
                display_mode: display,
            },
        )
        .unwrap();
        layout(&nodes, display)
            .unwrap()
            .items()
            .into_iter()
            .filter_map(|item| match item {
                Item::Path {
                    x,
                    y,
                    width,
                    height,
                    path,
                    ..
                } => Some(((x, y, width, height), path.view_box, path.data.into_owned())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn oiint_overlays_an_oval_on_the_double_integral() {
        // Text style: the small oval at the glyph's baseline, over the Size1 glyph of `\iint`.
        let (w, h, d) = sized("\\oiint", false);
        let (gw, gh, gd, _) = glyph('\u{222c}', Face::Size1Regular);
        assert!(
            close(w, gw.max(0.957)) && close(h, gh.max(0.499)) && close(d, gd),
            "{w} {h} {d}"
        );
        let small = ovals("\\oiint", false);
        let [((x, y, width, height), view_box, data)] = small.as_slice() else {
            panic!("one oval: {small:?}");
        };
        assert!(close(*x, 0.0) && close(*y, -0.499), "{x} {y}");
        assert!(close(*width, 0.957) && close(*height, 0.499));
        assert_eq!(*view_box, [0.0, 0.0, 957.0, 499.0]);
        assert_eq!(data, crate::math::svg_data::path("oiintSize1").unwrap());

        // Display style: the large oval, 0.08em lower, over the Size2 glyph.
        let (w, h, d) = sized("\\oiint", true);
        let (gw, gh, gd, _) = glyph('\u{222c}', Face::Size2Regular);
        assert!(
            close(w, gw.max(1.472)) && close(h, gh.max(0.659 - 0.08)) && close(d, gd.max(0.08)),
            "{w} {h} {d}"
        );
        let large = ovals("\\oiint", true);
        let [((_, y, width, height), view_box, data)] = large.as_slice() else {
            panic!("one oval: {large:?}");
        };
        assert!(close(*y, 0.08 - 0.659), "{y}");
        assert!(close(*width, 1.472) && close(*height, 0.659));
        assert_eq!(*view_box, [0.0, 0.0, 1472.0, 659.0]);
        assert_eq!(data, crate::math::svg_data::path("oiintSize2").unwrap());
    }

    #[test]
    fn oiiint_overlays_the_wider_oval_on_the_triple_integral() {
        let small = ovals("\\oiiint", false);
        let [((_, _, width, height), view_box, data)] = small.as_slice() else {
            panic!("one oval: {small:?}");
        };
        assert!(close(*width, 1.304) && close(*height, 0.499));
        assert_eq!(*view_box, [0.0, 0.0, 1304.0, 499.0]);
        assert_eq!(data, crate::math::svg_data::path("oiiintSize1").unwrap());
        let large = ovals("\\oiiint", true);
        let [((_, _, width, height), _, data)] = large.as_slice() else {
            panic!("one oval: {large:?}");
        };
        assert!(close(*width, 1.98) && close(*height, 0.659));
        assert_eq!(data, crate::math::svg_data::path("oiiintSize2").unwrap());
    }

    #[test]
    fn oiint_subscripts_tuck_under_the_slant_as_iint_subscripts_do() {
        // Both subscripts start at the base's right edge less the integral's slant; the oval
        // makes `\oiint`'s base wider than `\iint`'s.
        for (display, face) in [(false, Face::Size1Regular), (true, Face::Size2Regular)] {
            let italic = glyph('\u{222c}', face).3;
            assert!(italic > 0.0);
            for name in ["\\iint", "\\oiint"] {
                let base = sized(name, display).0;
                let (glyphs, _) = placed(&format!("{name}\\nolimits_a"), display);
                let a = glyphs
                    .iter()
                    .find(|(t, _)| t == "a")
                    .map(|&(_, x)| x)
                    .unwrap();
                assert!(close(a, base - italic), "{name}: {a} vs {base} - {italic}");
            }
        }
    }

    #[test]
    fn oiint_with_scripts_matches_katex() {
        // KaTeX's heights and depths for the contour integrals with scripts, from the oracle.
        for src in ["\\oiint_i^n", "\\oiiint_i^n", "\\oiint\\nolimits_i^n"] {
            let (_, h, d) = sized(src, false);
            assert!(close(h, 0.859292) && close(d, 0.3557), "{src}: {h} {d}");
        }
    }
}
