//! Delimiters: the null delimiter, `\big` and its sizes, `\left`...`\right` with `\middle`, and a
//! fraction's delimiters.
//!
//! Adapted from KaTeX's `src/delimiter.ts` (`makeSmallDelim`, `makeLargeDelim`,
//! `makeStackedDelim`, `sizedDelim`, `customSizedDelim`, `leftRightDelim`),
//! `src/functions/delimsizing.ts`, and the delimiter part of `src/functions/genfrac.ts` (MIT,
//! Copyright (c) 2013-2020 Khan Academy and other contributors).
//!
//! A delimiter is one of three kinds, as in KaTeX: a glyph of the main font in a smaller style, a
//! glyph of one of the four size fonts, or a stack of pieces for the delimiters that grow without
//! bound. A stack's straight part is one SVG path box of any height, so a tall delimiter costs
//! the same as a short one.

use std::borrow::Cow;
use std::ops::Range;

use super::{
    Atom, Built, Class, build, cancel_binary_operators, insert_spaces,
    symbol::{drawn_text, source_text},
};
use crate::math::{
    MathError,
    ast::{MathClass, Node, NodeKind},
    boxes::{Bx, Fit, Part, Position},
    metrics::{self, CharMetrics},
    metrics_data::Face,
    options::{Em, Options},
    style::Style,
    svg_paths,
    symbols::{Mode, Name},
};

/// The most a `\left` ... `\right` delimiter stretches, in em (LMATH-6).
const MAX_STRETCH_EM: f64 = 16.0;

/// How far the pieces of a stacked delimiter overlap, in em.
const LAP: f64 = 0.008;

/// The height plus depth of each size font's delimiter, found by looking at TeX's output of
/// `$\bigl| \Bigl| \biggl| \Biggl| \showlists$`.
const SIZE_TO_MAX_HEIGHT: [f64; 4] = [1.2, 1.8, 2.4, 3.0];

/// One step of the sequence a delimiter follows as it grows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    /// The main font's glyph, set in this style.
    Small(Style),
    /// The glyph of the size font 1 to 4.
    Large(u8),
    /// Pieces stacked to any height.
    Stack,
}

/// Delimiters that never stack try small delimiters and large delimiters only.
const NEVER_STACK: [Step; 7] = [
    Step::Small(Style::ScriptScript),
    Step::Small(Style::Script),
    Step::Small(Style::Text),
    Step::Large(1),
    Step::Large(2),
    Step::Large(3),
    Step::Large(4),
];

/// Delimiters that always stack try the small delimiters first, then stack.
const ALWAYS_STACK: [Step; 4] = [
    Step::Small(Style::ScriptScript),
    Step::Small(Style::Script),
    Step::Small(Style::Text),
    Step::Stack,
];

/// Delimiters that stack when large try the small and then the large delimiters, and stack
/// afterwards.
const STACK_WHEN_LARGE: [Step; 8] = [
    Step::Small(Style::ScriptScript),
    Step::Small(Style::Script),
    Step::Small(Style::Text),
    Step::Large(1),
    Step::Large(2),
    Step::Large(3),
    Step::Large(4),
    Step::Stack,
];

const STACK_LARGE: [&str; 21] = [
    "(", "\\lparen", ")", "\\rparen", "[", "\\lbrack", "]", "\\rbrack", "\\{", "\\lbrace", "\\}",
    "\\rbrace", "\\lfloor", "\\rfloor", "\u{230a}", "\u{230b}", "\\lceil", "\\rceil", "\u{2308}",
    "\u{2309}", "\\surd",
];

const STACK_ALWAYS: [&str; 22] = [
    "\\uparrow",
    "\\downarrow",
    "\\updownarrow",
    "\\Uparrow",
    "\\Downarrow",
    "\\Updownarrow",
    "|",
    "\\|",
    "\\vert",
    "\\Vert",
    "\\lvert",
    "\\rvert",
    "\\lVert",
    "\\rVert",
    "\\lgroup",
    "\\rgroup",
    "\u{27ee}",
    "\u{27ef}",
    "\\lmoustache",
    "\\rmoustache",
    "\u{23b0}",
    "\u{23b1}",
];

const STACK_NEVER: [&str; 8] = [
    "<",
    ">",
    "\\langle",
    "\\rangle",
    "/",
    "\\backslash",
    "\\lt",
    "\\gt",
];

/// What every delimiter function needs: the mode of the node, the options it is built in, and
/// its place in the source for errors.
#[derive(Clone, Copy)]
struct Ctx<'a, 't> {
    mode: Mode,
    options: &'a Options<'t>,
    span: &'a Range<usize>,
}

/// The empty space where a delimiter is left out: `\nulldelimiterspace`, 1.2pt of the base
/// size (KaTeX's `makeNullDelimiter`).
pub(super) fn null_delimiter<'t>(options: &Options<'t>) -> Bx<'t> {
    Bx::kern(Em(0.12 / options.size_multiplier()))
}

/// The delimiter on one side of a fraction (`\binom`, `\genfrac`): `None` is a null delimiter.
/// It is sized by the style alone (rule 15e): `delim1` in display style, `delim2` otherwise.
pub(super) fn frac_delimiter<'t>(
    delim: Option<&'t str>,
    mode: Mode,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Bx<'t>, MathError> {
    let delim = match delim {
        None | Some(".") => return Ok(null_delimiter(options)),
        Some(delim) => delim,
    };
    let size = match options.style.size() {
        0 => options.metrics().delim1,
        3 => options.having_style(Style::Script).metrics().delim2,
        _ => options.metrics().delim2,
    };
    let ctx = Ctx {
        mode,
        options,
        span: &span,
    };
    custom_sized_delim(delim, size, true, ctx)
}

/// `\big(` and the other fixed sizes: `size` is 1 to 4, spaced as `class`.
pub(super) fn delim_sizing<'t>(
    size: u8,
    class: MathClass,
    delim: &'t str,
    mode: Mode,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let class = Some(Class::from(class));
    if delim == "." {
        // An empty delimiter still counts as an element, though it shows nothing.
        return Ok(Built::One(Atom::new(Bx::kern(Em::ZERO), class, options)));
    }
    let ctx = Ctx {
        mode,
        options,
        span: &span,
    };
    let bx = sized_delim(delim, size, ctx)?;
    Ok(Built::One(Atom::new(bx, class, options)))
}

/// `\left ... \right` around `body`, with any `\middle` delimiters in it grown to match.
pub(super) fn left_right<'t>(
    body: &'t [Node<'_>],
    (left, right): (&'t str, &'t str),
    right_color: Option<&'t str>,
    mode: Mode,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let ctx = Ctx {
        mode,
        options,
        span: &span,
    };
    // The body as `expression` builds it, but keeping where the `\middle` delimiters are: they
    // are built small and replaced once the body's size is known.
    let mut atoms = Vec::new();
    let mut middles: Vec<Option<&'t str>> = Vec::new();
    for node in body {
        match build(node, options)? {
            Built::One(atom) => {
                middles.push(match &node.kind {
                    NodeKind::Middle { delim } if delim != "." => Some(delim.as_str()),
                    _ => None,
                });
                atoms.push(atom);
            }
            Built::Many(more) => {
                middles.extend(more.iter().map(|_| None));
                atoms.extend(more);
            }
        }
    }

    let mut inner_height = Em::ZERO;
    let mut inner_depth = Em::ZERO;
    for (atom, middle) in atoms.iter().zip(&middles) {
        if middle.is_none() {
            inner_height = inner_height.max(atom.bx.height);
            inner_depth = inner_depth.max(atom.bx.depth);
        }
    }
    // The size of delimiters is the same whatever the style, so the body's size is scaled down
    // to the base size.
    let scale = options.size_multiplier();
    let (inner_height, inner_depth) = (inner_height.0 * scale, inner_depth.0 * scale);

    let left = if left == "." {
        null_delimiter(options)
    } else {
        left_right_delim(left, inner_height, inner_depth, ctx)?
    };
    for (atom, middle) in atoms.iter_mut().zip(&middles) {
        if let Some(delim) = middle {
            atom.bx = left_right_delim(delim, inner_height, inner_depth, ctx)?;
        }
    }
    let right = if right == "." {
        null_delimiter(options)
    } else {
        let colored = match right_color {
            Some(color) => options.with_color(color),
            None => *options,
        };
        let ctx = Ctx {
            options: &colored,
            ..ctx
        };
        left_right_delim(right, inner_height, inner_depth, ctx)?
    };

    // Spaces as the list rules of a group give them.
    let glue_options = match body {
        [
            Node {
                kind: NodeKind::Sizing { size, .. },
                ..
            },
        ] => options.having_size(*size),
        [
            Node {
                kind: NodeKind::Styling { style, .. },
                ..
            },
        ] => options.having_style((*style).into()),
        _ => *options,
    };
    cancel_binary_operators(&mut atoms, false);
    let atoms = insert_spaces(atoms, &glue_options, false);

    let mut boxes = Vec::with_capacity(atoms.len() + 2);
    boxes.push(left);
    boxes.extend(atoms.into_iter().map(|atom| atom.bx));
    boxes.push(right);
    Ok(Built::One(Atom::new(
        Bx::hlist(boxes),
        Some(Class::Inner),
        options,
    )))
}

/// `\middle|` on its own; inside `\left ... \right`, `left_right` sizes it.
pub(super) fn middle<'t>(
    delim: &'t str,
    mode: Mode,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let bx = if delim == "." {
        null_delimiter(options)
    } else {
        let ctx = Ctx {
            mode,
            options,
            span: &span,
        };
        sized_delim(delim, 1, ctx)?
    };
    Ok(Built::One(Atom::new(bx, None, options)))
}

/// Which glyph the radical sign is drawn from at a height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Surd {
    /// The main font's, in the style that fits.
    Small,
    /// The size font 1 to 4's.
    Large(u8),
    /// Drawn tall as one path.
    Tall,
}

/// The radical sign that is first to hold `height` em (KaTeX's `traverseSequence` over the
/// sequence of `\surd`), set in `options`.
pub(super) fn surd_size(
    height: f64,
    options: &Options<'_>,
    span: &Range<usize>,
) -> Result<Surd, MathError> {
    let ctx = Ctx {
        mode: Mode::Math,
        options,
        span,
    };
    Ok(
        match traverse_sequence("\\surd", height, &STACK_WHEN_LARGE, ctx)? {
            Step::Small(_) => Surd::Small,
            Step::Large(size) => Surd::Large(size),
            Step::Stack => Surd::Tall,
        },
    )
}

/// The text a delimiter draws, as the symbol table replaces it (KaTeX's `symbols[mode][delim]
/// .replace`).
fn drawn<'t>(delim: &'t str, mode: Mode) -> Cow<'t, str> {
    let name = match delim.strip_prefix('\\') {
        Some(word) => Name::Control(word),
        None => Name::Char(delim),
    };
    drawn_text(Some(name), &source_text(Name::Char(delim)), mode)
}

fn size_face(size: u8) -> Face {
    match size {
        1 => Face::Size1Regular,
        2 => Face::Size2Regular,
        3 => Face::Size3Regular,
        _ => Face::Size4Regular,
    }
}

/// The metrics of a delimiter's symbol in a face (KaTeX's `getMetrics`).
fn symbol_metrics(delim: &str, face: Face, ctx: Ctx<'_, '_>) -> Result<CharMetrics, MathError> {
    let text = drawn(delim, Mode::Math);
    text.chars()
        .next()
        .and_then(|c| metrics::glyph(c, face, Mode::Math))
        .ok_or_else(|| {
            MathError::new(
                format!("Unsupported symbol {delim} and font size {face:?}."),
                ctx.span.clone(),
            )
        })
}

/// A delimiter span in a given style: its size follows the style's, in the em of `options`
/// (KaTeX's `styleWrap`).
fn style_wrap<'t>(delim: Bx<'t>, to_style: Style, options: &Options<'t>) -> Bx<'t> {
    let factor = options.having_base_style(to_style).size_multiplier() / options.size_multiplier();
    Bx::scaled(factor, Bx::hlist(vec![delim]))
}

/// A span centered on the axis of `options` though it is set in `style` (KaTeX's `centerSpan`).
/// KaTeX moves the delimiter by `shift` of its own em but counts `shift` of the outer em in its
/// height and depth; both are kept.
fn center_span<'t>(span: Bx<'t>, options: &Options<'t>, style: Style) -> Bx<'t> {
    let ratio = options.size_multiplier() / options.having_base_style(style).size_multiplier();
    let shift = (1.0 - ratio) * options.metrics().axis_height;
    if shift == 0.0 {
        return span;
    }
    let (height, depth) = (span.height - Em(shift), span.depth + Em(shift));
    let mut centered = Bx::vlist_shifted(vec![(span, Em(shift / ratio), Em::ZERO)]);
    centered.height = height;
    centered.depth = depth;
    centered
}

/// A delimiter of the main font, set in a smaller style (KaTeX's `makeSmallDelim`).
fn small_delim<'t>(delim: &'t str, style: Style, center: bool, ctx: Ctx<'_, 't>) -> Bx<'t> {
    let glyph = Bx::glyph(
        drawn(delim, ctx.mode),
        Face::MainRegular,
        ctx.mode,
        ctx.options,
    );
    let span = style_wrap(glyph, style, ctx.options);
    if center {
        center_span(span, ctx.options, style)
    } else {
        span
    }
}

/// A delimiter of a size font, always in text style (KaTeX's `makeLargeDelim`).
fn large_delim<'t>(delim: &'t str, size: u8, center: bool, ctx: Ctx<'_, 't>) -> Bx<'t> {
    let glyph = Bx::glyph(
        drawn(delim, ctx.mode),
        size_face(size),
        ctx.mode,
        ctx.options,
    );
    let span = style_wrap(Bx::hlist(vec![glyph]), Style::Text, ctx.options);
    if center {
        center_span(span, ctx.options, Style::Text)
    } else {
        span
    }
}

/// The pieces a stacked delimiter is made of.
struct Pieces<'t> {
    top: &'t str,
    middle: Option<&'t str>,
    repeat: &'t str,
    bottom: &'t str,
    face: Face,
    /// The name and view box width of the path that draws the whole delimiter, for those that
    /// have one.
    path: Option<(&'static str, f64)>,
}

fn pieces(delim: &str) -> Pieces<'_> {
    let mut p = Pieces {
        top: delim,
        middle: None,
        repeat: delim,
        bottom: delim,
        face: Face::Size1Regular,
        path: None,
    };
    // `\u{23d0}` and `\u{2016}` stand in for `|` and `\|` in the repeats of the arrows.
    match delim {
        "\\uparrow" => (p.repeat, p.bottom) = ("\u{23d0}", "\u{23d0}"),
        "\\Uparrow" => (p.repeat, p.bottom) = ("\u{2016}", "\u{2016}"),
        "\\downarrow" => (p.top, p.repeat) = ("\u{23d0}", "\u{23d0}"),
        "\\Downarrow" => (p.top, p.repeat) = ("\u{2016}", "\u{2016}"),
        "\\updownarrow" => (p.top, p.repeat, p.bottom) = ("\\uparrow", "\u{23d0}", "\\downarrow"),
        "\\Updownarrow" => (p.top, p.repeat, p.bottom) = ("\\Uparrow", "\u{2016}", "\\Downarrow"),
        "|" | "\\lvert" | "\\rvert" | "\\vert" => {
            p.repeat = "\u{2223}";
            p.path = Some(("vert", 333.0));
        }
        "\\|" | "\\lVert" | "\\rVert" | "\\Vert" => {
            p.repeat = "\u{2225}";
            p.path = Some(("doublevert", 556.0));
        }
        "[" | "\\lbrack" => {
            (p.top, p.repeat, p.bottom) = ("\u{23a1}", "\u{23a2}", "\u{23a3}");
            (p.face, p.path) = (Face::Size4Regular, Some(("lbrack", 667.0)));
        }
        "]" | "\\rbrack" => {
            (p.top, p.repeat, p.bottom) = ("\u{23a4}", "\u{23a5}", "\u{23a6}");
            (p.face, p.path) = (Face::Size4Regular, Some(("rbrack", 667.0)));
        }
        "\\lfloor" | "\u{230a}" => {
            (p.top, p.repeat, p.bottom) = ("\u{23a2}", "\u{23a2}", "\u{23a3}");
            (p.face, p.path) = (Face::Size4Regular, Some(("lfloor", 667.0)));
        }
        "\\lceil" | "\u{2308}" => {
            (p.top, p.repeat, p.bottom) = ("\u{23a1}", "\u{23a2}", "\u{23a2}");
            (p.face, p.path) = (Face::Size4Regular, Some(("lceil", 667.0)));
        }
        "\\rfloor" | "\u{230b}" => {
            (p.top, p.repeat, p.bottom) = ("\u{23a5}", "\u{23a5}", "\u{23a6}");
            (p.face, p.path) = (Face::Size4Regular, Some(("rfloor", 667.0)));
        }
        "\\rceil" | "\u{2309}" => {
            (p.top, p.repeat, p.bottom) = ("\u{23a4}", "\u{23a5}", "\u{23a5}");
            (p.face, p.path) = (Face::Size4Regular, Some(("rceil", 667.0)));
        }
        "(" | "\\lparen" => {
            (p.top, p.repeat, p.bottom) = ("\u{239b}", "\u{239c}", "\u{239d}");
            (p.face, p.path) = (Face::Size4Regular, Some(("lparen", 875.0)));
        }
        ")" | "\\rparen" => {
            (p.top, p.repeat, p.bottom) = ("\u{239e}", "\u{239f}", "\u{23a0}");
            (p.face, p.path) = (Face::Size4Regular, Some(("rparen", 875.0)));
        }
        "\\{" | "\\lbrace" => {
            (p.top, p.middle, p.bottom) = ("\u{23a7}", Some("\u{23a8}"), "\u{23a9}");
            (p.repeat, p.face) = ("\u{23aa}", Face::Size4Regular);
        }
        "\\}" | "\\rbrace" => {
            (p.top, p.middle, p.bottom) = ("\u{23ab}", Some("\u{23ac}"), "\u{23ad}");
            (p.repeat, p.face) = ("\u{23aa}", Face::Size4Regular);
        }
        "\\lgroup" | "\u{27ee}" => {
            (p.top, p.bottom, p.repeat) = ("\u{23a7}", "\u{23a9}", "\u{23aa}");
            p.face = Face::Size4Regular;
        }
        "\\rgroup" | "\u{27ef}" => {
            (p.top, p.bottom, p.repeat) = ("\u{23ab}", "\u{23ad}", "\u{23aa}");
            p.face = Face::Size4Regular;
        }
        "\\lmoustache" | "\u{23b0}" => {
            (p.top, p.bottom, p.repeat) = ("\u{23a7}", "\u{23ad}", "\u{23aa}");
            p.face = Face::Size4Regular;
        }
        "\\rmoustache" | "\u{23b1}" => {
            (p.top, p.bottom, p.repeat) = ("\u{23ab}", "\u{23a9}", "\u{23aa}");
            p.face = Face::Size4Regular;
        }
        _ => {}
    }
    p
}

/// A glyph piece of a stacked delimiter.
fn glyph_piece<'t>(symbol: &'t str, face: Face, ctx: Ctx<'_, 't>) -> Bx<'t> {
    let glyph = Bx::glyph(drawn(symbol, ctx.mode), face, ctx.mode, ctx.options);
    Bx::hlist(vec![Bx::hlist(vec![glyph])])
}

/// The straight part of a stacked delimiter, as an SVG path `height` em tall (KaTeX's
/// `makeInner`).
fn inner_piece<'t>(symbol: &str, height: f64, ctx: Ctx<'_, 't>) -> Result<Bx<'t>, MathError> {
    let width = symbol
        .chars()
        .next()
        .and_then(|c| {
            metrics::glyph(c, Face::Size4Regular, Mode::Math)
                .or_else(|| metrics::glyph(c, Face::Size1Regular, Mode::Math))
        })
        .map(|m| m.width)
        .ok_or_else(|| {
            MathError::new(
                format!("Unsupported symbol {symbol} and font size Size4Regular."),
                ctx.span.clone(),
            )
        })?;
    let view_height = (1000.0 * height).round();
    Ok(Bx::path(
        svg_paths::inner_path(symbol, view_height),
        [0.0, 0.0, 1000.0 * width, view_height],
        Fit::Stretch,
        (Em(width), Em(height), Em::ZERO),
        ctx.options,
    ))
}

/// A delimiter stacked from pieces to at least `height_total` em (KaTeX's `makeStackedDelim`):
/// the top, the bottom, an optional middle, and between them a path as tall as needed.
fn stacked_delim<'t>(
    delim: &'t str,
    height_total: f64,
    center: bool,
    ctx: Ctx<'_, 't>,
) -> Result<Bx<'t>, MathError> {
    let p = pieces(delim);
    let total = |symbol: &str| -> Result<f64, MathError> {
        let m = symbol_metrics(symbol, p.face, ctx)?;
        Ok(m.height + m.depth)
    };
    let top_total = total(p.top)?;
    let repeat_total = total(p.repeat)?;
    let bottom_total = total(p.bottom)?;
    let (middle_total, middle_factor) = match p.middle {
        Some(middle) => (total(middle)?, 2.0),
        None => (0.0, 1.0),
    };

    // The least the delimiter can be is its top, bottom, and middle.
    let min_height = top_total + bottom_total + middle_total;
    let repeat_count = if repeat_total > 0.0 {
        ((height_total - min_height) / (middle_factor * repeat_total))
            .ceil()
            .max(0.0)
    } else {
        0.0
    };
    let real_total = min_height + repeat_count * middle_factor * repeat_total;
    if !real_total.is_finite() {
        return Err(MathError::new("A delimiter is too tall", ctx.span.clone()));
    }

    // The delimiter is centered on the axis: of the style it is built in, if `center`.
    let mut axis = ctx.options.metrics().axis_height;
    if center {
        axis *= ctx.options.size_multiplier();
    }
    let depth = real_total / 2.0 - axis;

    let lap = Part::Kern(Em(-LAP));
    let mut stack: Vec<Part<'t>> = Vec::new();
    if let Some((label, view_width)) = p.path {
        // One SVG instead of stacked glyphs.
        let mid_height = real_total - top_total - bottom_total;
        let view_height = (real_total * 1000.0).round();
        let data = svg_paths::tall_delim(label, (mid_height * 1000.0).round())
            .ok_or_else(|| MathError::new("Unknown stretchy delimiter.", ctx.span.clone()))?;
        stack.push(Part::of(Bx::path(
            data,
            [0.0, 0.0, view_width, view_height],
            Fit::Stretch,
            (Em(view_width / 1000.0), Em(view_height / 1000.0), Em::ZERO),
            ctx.options,
        )));
    } else {
        // Bottom to top, each piece overlapping the next.
        stack.push(Part::of(glyph_piece(p.bottom, p.face, ctx)));
        stack.push(lap.clone());
        match p.middle {
            None => {
                let inner = real_total - top_total - bottom_total + 2.0 * LAP;
                stack.push(Part::of(inner_piece(p.repeat, inner, ctx)?));
            }
            Some(middle) => {
                let inner =
                    (real_total - top_total - bottom_total - middle_total) / 2.0 + 2.0 * LAP;
                stack.push(Part::of(inner_piece(p.repeat, inner, ctx)?));
                stack.push(lap.clone());
                stack.push(Part::of(glyph_piece(middle, p.face, ctx)));
                stack.push(lap.clone());
                stack.push(Part::of(inner_piece(p.repeat, inner, ctx)?));
            }
        }
        stack.push(lap);
        stack.push(Part::of(glyph_piece(p.top, p.face, ctx)));
    }
    let list = Bx::vlist(Position::Bottom(Em(depth)), stack);
    Ok(style_wrap(list, Style::Text, ctx.options))
}

/// `<` and `>` and their Unicode forms are `\langle` and `\rangle` as delimiters.
fn angle(delim: &str) -> &str {
    match delim {
        "<" | "\\lt" | "\u{27e8}" => "\\langle",
        ">" | "\\gt" | "\u{27e9}" => "\\rangle",
        _ => delim,
    }
}

/// A delimiter of one of the four sizes, as `\big` to `\Bigg` make it (KaTeX's
/// `makeSizedDelim`). Sized delimiters are never centered.
fn sized_delim<'t>(delim: &'t str, size: u8, ctx: Ctx<'_, 't>) -> Result<Bx<'t>, MathError> {
    let delim = angle(delim);
    let Some(max_height) = usize::from(size)
        .checked_sub(1)
        .and_then(|at| SIZE_TO_MAX_HEIGHT.get(at))
    else {
        return Err(MathError::new(
            format!("Illegal delimiter size: {size}"),
            ctx.span.clone(),
        ));
    };
    if STACK_LARGE.contains(&delim) || STACK_NEVER.contains(&delim) {
        Ok(large_delim(delim, size, false, ctx))
    } else if STACK_ALWAYS.contains(&delim) {
        stacked_delim(delim, *max_height, false, ctx)
    } else {
        Err(MathError::new(
            format!("Illegal delimiter: '{delim}'"),
            ctx.span.clone(),
        ))
    }
}

/// The first step of a sequence that holds `height` em of height plus depth (KaTeX's
/// `traverseSequence`).
fn traverse_sequence(
    delim: &str,
    height: f64,
    sequence: &[Step],
    ctx: Ctx<'_, '_>,
) -> Result<Step, MathError> {
    // Smaller styles start earlier in the sequence: scriptscript at 0, script at 1, text and
    // display at 2.
    let start = usize::from(3 - ctx.options.style.size()).min(2);
    for &step in sequence.iter().skip(start) {
        let face = match step {
            // Always the last step.
            Step::Stack => break,
            Step::Small(_) => Face::MainRegular,
            Step::Large(size) => size_face(size),
        };
        let m = symbol_metrics(delim, face, ctx)?;
        let mut height_depth = m.height + m.depth;
        // Small delimiters are scaled down versions of the same font.
        if let Step::Small(style) = step {
            height_depth *= ctx.options.having_base_style(style).size_multiplier();
        }
        if height_depth > height {
            return Ok(step);
        }
    }
    Ok(sequence.last().copied().unwrap_or(Step::Stack))
}

/// A delimiter of at least `height` em (height plus depth), optionally centered on the axis
/// (KaTeX's `makeCustomSizedDelim`).
fn custom_sized_delim<'t>(
    delim: &'t str,
    height: f64,
    center: bool,
    ctx: Ctx<'_, 't>,
) -> Result<Bx<'t>, MathError> {
    let delim = angle(delim);
    let sequence: &[Step] = if STACK_NEVER.contains(&delim) {
        &NEVER_STACK
    } else if STACK_LARGE.contains(&delim) {
        &STACK_WHEN_LARGE
    } else {
        &ALWAYS_STACK
    };
    Ok(match traverse_sequence(delim, height, sequence, ctx)? {
        Step::Small(style) => small_delim(delim, style, center, ctx),
        Step::Large(size) => large_delim(delim, size, center, ctx),
        Step::Stack => stacked_delim(delim, height, center, ctx)?,
    })
}

/// A delimiter for `\left` and `\right` around an expression `height` high and `depth` deep
/// (KaTeX's `makeLeftRightDelim`, by TeX's `make_left_right`).
fn left_right_delim<'t>(
    delim: &'t str,
    height: f64,
    depth: f64,
    ctx: Ctx<'_, 't>,
) -> Result<Bx<'t>, MathError> {
    // The delimiters are always centered, so the axis is always shifted.
    let axis = ctx.options.metrics().axis_height * ctx.options.size_multiplier();
    let delimiter_factor = 901.0;
    let delimiter_extend = 5.0 / ctx.options.metrics().pt_per_em;
    let max_dist_from_axis = (height - axis).max(depth + axis);
    // TeX does these sums in integers of 1/65536 pt, so it truncates where this does not.
    let total_height = (max_dist_from_axis / 500.0 * delimiter_factor)
        .max(2.0 * max_dist_from_axis - delimiter_extend)
        .min(MAX_STRETCH_EM);
    custom_sized_delim(delim, total_height, true, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{
        boxes::Item,
        layout::layout,
        parser::{Settings, parse_with},
    };

    /// Lays out `src` and gives `check` the box.
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

    /// The glyphs of a layout as (text, face, x, y, scale).
    fn glyphs(src: &str, display: bool) -> Vec<(String, Face, f64, f64, f64)> {
        lay(src, display, |bx| {
            bx.items()
                .into_iter()
                .filter_map(|item| match item {
                    Item::Glyph {
                        x, y, scale, glyph, ..
                    } => Some((glyph.text.to_string(), glyph.face, x, y, scale)),
                    _ => None,
                })
                .collect()
        })
    }

    /// The paths of a layout as (x, y, width, height), y from the top.
    fn paths(src: &str, display: bool) -> Vec<(f64, f64, f64, f64)> {
        lay(src, display, |bx| {
            bx.items()
                .into_iter()
                .filter_map(|item| match item {
                    Item::Path {
                        x,
                        y,
                        width,
                        height,
                        ..
                    } => Some((x, y, width, height)),
                    _ => None,
                })
                .collect()
        })
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    /// Equal to a thousandth of an em, which a path's view box rounds to.
    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-3
    }

    fn metrics_of(c: char, face: Face) -> CharMetrics {
        metrics::glyph(c, face, Mode::Math).unwrap()
    }

    #[test]
    fn big_delimiters_take_the_size_fonts() {
        // `\big(` to `\Bigg(` are the glyphs of Size1 to Size4, as high and deep as the font
        // says, and as wide.
        for (command, face) in [
            ("\\big(", Face::Size1Regular),
            ("\\Big(", Face::Size2Regular),
            ("\\bigg(", Face::Size3Regular),
            ("\\Bigg(", Face::Size4Regular),
        ] {
            let m = metrics_of('(', face);
            let (w, h, d) = size(command, false);
            assert!(close(w, m.width), "{command}: {w}");
            assert!(close(h, m.height), "{command}: {h}");
            assert!(close(d, m.depth), "{command}: {d}");
            let found = glyphs(command, false);
            assert_eq!(
                found
                    .iter()
                    .map(|g| (g.0.as_str(), g.1))
                    .collect::<Vec<_>>(),
                [("(", face)],
                "{command}"
            );
            assert_eq!((found[0].2, found[0].3, found[0].4), (0.0, 0.0, 1.0));
        }
        // The heights of the four sizes, as TeX sets them.
        let (_, h, d) = size("\\Bigg(", false);
        assert!(near(h + d, 3.0), "{h} {d}");
    }

    #[test]
    fn big_bars_stack_to_the_size() {
        // `|` always stacks: one path for the whole delimiter, never centered, so its height
        // less its depth is twice the axis height. Its extent is the size, rounded up to whole
        // repeats of the middle piece (KaTeX's oracle: `\bigg|` is 1.45 above and 0.95 below).
        for (command, total) in [
            ("\\big|", 1.2),
            ("\\Big|", 1.8),
            ("\\bigg|", 2.4),
            ("\\Bigg|", 3.0),
        ] {
            let (w, h, d) = size(command, false);
            assert!(near(h - d, 0.5), "{command}: {h} {d}");
            assert!(h + d >= total - 1e-9 && h + d < total + 0.6, "{command}");
            assert!(close(w, 0.333), "{command}: {w}");
        }
        let (_, h, d) = size("\\bigg|", false);
        assert!(
            (h - 1.45).abs() < 0.01 && (d - 0.95).abs() < 0.01,
            "{h} {d}"
        );
        // The delimiter is one path of the box's size, at the left edge.
        let found = paths("\\Bigg|", false);
        assert_eq!(found.len(), 1);
        let (x, _, width, height) = found[0];
        let (_, h, d) = size("\\Bigg|", false);
        assert!(close(x, 0.0) && close(width, 0.333) && near(height, h + d));
    }

    #[test]
    fn big_delimiters_are_spaced_by_their_class() {
        // `\bigl` opens and `\bigr` closes: nothing between them and a letter; `\bigm` is a
        // relation, so thick spaces go on both sides of it.
        let (w, ..) = size("x\\bigm|x", false);
        let (wx, ..) = size("x", false);
        let (wb, ..) = size("\\bigm|", false);
        assert!(close(w, 2.0 * wx + wb + 2.0 * 5.0 / 18.0), "{w}");
        let (w, ..) = size("x\\bigl(x", false);
        let (wb, ..) = size("\\bigl(", false);
        assert!(close(w, 2.0 * wx + wb), "{w}");
        // An empty delimiter takes no room.
        let (w, h, d) = size("\\big.", false);
        assert_eq!((w, h, d), (0.0, 0.0, 0.0));
    }

    #[test]
    fn left_right_around_a_fraction_takes_the_first_size_font() {
        // `\frac12` in text style is 0.845108 high and 0.345 deep. About the axis (0.25) that is
        // 0.595108 above and 0.595 below; TeX's rule gives max(0.595108 / 500 * 901, 2 * 0.595108
        // - 0.5) = 1.0724, which Size1's 1.2 is the first to hold.
        let (w, h, d) = size("\\left(\\frac12\\right)", false);
        let (frac_width, ..) = size("\\frac12", false);
        let paren = metrics_of('(', Face::Size1Regular);
        assert!(close(w, 2.0 * paren.width + frac_width), "{w}");
        assert!(close(h, paren.height) && close(d, paren.depth), "{h} {d}");
        assert!(near(h, 0.85) && near(d, 0.35), "{h} {d}");
        let found = glyphs("\\left(\\frac12\\right)", false);
        let texts: Vec<_> = found.iter().map(|g| (g.0.as_str(), g.1)).collect();
        assert_eq!(
            texts,
            [
                ("(", Face::Size1Regular),
                // The denominator is first in the stack, which lists bottom to top.
                ("2", Face::MainRegular),
                ("1", Face::MainRegular),
                (")", Face::Size1Regular)
            ]
        );
        // The left delimiter is at the left edge; the right one follows the fraction.
        assert!(close(found[0].2, 0.0));
        assert!(
            close(found[3].2, paren.width + frac_width),
            "{}",
            found[3].2
        );
    }

    #[test]
    fn small_bodies_keep_the_main_font_delimiter() {
        // `(x)` needs less than the main font's 1em: TeX's rule gives 0.5 above and below the
        // axis at most, and the plain parenthesis serves.
        let (w, h, d) = size("\\left(x\\right)", false);
        let paren = metrics_of('(', Face::MainRegular);
        let (wx, ..) = size("x", false);
        assert!(close(w, 2.0 * paren.width + wx), "{w}");
        assert!(close(h, 0.75) && close(d, 0.25), "{h} {d}");
        let found = glyphs("\\left(x\\right)", false);
        assert_eq!(found[0].1, Face::MainRegular);
        assert_eq!(found[2].1, Face::MainRegular);
    }

    #[test]
    fn left_right_is_an_inner_atom() {
        // Inner atoms take thin spaces next to an ordinary atom (3mu), which a bare
        // parenthesis group would not.
        let (w, ..) = size("x\\left(x\\right)x", false);
        let (wx, ..) = size("x", false);
        let (wp, ..) = size("\\left(x\\right)", false);
        assert!(close(w, 2.0 * wx + wp + 2.0 * 3.0 / 18.0), "{w}");
    }

    #[test]
    fn a_tall_body_stacks_the_delimiter() {
        // Four rows of a matrix are taller than Size4's 3em, so the parenthesis is one path
        // sized in whole repeats of its middle piece, centered on the axis.
        let body = "\\begin{matrix}a\\\\b\\\\c\\\\d\\end{matrix}";
        let (_, body_height, body_depth) = size(body, false);
        let max_dist = (body_height - 0.25).max(body_depth + 0.25);
        let wanted = (max_dist / 500.0 * 901.0).max(2.0 * max_dist - 0.5);
        assert!(wanted > 3.0, "{wanted}");
        let face = Face::Size4Regular;
        let total = |c: char| {
            let m = metrics_of(c, face);
            m.height + m.depth
        };
        let least = total('\u{239b}') + total('\u{239d}');
        let repeat = total('\u{239c}');
        let real = least + ((wanted - least) / repeat).ceil() * repeat;
        let src = format!("\\left({body}\\right)");
        let (w, h, d) = size(&src, false);
        assert!(
            near(h, real / 2.0 + 0.25) && near(d, real / 2.0 - 0.25),
            "{h} {d}"
        );
        // Each delimiter is one path, as wide as its pieces (0.875em).
        let found = paths(&src, false);
        assert_eq!(found.len(), 2);
        assert!(
            close(found[0].0, 0.0) && close(found[0].2, 0.875),
            "{found:?}"
        );
        assert!(
            near(found[0].3, real) && near(found[1].3, real),
            "{found:?}"
        );
        assert!(close(found[1].0, w - 0.875), "{found:?}");
        // The path is centered about the axis: it starts `real / 2 + 0.25` above the baseline.
        assert!(near(found[0].1, -(real / 2.0 + 0.25)), "{found:?}");
    }

    #[test]
    fn a_stacked_brace_is_pieces_and_paths() {
        // A brace stacks a top, a middle, a bottom, and a path above and below the middle. The
        // body is 3em high, which TeX's rule turns into a delimiter of 2 * (3 - 0.25) - 0.5 =
        // 5em at least, past Size4's 3em.
        let src = "\\left\\{\\rule{1em}{3em}\\right.";
        let found = glyphs(src, false);
        let texts: Vec<_> = found.iter().map(|g| g.0.as_str()).collect();
        assert_eq!(texts, ["\u{23a9}", "\u{23a8}", "\u{23a7}"]);
        assert!(
            found
                .iter()
                .all(|g| g.1 == Face::Size4Regular && g.2 == 0.0)
        );
        let inner = paths(src, false);
        assert_eq!(inner.len(), 2);
        let metrics = |c| metrics_of(c, Face::Size4Regular);
        let total = |c| metrics(c).height + metrics(c).depth;
        let (top, middle, bottom) = (total('\u{23a7}'), total('\u{23a8}'), total('\u{23a9}'));
        // Whole repeats of the path, two for each, to hold 5em.
        let least = top + middle + bottom;
        let repeat = total('\u{23aa}');
        let real = least + ((5.0 - least) / (2.0 * repeat)).ceil() * 2.0 * repeat;
        // The two paths are as tall as each other, and a lap longer at each end than their share.
        let share = (real - least) / 2.0 + 2.0 * LAP;
        assert!(
            near(inner[0].3, share) && near(inner[1].3, share),
            "{inner:?}"
        );
        // The bottom piece's baseline is its depth above the delimiter's bottom, which is
        // `real / 2 - 0.25` below the baseline.
        let at = real / 2.0 - 0.25 - metrics('\u{23a9}').depth;
        assert!(near(found[0].3, at), "{} {at}", found[0].3);
        // The pieces overlap by a lap: the next path starts a lap above the bottom piece's top.
        let bottom_top = found[0].3 - metrics('\u{23a9}').height;
        assert!(near(inner[0].1 + inner[0].3, bottom_top + LAP), "{inner:?}");
    }

    #[test]
    fn empty_delimiters_are_null_delimiter_spaces() {
        let (w, h, d) = size("\\left.x\\right.", false);
        let (wx, hx, dx) = size("x", false);
        assert!(close(w, wx + 2.0 * 0.12), "{w}");
        assert_eq!((h, d), (hx, dx));
        // In a smaller style the space is still 1.2pt of the base size.
        let (w, ..) = size("\\scriptstyle\\left.\\right.", false);
        assert!(close(w, 2.0 * 0.12), "{w}");
    }

    #[test]
    fn middle_delimiters_grow_with_the_body() {
        // Both `|` are the same height: the one in the middle is sized as the outer ones.
        let src = "\\left|\\frac12\\middle|\\frac12\\right|";
        let found = paths(src, false);
        assert_eq!(found.len(), 3, "{found:?}");
        assert!(found.iter().all(|p| close(p.3, found[0].3)), "{found:?}");
        // 1.0724 is more than the main font's `|` holds, so they stack to at least that.
        assert!(found[0].3 >= 1.0724 - 1e-9, "{found:?}");
        // Centered on the axis: `height / 2 + 0.25` above the baseline.
        assert!(near(found[1].1, -(found[1].3 / 2.0 + 0.25)), "{found:?}");
        // No spaces around a middle delimiter, which has no class.
        let (w, ..) = size(src, false);
        let (wf, ..) = size("\\frac12", false);
        assert!(close(w, 2.0 * wf + 3.0 * 0.333), "{w}");
    }

    #[test]
    fn middle_by_itself_is_a_big_delimiter() {
        let options = Options::new(false);
        let Ok(Built::One(atom)) = middle("|", Mode::Math, &options, 0..1) else {
            panic!("a delimiter")
        };
        // As `\big|` but with no class.
        assert_eq!(atom.class, None);
        let (_, h, d) = size("\\big|", false);
        assert!(close(atom.bx.height.0, h) && close(atom.bx.depth.0, d));
        let Ok(Built::One(null)) = middle(".", Mode::Math, &options, 0..1) else {
            panic!("a null delimiter")
        };
        assert_eq!(null.bx.width, Em(0.12));
    }

    #[test]
    fn fraction_delimiters_follow_the_style() {
        // delim2 is 1.01em: Size1 is the first to hold it. delim1 is 2.39em: Size3's 2.4em.
        let text = glyphs("\\binom12", false);
        assert_eq!(text[0].1, Face::Size1Regular);
        assert_eq!(text[text.len() - 1].1, Face::Size1Regular);
        let display = glyphs("\\binom12", true);
        assert_eq!(display[0].1, Face::Size3Regular);
        assert_eq!(display[display.len() - 1].1, Face::Size3Regular);
        // Display: the delimiter is 1.45 high and 0.95 deep, as Size3's glyph.
        let (_, h, d) = size("\\binom12", true);
        let m = metrics_of('(', Face::Size3Regular);
        assert!(close(h, m.height) && close(d, m.depth), "{h} {d}");
        // Text: the delimiter is 0.85 above and 0.35 below, less than the fraction's 0.895 above;
        // it is as wide as the fraction and two Size1 parentheses.
        let (w, h, d) = size("\\binom12", false);
        let (bare_width, bare_height, bare_depth) = size("\\genfrac{}{}{0pt}{}12", false);
        let paren = metrics_of('(', Face::Size1Regular);
        assert!(close(h, bare_height) && near(h, 0.895108), "{h}");
        assert!(near(d, 0.35) && d >= bare_depth, "{d}");
        assert!(close(w, bare_width - 2.0 * 0.12 + 2.0 * paren.width), "{w}");
        // `\genfrac` with one delimiter and one empty: only one side has a glyph.
        let one = glyphs("\\genfrac[.{0pt}{}{a}{b}", false);
        assert_eq!(one.iter().filter(|g| g.1 == Face::Size1Regular).count(), 1);
    }

    #[test]
    fn a_delimiter_of_another_style_is_centered_on_the_axis() {
        // In script style (0.7 of the base size) a size font glyph is 1/0.7 of the style's size,
        // so its center is 0.25 / 0.7 above the baseline; KaTeX moves it down by (1 - 0.7) *
        // 0.25 of its own em, which puts it on the axis, 0.25 in the style's em. Its height and
        // depth change by the same 0.075 in the style's em, as KaTeX counts them.
        let options = Options::new(false).having_style(Style::Script);
        let span = 0..1;
        let ctx = Ctx {
            mode: Mode::Math,
            options: &options,
            span: &span,
        };
        let bx = custom_sized_delim("(", 1.1, true, ctx).unwrap();
        let m = metrics_of('(', Face::Size1Regular);
        assert!(
            close(bx.height.0, m.height / 0.7 - 0.075),
            "{}",
            bx.height.0
        );
        assert!(close(bx.depth.0, m.depth / 0.7 + 0.075), "{}", bx.depth.0);
        let items = bx.items();
        let [Item::Glyph { y, scale, .. }] = items.as_slice() else {
            panic!("one glyph: {items:?}")
        };
        assert!(close(*scale, 1.0 / 0.7), "{scale}");
        assert!(close(*y, 0.075 / 0.7), "{y}");
        // The glyph's middle is on the axis.
        let middle = *y - (m.height - m.depth) / 2.0 / 0.7;
        assert!(near(middle, -0.25), "{middle}");
    }

    #[test]
    fn every_delimiter_lays_out() {
        let names = [
            "(",
            ")",
            "[",
            "]",
            "\\{",
            "\\}",
            "\\lparen",
            "\\rparen",
            "\\lbrack",
            "\\rbrack",
            "\\lbrace",
            "\\rbrace",
            "\\lfloor",
            "\\rfloor",
            "\\lceil",
            "\\rceil",
            "<",
            ">",
            "\\langle",
            "\\rangle",
            "\\lt",
            "\\gt",
            "\\lvert",
            "\\rvert",
            "\\lVert",
            "\\rVert",
            "\\lgroup",
            "\\rgroup",
            "\\lmoustache",
            "\\rmoustache",
            "/",
            "\\backslash",
            "|",
            "\\vert",
            "\\|",
            "\\Vert",
            "\\uparrow",
            "\\Uparrow",
            "\\downarrow",
            "\\Downarrow",
            "\\updownarrow",
            "\\Updownarrow",
            ".",
        ];
        for name in names {
            for body in ["x", "\\rule{1em}{4em}", "\\rule{1em}{300em}"] {
                let src = format!("\\left{name} {body}\\right{name}");
                let nodes = parse_with(
                    &src,
                    Settings {
                        display_mode: false,
                    },
                )
                .unwrap();
                let bx = layout(&nodes, false).unwrap_or_else(|e| panic!("{src}: {e}"));
                // The delimiter never stretches past the cap, however tall the body.
                let capped = bx.height.0 <= 300.0 && bx.depth.0 <= 300.0;
                assert!(capped, "{src}");
                assert!(bx.items().len() < 64, "{src}: {} items", bx.items().len());
            }
            if name != "." {
                for command in ["\\big", "\\Big", "\\bigg", "\\Bigg"] {
                    let src = format!("{command}{name} ");
                    let nodes = parse_with(
                        &src,
                        Settings {
                            display_mode: false,
                        },
                    )
                    .unwrap();
                    layout(&nodes, false).unwrap_or_else(|e| panic!("{src}: {e}"));
                }
            }
        }
    }

    #[test]
    fn a_huge_body_stretches_no_further_than_the_cap() {
        // The body is 900em tall; the delimiter is capped at 16em (LMATH-6) and is one path.
        let src = "\\left(\\rule{1em}{900em}\\right)";
        let found = paths(src, false);
        assert_eq!(found.len(), 2);
        for (_, _, _, height) in found {
            assert!((16.0..17.0).contains(&height), "{height}");
        }
        // A brace is pieces and two paths, however tall.
        let src = "\\left\\{\\rule{1em}{900em}\\right\\}";
        assert_eq!(paths(src, false).len(), 4);
        assert_eq!(glyphs(src, false).len(), 6);
    }

    #[test]
    fn unknown_delimiters_are_errors() {
        let options = Options::new(false);
        let err = middle("x", Mode::Math, &options, 3..4).map(|_| ());
        assert_eq!(err.unwrap_err().message, "Illegal delimiter: 'x'");
        let Err(err) = delim_sizing(5, MathClass::Ord, "(", Mode::Math, &options, 0..1) else {
            panic!("a size past 4")
        };
        assert_eq!(err.message, "Illegal delimiter size: 5");
    }

    #[test]
    fn heights_and_depths_agree_with_katex() {
        // KaTeX's answers (the baseline oracle's) for inputs with delimiters.
        for (src, height, depth) in [
            (
                "x^{x^{x}}\\Big|x_{x_{x_{x_{x}}}}\\bigg|x^{x^{x_{x_{x_{x_{x}}}}}}\\bigg|\n",
                1.44998,
                0.95002,
            ),
            ("a+\\left(x+y\\right)-x", 0.75, 0.25),
            (
                "\\left( x^2 \\right) \\left\\{ x^{x^{x^{x^x}}} \\right.",
                1.45,
                0.95003,
            ),
            (
                "\\left( x^2 \\middle/ \\right) \\left\\{ x^{x^{x^{x^x}}} \\middle/ y \\right.\\left(x\\middle|y\\,\\middle|\\,z\\right)",
                1.45,
                0.95003,
            ),
            ("a \\bigl. + 2 \\quad \\left. + a \\right)", 0.75, 0.25),
            (
                "\\dbinom{a}{b}\\tbinom{a}{b}^{\\binom{a}{b}+17}{\\scriptscriptstyle \\binom a b}",
                1.45,
                0.95003,
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
