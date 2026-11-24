//! Layout: the parse tree to a box tree (LMATH-2, step 4).
//!
//! Adapted from KaTeX's `src/buildHTML.ts`, `src/buildCommon.ts`, `src/spacingData.ts`, and the
//! HTML builders of `src/functions/` (MIT, Copyright (c) 2013-2020 Khan Academy and other
//! contributors), with TeX's rules from the TeXbook, Appendix G, as KaTeX applies them. Each
//! node builds to one or more atoms: a box with the math class TeX spaces it by. A list of atoms
//! gets the spaces between classes and has binary operators turned ordinary where TeX does.
//! Boxes are sized in em of the font they were built in, and a size change wraps the box in
//! `Bx::scaled`, as KaTeX's size classes do.
//!
//! This module holds the list rules and the dispatch; each family of nodes has its own module.
//! Nodes not laid out yet fail with "... is not laid out yet", which the baseline test counts.

mod accent;
mod array;
mod arrow;
mod delimiter;
mod enclose;
mod frac;
mod horiz_brace;
mod misc;
mod op;
mod scripts;
mod sqrt;
mod stretchy;
mod symbol;

use std::borrow::Cow;
use std::ops::Range;

use super::{
    MathError,
    ast::{MathClass, Node, NodeKind},
    boxes::Bx,
    options::{Em, MathFont, Options},
};

/// Lays out a formula: its nodes in display or text style at `\normalsize`.
pub(crate) fn layout<'t>(nodes: &'t [Node<'_>], display: bool) -> Result<Bx<'t>, MathError> {
    let options = Options::new(display);
    let atoms = expression(nodes, &options, Grouping::Root)?;
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
    /// A line break (`\\` outside a table), which starts a new line for spacing at the root.
    newline: bool,
}

impl<'t> Atom<'t> {
    fn new(bx: Bx<'t>, class: Option<Class>, options: &Options<'_>) -> Self {
        Self {
            bx,
            class,
            space: false,
            tight: options.style.is_tight(),
            newline: false,
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

/// How a list of nodes stands: as part of the list around it (spacing is left to that list), as
/// a group of its own, or as the formula itself, where a line break starts spacing afresh as the
/// start of a list does (KaTeX's `"root"`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Grouping {
    Partial,
    Group,
    Root,
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
    let root = grouping == Grouping::Root;
    cancel_binary_operators(&mut atoms, root);
    Ok(insert_spaces(atoms, &glue_options, root))
}

/// The pairs of neighbors that spacing looks at: the atoms that are not spaces, in order, from
/// the start of the list (`None`) to its end (`None`). At the root a line break starts over, as
/// KaTeX's `traverseNonSpaceNodes` does: the atom after it has the start of a list on its left,
/// and the atom before it is paired with nothing on its right.
fn neighbors(atoms: &[Atom<'_>], root: bool) -> Vec<(Option<usize>, Option<usize>)> {
    let mut pairs = Vec::new();
    let mut prev = None;
    for (i, atom) in atoms.iter().enumerate() {
        if atom.space {
            if root && atom.newline {
                prev = None;
            }
            continue;
        }
        pairs.push((prev, Some(i)));
        prev = Some(i);
    }
    pairs.push((prev, None));
    pairs
}

/// TeX's rules 5 and 6: a binary operator with no operand on one side is ordinary. `None` is
/// the edge of the list.
fn cancel_binary_operators(atoms: &mut [Atom<'_>], root: bool) {
    for (prev, next) in neighbors(atoms, root) {
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
fn insert_spaces<'t>(atoms: Vec<Atom<'t>>, options: &Options<'t>, root: bool) -> Vec<Atom<'t>> {
    // The space to put after each atom, by index.
    let mut after: Vec<(usize, Em)> = Vec::new();
    for pair in neighbors(&atoms, root) {
        let (Some(p), Some(n)) = pair else {
            continue;
        };
        let (prev, next) = (&atoms[p], &atoms[n]);
        let (Some(left), Some(right)) = (prev.class, next.class) else {
            continue;
        };
        if let Some(mu) = space_between(left, right, next.tight) {
            let size = Em(mu * options.metrics().css_em_per_mu);
            after.push((p, size));
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

/// Atoms as one box of a class (KaTeX's `makeSpan([class], children)`).
fn group<'t>(atoms: Vec<Atom<'t>>, class: Option<Class>, options: &Options<'_>) -> Built<'t> {
    let bx = Bx::hlist(atoms.into_iter().map(|a| a.bx).collect());
    Built::One(Atom::new(bx, class, options))
}

/// One node, as the HTML builder of its KaTeX type.
fn build<'t>(node: &'t Node<'_>, options: &Options<'t>) -> Result<Built<'t>, MathError> {
    let span = node.span.clone();
    let mode = node.mode;
    match &node.kind {
        NodeKind::Symbol { group, name } => symbol::symbol(*group, *name, mode, options, span),
        NodeKind::CharCode(c) => {
            let text = Cow::Owned(c.to_string());
            symbol::ord(text, None, false, mode, options, span)
        }
        NodeKind::Group { body, semisimple } => {
            if *semisimple {
                Ok(Built::Many(expression(body, options, Grouping::Partial)?))
            } else {
                let atoms = expression(body, options, Grouping::Group)?;
                Ok(group(atoms, Some(Class::Ord), options))
            }
        }
        NodeKind::SupSub { base, sup, sub } => {
            scripts::supsub(base.as_deref(), sup.as_deref(), sub.as_deref(), options)
        }
        NodeKind::Frac {
            numer,
            denom,
            continued,
            bar,
            bar_size,
            left,
            right,
        } => frac::fraction(
            frac::Fraction {
                numer,
                denom,
                continued: *continued,
                bar: *bar,
                bar_size: bar_size.as_ref(),
                left: left.as_deref(),
                right: right.as_deref(),
                mode,
                span,
            },
            options,
        ),
        NodeKind::Styling {
            style,
            body,
            reset_font,
        } => {
            let mut inner = options.having_style((*style).into());
            if *reset_font {
                inner.font = None;
            }
            sizing_group(body, &inner, options)
        }
        NodeKind::Sizing { size, body } => sizing_group(body, &options.having_size(*size), options),
        NodeKind::Color { color, body } => {
            let inner = options.with_color(color);
            Ok(Built::Many(expression(body, &inner, Grouping::Partial)?))
        }
        NodeKind::Kern { dimension } => Ok(Built::One(Atom::space(
            Bx::kern(options.length(dimension, span)?),
            options,
        ))),
        NodeKind::Font { font, body } => {
            let Some(font) = MathFont::from_name(font) else {
                return Err(not_yet(&format!("the font {font}"), span));
            };
            build(body, &options.with_font(font))
        }
        NodeKind::Text { body, font } => {
            let inner = symbol::text_options(*font, options);
            let atoms = expression(body, &inner, Grouping::Group)?;
            Ok(group(atoms, Some(Class::Ord), &inner))
        }
        NodeKind::MClass { class, body, .. } => {
            let atoms = expression(body, options, Grouping::Group)?;
            Ok(group(atoms, Some((*class).into()), options))
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
            Ok(Built::One(Atom::new(rule, Some(Class::Ord), options)))
        }
        NodeKind::Internal => Ok(Built::Many(Vec::new())),

        // Delimiters and roots.
        NodeKind::DelimSizing { size, class, delim } => {
            delimiter::delim_sizing(*size, *class, delim, mode, options, span)
        }
        NodeKind::LeftRight {
            body,
            left,
            right,
            right_color,
        } => delimiter::left_right(
            body,
            (left, right),
            right_color.as_deref(),
            mode,
            options,
            span,
        ),
        NodeKind::Middle { delim } => delimiter::middle(delim, mode, options, span),
        NodeKind::Sqrt { body, index } => sqrt::sqrt(body, index.as_deref(), options, span),

        // Operators.
        NodeKind::Op {
            limits,
            symbol,
            suppress_base_shift,
            content,
            ..
        } => op::op(
            op::Operator {
                limits: *limits,
                symbol: *symbol,
                suppress_base_shift: *suppress_base_shift,
                content,
                mode,
                span,
            },
            options,
        ),
        NodeKind::OperatorName { body, .. } => op::operator_name(body, mode, options, span),

        // Accents and stretchy shapes.
        NodeKind::Accent {
            label,
            stretchy,
            shifty,
            base,
        } => accent::accent(
            accent::Accent {
                label,
                stretchy: *stretchy,
                shifty: *shifty,
                base,
                mode,
                span,
            },
            options,
        ),
        NodeKind::AccentUnder { label, base } => {
            accent::accent_under(label, base, mode, options, span)
        }
        NodeKind::XArrow { label, body, below } => {
            arrow::x_arrow(label, body, below.as_deref(), options, span)
        }
        NodeKind::HorizBrace {
            label,
            is_over,
            base,
        } => horiz_brace::horiz_brace(label, *is_over, base, options, span),
        NodeKind::Enclose {
            label,
            background_color,
            border_color,
            body,
        } => enclose::enclose(
            enclose::Enclosure {
                label,
                background_color: background_color.as_deref(),
                border_color: border_color.as_deref(),
                body,
                mode,
                span,
            },
            options,
        ),

        // Tables and line breaks.
        NodeKind::Array(array) => array::array(array, mode, options, span),
        NodeKind::Cr { new_line, size } => array::cr(*new_line, size.as_ref(), options, span),
        NodeKind::Tag { body, tag } => array::tag(body, tag, options, span),

        // Size and placement.
        NodeKind::Phantom { body } => misc::phantom(body, options, span),
        NodeKind::VPhantom { body } => misc::vphantom(body, options, span),
        NodeKind::Smash {
            body,
            height,
            depth,
        } => misc::smash(body, (*height, *depth), options, span),
        NodeKind::Lap { alignment, body } => misc::lap(alignment, body, options, span),
        NodeKind::RaiseBox { dy, body } => misc::raise_box(dy, body, options, span),
        NodeKind::VCenter { body } => misc::vcenter(body, options, span),
        NodeKind::Underline { body } => misc::underline(body, options, span),
        NodeKind::Overline { body } => misc::overline(body, options, span),
        NodeKind::Pmb { class, body } => misc::pmb(*class, body, options, span),
        NodeKind::ReflectBox { body } => misc::reflect_box(body, options, span),
        NodeKind::MathChoice {
            display,
            text,
            script,
            script_script,
        } => misc::math_choice([display, text, script, script_script], options, span),
        NodeKind::HtmlMathml { html, .. } => misc::html_mathml(html, options, span),
        NodeKind::HBox { body } => misc::hbox(body, options, span),
        NodeKind::Verb { body, star } => misc::verb(body, *star, mode, options, span),

        // Arguments as read, which a function takes apart before layout: KaTeX has no builder
        // for them either.
        NodeKind::Infix { .. }
        | NodeKind::Size { .. }
        | NodeKind::ColorToken(_)
        | NodeKind::Environment { .. }
        | NodeKind::Raw { .. }
        | NodeKind::Url { .. } => Err(MathError::new(
            format!("Got group of unknown type: '{}'", node.kind.type_name()),
            span,
        )),
    }
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
