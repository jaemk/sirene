//! Layout: the parse tree to a box tree (LMATH-2, step 4).
//!
//! Adapted from KaTeX's `src/buildHTML.ts`, `src/buildCommon.ts`, and the HTML builders of
//! `src/functions/` (MIT, Copyright (c) 2013-2020 Khan Academy and other contributors), with
//! TeX's rules from the TeXbook, Appendix G, as KaTeX applies them. Each node builds to one or
//! more atoms: a box with the math class TeX spaces it by. A list of atoms gets the spaces
//! between classes and has binary operators turned ordinary where TeX does. Boxes are sized in
//! em of the font they were built in, and a size change wraps the box in `Bx::scaled`, as
//! KaTeX's size classes do.
//!
//! Nodes not laid out yet fail with "... is not laid out yet", which the baseline test counts.

use std::borrow::Cow;
use std::ops::Range;

use super::{
    MathError,
    ast::{MathClass, Measurement, Node, NodeKind},
    boxes::{BoxKind, Bx},
    metrics,
    metrics_data::Face,
    options::{Em, MathFont, Options, Shape, TextFamily, Weight},
    style::Style,
    symbols::{self, AtomGroup, Font, Mode, Name},
};

/// Lays out a formula: its nodes in display or text style at `\normalsize`.
pub(crate) fn layout<'t>(nodes: &'t [Node<'_>], display: bool) -> Result<Bx<'t>, MathError> {
    let options = Options::new(display);
    let atoms = expression(nodes, &options, Grouping::Group)?;
    Ok(Bx::hlist(atoms.into_iter().map(|a| a.bx).collect()))
}

/// TeX's atom classes, as KaTeX's `mord` to `minner` classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Class {
    Ord,
    Op,
    Bin,
    Rel,
    Open,
    Close,
    Punct,
    Inner,
}

impl From<MathClass> for Class {
    fn from(class: MathClass) -> Self {
        match class {
            MathClass::Ord => Class::Ord,
            MathClass::Bin => Class::Bin,
            MathClass::Rel => Class::Rel,
            MathClass::Open => Class::Open,
            MathClass::Close => Class::Close,
            MathClass::Punct => Class::Punct,
            MathClass::Inner => Class::Inner,
        }
    }
}

/// A built box and how its neighbors space against it.
#[derive(Clone, Debug, PartialEq)]
struct Atom<'t> {
    bx: Bx<'t>,
    /// The class TeX spaces it by; `None` for a box that has none (a size change, a space).
    class: Option<Class>,
    /// An explicit space (`\,`, a kern, a math-mode space), which spacing passes over.
    space: bool,
    /// Built in script or scriptscript style, where fewer spaces apply.
    tight: bool,
}

impl<'t> Atom<'t> {
    fn new(bx: Bx<'t>, class: Option<Class>, options: &Options<'_>) -> Self {
        Self {
            bx,
            class,
            space: false,
            tight: options.style.is_tight(),
        }
    }

    fn space(bx: Bx<'t>, options: &Options<'_>) -> Self {
        Self {
            space: true,
            ..Self::new(bx, None, options)
        }
    }
}

/// What a node builds to: one atom, or atoms that join the list around them (KaTeX's document
/// fragment: `\color`, a size or style command, a `\begingroup` group).
enum Built<'t> {
    One(Atom<'t>),
    Many(Vec<Atom<'t>>),
}

impl<'t> Built<'t> {
    /// The atoms as one box.
    fn into_box(self) -> Bx<'t> {
        match self {
            Built::One(atom) => atom.bx,
            Built::Many(atoms) => Bx::hlist(atoms.into_iter().map(|a| a.bx).collect()),
        }
    }

    /// The class at the right edge, as KaTeX's `getTypeOfDomTree(node, "right")`.
    fn right_class(&self) -> Option<Class> {
        match self {
            Built::One(atom) => atom.class,
            Built::Many(atoms) => atoms.last().and_then(|a| a.class),
        }
    }
}

/// How a list of nodes stands: as part of the list around it (spacing is left to that list), or
/// as a group of its own. The formula is a group; line breaks are not built yet.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Grouping {
    Partial,
    Group,
}

fn not_yet(what: &str, span: Range<usize>) -> MathError {
    MathError::new(format!("{what} is not laid out yet"), span)
}

/// A list of nodes, as KaTeX's `buildExpression`.
fn expression<'t>(
    nodes: &'t [Node<'_>],
    options: &Options<'t>,
    grouping: Grouping,
) -> Result<Vec<Atom<'t>>, MathError> {
    let mut atoms = Vec::new();
    for node in nodes {
        match build(node, options)? {
            Built::One(atom) => atoms.push(atom),
            Built::Many(more) => atoms.extend(more),
        }
    }
    if grouping == Grouping::Partial {
        return Ok(atoms);
    }
    // A list of one size or style command takes its spaces in that size or style.
    let glue_options = match nodes {
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
    cancel_binary_operators(&mut atoms);
    Ok(insert_spaces(atoms, &glue_options))
}

/// The indices of the atoms that are not spaces, in order.
fn non_space(atoms: &[Atom<'_>]) -> Vec<usize> {
    (0..atoms.len()).filter(|&i| !atoms[i].space).collect()
}

/// TeX's rules 5 and 6: a binary operator with no operand on one side is ordinary. `None` is
/// the edge of the list.
fn cancel_binary_operators(atoms: &mut [Atom<'_>]) {
    let order = non_space(atoms);
    let neighbors = std::iter::once(None)
        .chain(order.iter().copied().map(Some))
        .zip(order.iter().copied().map(Some).chain(std::iter::once(None)));
    for (prev, next) in neighbors {
        let prev_class = prev.and_then(|p| atoms[p].class);
        let next_class = next.and_then(|n| atoms[n].class);
        if let Some(p) = prev
            && prev_class == Some(Class::Bin)
            && (next.is_none()
                || matches!(next_class, Some(Class::Rel | Class::Close | Class::Punct)))
        {
            atoms[p].class = Some(Class::Ord);
        } else if let Some(n) = next
            && next_class == Some(Class::Bin)
            && (prev.is_none()
                || matches!(
                    prev_class,
                    Some(Class::Bin | Class::Open | Class::Rel | Class::Op | Class::Punct)
                ))
        {
            atoms[n].class = Some(Class::Ord);
        }
    }
}

/// The spaces TeX puts between atoms by their classes, after the earlier atom and before any
/// explicit space that follows it.
fn insert_spaces<'t>(atoms: Vec<Atom<'t>>, options: &Options<'t>) -> Vec<Atom<'t>> {
    let order = non_space(&atoms);
    // The space to put after each atom, by index.
    let mut after: Vec<(usize, Em)> = Vec::new();
    for pair in order.windows(2) {
        let (prev, next) = (&atoms[pair[0]], &atoms[pair[1]]);
        let (Some(left), Some(right)) = (prev.class, next.class) else {
            continue;
        };
        if let Some(mu) = space_between(left, right, next.tight) {
            let size = Em(mu * options.metrics().css_em_per_mu);
            after.push((pair[0], size));
        }
    }
    if after.is_empty() {
        return atoms;
    }
    let mut out = Vec::with_capacity(atoms.len() + after.len());
    let mut spaces = after.into_iter().peekable();
    for (i, atom) in atoms.into_iter().enumerate() {
        out.push(atom);
        if let Some((_, size)) = spaces.next_if(|&(at, _)| at == i) {
            out.push(Atom::space(Bx::kern(size), options));
        }
    }
    out
}

/// TeX's table of spaces between classes, in mu: 3 thin, 4 medium, 5 thick. Script and
/// scriptscript styles keep only the thin spaces around operators.
fn space_between(left: Class, right: Class, tight: bool) -> Option<f64> {
    use Class::*;
    let (thin, medium, thick) = (Some(3.0), Some(4.0), Some(5.0));
    if tight {
        return match (left, right) {
            (Ord | Op | Close | Inner, Op) | (Op, Ord) => thin,
            _ => None,
        };
    }
    match (left, right) {
        (Ord, Op | Inner) | (Op, Ord | Op | Inner) => thin,
        (Ord | Close | Inner, Bin) => medium,
        (Ord | Op | Close | Inner, Rel) => thick,
        (Bin, Ord | Op | Open | Inner) => medium,
        (Rel, Ord | Op | Open | Inner) => thick,
        (Close, Op | Inner) => thin,
        (Punct, Ord | Op | Open | Close | Punct | Inner) => thin,
        (Punct, Rel) => thick,
        (Inner, Ord | Op | Open | Punct | Inner) => thin,
        _ => None,
    }
}

/// A node in `options`, then sized for `base` if the size changed (KaTeX's `buildGroup` with a
/// base options).
fn build_in<'t>(
    node: &'t Node<'_>,
    options: &Options<'t>,
    base: &Options<'t>,
) -> Result<Built<'t>, MathError> {
    let built = build(node, options)?;
    if options.size == base.size {
        return Ok(built);
    }
    let factor = options.size_multiplier() / base.size_multiplier();
    Ok(Built::One(Atom::new(
        Bx::scaled(factor, built.into_box()),
        None,
        options,
    )))
}

/// The nodes of a size or style change, each sized for the options around them (KaTeX's
/// `sizingGroup`).
fn sizing_group<'t>(
    nodes: &'t [Node<'_>],
    options: &Options<'t>,
    base: &Options<'t>,
) -> Result<Built<'t>, MathError> {
    let factor = options.size_multiplier() / base.size_multiplier();
    let atoms = expression(nodes, options, Grouping::Partial)?
        .into_iter()
        .map(|atom| Atom {
            bx: Bx::scaled(factor, atom.bx),
            ..atom
        })
        .collect();
    Ok(Built::Many(atoms))
}

/// One node, as the HTML builder of its KaTeX type.
fn build<'t>(node: &'t Node<'_>, options: &Options<'t>) -> Result<Built<'t>, MathError> {
    let span = node.span.clone();
    Ok(match &node.kind {
        NodeKind::Symbol { group, name } => symbol(*group, *name, node.mode, options, span)?,
        NodeKind::CharCode(c) => {
            let text = Cow::Owned(c.to_string());
            ord(text, None, false, node.mode, options, span)?
        }
        NodeKind::Group { body, semisimple } => {
            if *semisimple {
                Built::Many(expression(body, options, Grouping::Partial)?)
            } else {
                let atoms = expression(body, options, Grouping::Group)?;
                group(atoms, Some(Class::Ord), options)
            }
        }
        NodeKind::SupSub { base, sup, sub } => {
            supsub(base.as_deref(), sup.as_deref(), sub.as_deref(), options)?
        }
        NodeKind::Frac {
            numer,
            denom,
            continued,
            bar,
            bar_size,
            left,
            right,
        } => {
            if left.is_some() || right.is_some() {
                return Err(not_yet("a fraction with delimiters", span));
            }
            let bar_size = bar_size.as_ref().map(|size| (size, span.clone()));
            fraction(numer, denom, *continued, *bar, bar_size, options)?
        }
        NodeKind::Styling {
            style,
            body,
            reset_font,
        } => {
            let mut inner = options.having_style((*style).into());
            if *reset_font {
                inner.font = None;
            }
            sizing_group(body, &inner, options)?
        }
        NodeKind::Sizing { size, body } => {
            sizing_group(body, &options.having_size(*size), options)?
        }
        NodeKind::Color { color, body } => {
            let inner = options.with_color(color);
            Built::Many(expression(body, &inner, Grouping::Partial)?)
        }
        NodeKind::Kern { dimension } => Built::One(Atom::space(
            Bx::kern(options.length(dimension, span)?),
            options,
        )),
        NodeKind::Font { font, body } => {
            let Some(font) = MathFont::from_name(font) else {
                return Err(not_yet(&format!("the font {font}"), span));
            };
            build(body, &options.with_font(font))?
        }
        NodeKind::Text { body, font } => {
            let inner = text_options(*font, options);
            let atoms = expression(body, &inner, Grouping::Group)?;
            group(atoms, Some(Class::Ord), &inner)
        }
        NodeKind::MClass { class, body, .. } => {
            let atoms = expression(body, options, Grouping::Group)?;
            group(atoms, Some((*class).into()), options)
        }
        NodeKind::Rule {
            shift,
            width,
            height,
        } => {
            let width = options.length(width, span.clone())?;
            let height = options.length(height, span.clone())?;
            let shift = match shift {
                Some(shift) => options.length(shift, span)?,
                None => Em::ZERO,
            };
            let rule = Bx::rule(width, height + shift, -shift, options);
            Built::One(Atom::new(rule, Some(Class::Ord), options))
        }
        NodeKind::Internal => Built::Many(Vec::new()),
        other => return Err(not_yet(other.type_name(), span)),
    })
}

/// Atoms as one box of a class (KaTeX's `makeSpan([class], children)`).
fn group<'t>(atoms: Vec<Atom<'t>>, class: Option<Class>, options: &Options<'_>) -> Built<'t> {
    let bx = Bx::hlist(atoms.into_iter().map(|a| a.bx).collect());
    Built::One(Atom::new(bx, class, options))
}

/// The options of `\text` and its font variants.
fn text_options<'t>(font: Option<&str>, options: &Options<'t>) -> Options<'t> {
    match font {
        Some("textrm" | "textnormal") => options.with_family(TextFamily::Rm),
        Some("textsf") => options.with_family(TextFamily::Sf),
        Some("texttt") => options.with_family(TextFamily::Tt),
        Some("textbf") => options.with_weight(Weight::Bold),
        Some("textmd") => options.with_weight(Weight::Medium),
        Some("textit") => options.with_shape(Shape::Italic),
        Some("textup") => options.with_shape(Shape::Upright),
        Some("emph") => {
            if options.shape == Some(Shape::Italic) {
                options.with_shape(Shape::Upright)
            } else {
                options.with_shape(Shape::Italic)
            }
        }
        // KaTeX gives plain `\text` the shape of no command, which clears the shape.
        Some("text") => Options {
            shape: None,
            font: None,
            ..*options
        },
        _ => *options,
    }
}

/// A symbol node: an ordinary symbol, an operator or relation from the main or AMS font, or a
/// space.
fn symbol<'t>(
    group: AtomGroup,
    name: Name<'t>,
    mode: Mode,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let class = match group {
        AtomGroup::MathOrd => return ord(source_text(name), Some(name), true, mode, options, span),
        AtomGroup::TextOrd => {
            return ord(source_text(name), Some(name), false, mode, options, span);
        }
        AtomGroup::Spacing => {
            if matches!(name, Name::Control("nobreak" | "allowbreak")) {
                return Ok(Built::One(Atom::space(Bx::kern(Em::ZERO), options)));
            }
            if mode == Mode::Text {
                return ord(source_text(name), Some(name), false, mode, options, span);
            }
            let glyph = math_symbol(name, mode, options);
            let bx = Bx::hlist(vec![glyph]);
            return Ok(Built::One(Atom::space(bx, options)));
        }
        AtomGroup::Bin => Class::Bin,
        AtomGroup::Rel => Class::Rel,
        AtomGroup::Open => Class::Open,
        AtomGroup::Close => Class::Close,
        AtomGroup::Punct => Class::Punct,
        AtomGroup::Inner => Class::Inner,
        AtomGroup::Op | AtomGroup::Accent => {
            return Err(not_yet("an operator or accent symbol", span));
        }
    };
    let glyph = math_symbol(name, mode, options);
    Ok(Built::One(Atom::new(glyph, Some(class), options)))
}

/// The text of a symbol as the source writes it (KaTeX's `group.text`).
fn source_text(name: Name<'_>) -> Cow<'_, str> {
    match name {
        Name::Char(text) => Cow::Borrowed(text),
        Name::Control(word) => Cow::Owned(format!("\\{word}")),
    }
}

/// What a symbol draws: the table's text for it, or the source's character.
fn drawn_text<'t>(name: Option<Name<'t>>, text: &Cow<'t, str>, mode: Mode) -> Cow<'t, str> {
    match name.and_then(|name| symbols::lookup(mode, name)) {
        Some(symbol) => Cow::Borrowed(symbol.replace),
        None => text.clone(),
    }
}

fn has_metrics(text: &str, face: Face, mode: Mode) -> bool {
    text.chars()
        .next()
        .is_some_and(|c| metrics::glyph(c, face, mode).is_some())
}

/// An operator, relation, delimiter, or punctuation symbol in the main font or the AMS font, as
/// its table entry says (KaTeX's `mathsym`).
fn math_symbol<'t>(name: Name<'t>, mode: Mode, options: &Options<'t>) -> Bx<'t> {
    let symbol = symbols::lookup(mode, name);
    let text = drawn_text(Some(name), &source_text(name), mode);
    let face =
        if options.font == Some(MathFont::BoldSymbol) && has_metrics(&text, Face::MainBold, mode) {
            Face::MainBold
        } else if name == Name::Char("\\") || symbol.is_none_or(|s| s.font == Font::Main) {
            Face::MainRegular
        } else {
            Face::AmsRegular
        };
    Bx::glyph(text, face, mode, options)
}

/// An ordinary symbol, a letter or digit, in the font the options pick (KaTeX's `makeOrd`).
/// `name` is the symbol's table name, if it has one; `mathord` says it is set as a variable.
fn ord<'t>(
    text: Cow<'t, str>,
    name: Option<Name<'t>>,
    mathord: bool,
    mode: Mode,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    if text
        .chars()
        .next()
        .is_some_and(|c| ('\u{1d400}'..='\u{1d7ff}').contains(&c))
    {
        return Err(not_yet("a mathematical alphanumeric character", span));
    }
    let drawn = drawn_text(name, &text, mode);
    let one = |face: Face| {
        Built::One(Atom::new(
            Bx::glyph(drawn.clone(), face, mode, options),
            Some(Class::Ord),
            options,
        ))
    };
    let use_font = mode == Mode::Math || options.font.is_some();
    let chosen = if use_font {
        match options.font {
            Some(MathFont::BoldSymbol) => Some(
                if mathord && has_metrics(&drawn, Face::MathBoldItalic, mode) {
                    Face::MathBoldItalic
                } else {
                    Face::MainBold
                },
            ),
            Some(font) => Some(math_font_face(font)),
            None => None,
        }
    } else if let Some(family) = options.family {
        Some(text_face(family, options).map_err(|message| MathError::new(message, span.clone()))?)
    } else {
        None
    };
    if let Some(face) = chosen {
        if has_metrics(&drawn, face, mode) {
            return Ok(one(face));
        }
        // A monospace font has no ligatures: `--` is two hyphens.
        if face == Face::TypewriterRegular && matches!(text.as_ref(), "--" | "---" | "``" | "''") {
            let atoms = text
                .chars()
                .map(|c| {
                    let glyph = Bx::glyph(Cow::Owned(c.to_string()), face, mode, options);
                    Atom::new(glyph, Some(Class::Ord), options)
                })
                .collect();
            return Ok(Built::Many(atoms));
        }
    }
    if mathord {
        return Ok(one(Face::MathItalic));
    }
    let family = match name.and_then(|name| symbols::lookup(mode, name)) {
        Some(symbol) if symbol.font == Font::Ams => TextFamily::Ams,
        _ => TextFamily::Rm,
    };
    let face = text_face(family, options).map_err(|message| MathError::new(message, span))?;
    Ok(one(face))
}

/// The face of a math font command (KaTeX's `fontMap`).
fn math_font_face(font: MathFont) -> Face {
    match font {
        MathFont::Rm => Face::MainRegular,
        MathFont::It => Face::MainItalic,
        MathFont::Bf | MathFont::BoldSymbol => Face::MainBold,
        MathFont::Normal => Face::MathItalic,
        MathFont::SfIt => Face::SansSerifItalic,
        MathFont::Bb => Face::AmsRegular,
        MathFont::Cal => Face::CaligraphicRegular,
        MathFont::Frak => Face::FrakturRegular,
        MathFont::Scr => Face::ScriptRegular,
        MathFont::Sf => Face::SansSerifRegular,
        MathFont::Tt => Face::TypewriterRegular,
    }
}

/// The face of a text family in the options' weight and shape (KaTeX's
/// `retrieveTextFontName`). KaTeX has no metrics for some pairs (`\textbf{\textsf{\textit x}}`)
/// and fails there with this message.
fn text_face(family: TextFamily, options: &Options<'_>) -> Result<Face, String> {
    let bold = options.weight == Some(Weight::Bold);
    let italic = options.shape == Some(Shape::Italic);
    let face = match (family, bold, italic) {
        (TextFamily::Rm, true, true) => Some(Face::MainBoldItalic),
        (TextFamily::Rm, true, false) => Some(Face::MainBold),
        (TextFamily::Rm, false, true) => Some(Face::MainItalic),
        (TextFamily::Rm, false, false) => Some(Face::MainRegular),
        (TextFamily::Sf, true, false) => Some(Face::SansSerifBold),
        (TextFamily::Sf, false, true) => Some(Face::SansSerifItalic),
        (TextFamily::Sf, false, false) => Some(Face::SansSerifRegular),
        (TextFamily::Tt, false, false) => Some(Face::TypewriterRegular),
        (TextFamily::Ams, false, false) => Some(Face::AmsRegular),
        _ => None,
    };
    face.ok_or_else(|| {
        let base = match family {
            TextFamily::Rm => "Main",
            TextFamily::Sf => "SansSerif",
            TextFamily::Tt => "Typewriter",
            TextFamily::Ams => "AMS",
        };
        let style = match (bold, italic) {
            (true, true) => "BoldItalic",
            (true, false) => "Bold",
            (false, true) => "Italic",
            (false, false) => "Regular",
        };
        format!("Font metrics not found for font: {base}-{style}.")
    })
}

/// A box with space after it (KaTeX's `marginRight` on a stack's child).
fn with_space_after(bx: Bx<'_>, space: Em) -> Bx<'_> {
    Bx::hlist(vec![bx, Bx::kern(space)])
}

/// Superscripts and subscripts, by the TeXbook's rules 18a to 18f.
fn supsub<'t>(
    base: Option<&'t Node<'_>>,
    sup: Option<&'t Node<'_>>,
    sub: Option<&'t Node<'_>>,
    options: &Options<'t>,
) -> Result<Built<'t>, MathError> {
    let built = match base {
        Some(base) => build(base, options)?,
        None => Built::One(Atom::new(Bx::hlist(Vec::new()), None, options)),
    };
    let class = built.right_class().unwrap_or(Class::Ord);
    // A subscript does not move right by a lone symbol's italic correction.
    let italic = match &built {
        Built::One(Atom {
            bx: Bx {
                kind: BoxKind::Glyph(glyph),
                ..
            },
            ..
        }) => glyph.italic,
        _ => Em::ZERO,
    };
    let base_box = built.into_box();
    let metrics = options.metrics();
    let char_box = base.is_some_and(Node::is_character_box);

    let mut sup_shift = 0.0;
    let mut sub_shift = 0.0;
    let mut sup_box = None;
    let mut sub_box = None;
    if let Some(sup) = sup {
        let inner = options.having_style(options.style.sup());
        let bx = build_in(sup, &inner, options)?.into_box();
        if !char_box {
            sup_shift = base_box.height.0
                - inner.metrics().sup_drop * inner.size_multiplier() / options.size_multiplier();
        }
        sup_box = Some(bx);
    }
    if let Some(sub) = sub {
        let inner = options.having_style(options.style.sub());
        let bx = build_in(sub, &inner, options)?.into_box();
        if !char_box {
            sub_shift = base_box.depth.0
                + inner.metrics().sub_drop * inner.size_multiplier() / options.size_multiplier();
        }
        sub_box = Some(bx);
    }

    let min_sup_shift = if options.style == Style::Display {
        metrics.sup1
    } else if options.style.is_cramped() {
        metrics.sup3
    } else {
        metrics.sup2
    };
    // TeX's \scriptspace, 0.5pt at any size.
    let script_space = Em(0.5 / metrics.pt_per_em / options.size_multiplier());
    let margin_left = -italic;

    let scripts = match (sup_box, sub_box) {
        (Some(supm), Some(subm)) => {
            sup_shift = sup_shift
                .max(min_sup_shift)
                .max(supm.depth.0 + 0.25 * metrics.x_height);
            sub_shift = sub_shift.max(metrics.sub2);
            let max_width = 4.0 * metrics.default_rule_thickness;
            if (sup_shift - supm.depth.0) - (subm.height.0 - sub_shift) < max_width {
                sub_shift = max_width - (sup_shift - supm.depth.0) + subm.height.0;
                let psi = 0.8 * metrics.x_height - (sup_shift - supm.depth.0);
                if psi > 0.0 {
                    sup_shift += psi;
                    sub_shift -= psi;
                }
            }
            Bx::vlist_shifted(vec![
                (
                    with_space_after(subm, script_space),
                    Em(sub_shift),
                    margin_left,
                ),
                (
                    with_space_after(supm, script_space),
                    Em(-sup_shift),
                    Em::ZERO,
                ),
            ])
        }
        (None, Some(subm)) => {
            sub_shift = sub_shift
                .max(metrics.sub1)
                .max(subm.height.0 - 0.8 * metrics.x_height);
            Bx::vlist(
                super::boxes::Position::Shift(Em(sub_shift)),
                vec![super::boxes::Part::Box {
                    child: with_space_after(subm, script_space),
                    dx: margin_left,
                }],
            )
        }
        (Some(supm), None) => {
            sup_shift = sup_shift
                .max(min_sup_shift)
                .max(supm.depth.0 + 0.25 * metrics.x_height);
            Bx::vlist(
                super::boxes::Position::Shift(Em(-sup_shift)),
                vec![super::boxes::Part::of(with_space_after(supm, script_space))],
            )
        }
        (None, None) => Bx::hlist(Vec::new()),
    };
    let bx = Bx::hlist(vec![base_box, scripts]);
    Ok(Built::One(Atom::new(bx, Some(class), options)))
}

/// A fraction, by the TeXbook's rules 15a to 15e (KaTeX's `genfrac`), without delimiters.
fn fraction<'t>(
    numer: &'t Node<'_>,
    denom: &'t Node<'_>,
    continued: bool,
    bar: bool,
    bar_size: Option<(&Measurement, Range<usize>)>,
    options: &Options<'t>,
) -> Result<Built<'t>, MathError> {
    let style = options.style;
    let metrics = options.metrics();
    let numer_options = options.having_style(style.frac_num());
    let mut numer = build_in(numer, &numer_options, options)?.into_box();
    if continued {
        // `\cfrac` puts a strut in the numerator (TeXbook p. 353).
        numer.height = numer.height.max(Em(8.5 / metrics.pt_per_em));
        numer.depth = numer.depth.max(Em(3.5 / metrics.pt_per_em));
    }
    let denom_options = options.having_style(style.frac_den());
    let denom = build_in(denom, &denom_options, options)?.into_box();

    let width = numer.width.max(denom.width);
    let rule = if bar {
        let thickness = match bar_size {
            Some((size, span)) => options.length(size, span)?,
            None => Em(metrics.default_rule_thickness),
        };
        Some(Bx::line(width, thickness, options))
    } else {
        None
    };
    let (rule_width, rule_spacing) = match &rule {
        Some(rule) => (rule.height.0, rule.height.0),
        None => (0.0, metrics.default_rule_thickness),
    };

    let display = style.size() == Style::Display.size();
    let (mut num_shift, clearance, mut denom_shift) = match (display, rule_width > 0.0) {
        (true, true) => (metrics.num1, 3.0 * rule_spacing, metrics.denom1),
        (true, false) => (metrics.num1, 7.0 * rule_spacing, metrics.denom1),
        (false, true) => (metrics.num2, rule_spacing, metrics.denom2),
        (false, false) => (metrics.num3, 3.0 * rule_spacing, metrics.denom2),
    };

    let center = |bx: &Bx<'_>| (width - bx.width) / 2.0;
    let numer_dx = center(&numer);
    let denom_dx = center(&denom);
    let stack = match rule {
        None => {
            let candidate = (num_shift - numer.depth.0) - (denom.height.0 - denom_shift);
            if candidate < clearance {
                num_shift += 0.5 * (clearance - candidate);
                denom_shift += 0.5 * (clearance - candidate);
            }
            Bx::vlist_shifted(vec![
                (denom, Em(denom_shift), denom_dx),
                (numer, Em(-num_shift), numer_dx),
            ])
        }
        Some(rule) => {
            let axis = metrics.axis_height;
            let above = (num_shift - numer.depth.0) - (axis + 0.5 * rule_width);
            if above < clearance {
                num_shift += clearance - above;
            }
            let below = (axis - 0.5 * rule_width) - (denom.height.0 - denom_shift);
            if below < clearance {
                denom_shift += clearance - below;
            }
            let mid_shift = -(axis - 0.5 * rule_width);
            Bx::vlist_shifted(vec![
                (denom, Em(denom_shift), denom_dx),
                (rule, Em(mid_shift), Em::ZERO),
                (numer, Em(-num_shift), numer_dx),
            ])
        }
    };

    // The null delimiters, 1.2pt of the base size each (`\nulldelimiterspace`).
    let null = || Bx::kern(Em(0.12 / options.size_multiplier()));
    let right = if continued {
        Bx::kern(Em::ZERO)
    } else {
        null()
    };
    let bx = Bx::hlist(vec![null(), stack, right]);
    Ok(Built::One(Atom::new(bx, Some(Class::Ord), options)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::parser::{Settings, parse_with};

    fn laid_out(src: &str, display: bool) -> (f64, f64, f64) {
        let nodes = parse_with(
            src,
            Settings {
                display_mode: display,
            },
        )
        .unwrap();
        let bx = layout(&nodes, display).unwrap();
        (bx.width.0, bx.height.0, bx.depth.0)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn letters_and_spaces_between_classes() {
        // x + y: two italic letters and a plus with medium spaces (4mu each).
        let (w, h, d) = laid_out("x+y", false);
        let x = 0.57153;
        let plus = 0.77778;
        let y = 0.49028 + 0.03588;
        assert!(close(w, x + plus + y + 2.0 * 4.0 / 18.0), "{w}");
        assert!(close(h, 0.58333), "{h}");
        assert!(close(d, 0.19444), "{d}");
        // A leading plus is ordinary: no spaces.
        let (w, ..) = laid_out("+x", false);
        assert!(close(w, plus + x), "{w}");
        // A relation takes thick spaces.
        let (w, ..) = laid_out("x=x", false);
        assert!(close(w, 2.0 * x + 0.77778 + 2.0 * 5.0 / 18.0), "{w}");
    }

    #[test]
    fn scripts_shrink_and_move() {
        let (_, h, d) = laid_out("x^2", false);
        assert!(close(h, 0.8141079999999999), "{h}");
        assert!(close(d, 0.0), "{d}");
        let (_, h, d) = laid_out("x_2", false);
        assert!(close(h, 0.43056), "{h}");
        assert!(close(d, 0.15), "{d}");
    }

    #[test]
    fn fractions_center_on_the_axis() {
        let (_, h, d) = laid_out("\\frac12", false);
        assert!(close(h, 0.845108), "{h}");
        assert!(close(d, 0.345), "{d}");
        let (_, h, d) = laid_out("\\frac12", true);
        assert!(close(h, 1.32144), "{h}");
        assert!(close(d, 0.686), "{d}");
    }

    #[test]
    fn unbuilt_nodes_say_so() {
        let nodes = parse_with(
            "\\sqrt x",
            Settings {
                display_mode: false,
            },
        )
        .unwrap();
        let err = layout(&nodes, false).unwrap_err();
        assert_eq!(err.message, "sqrt is not laid out yet");
    }
}
