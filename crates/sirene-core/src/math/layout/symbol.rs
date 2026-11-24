//! Symbols: letters, digits, operators, relations, delimiters, and spaces, and the face each is
//! drawn in.
//!
//! Adapted from KaTeX's `src/buildCommon.ts` (`makeOrd`, `mathsym`, `retrieveTextFontName`,
//! `fontMap`), `src/functions/symbolsOrd.ts`, `symbolsOp.ts`, `symbolsSpacing.ts`, and
//! `text.ts` (MIT, Copyright (c) 2013-2020 Khan Academy and other contributors).

use std::borrow::Cow;
use std::ops::Range;

use super::{Atom, Built, Class, not_yet};
use crate::math::{
    MathError,
    boxes::Bx,
    metrics,
    metrics_data::Face,
    options::{Em, MathFont, Options, Shape, TextFamily, Weight},
    symbols::{self, AtomGroup, Font, Mode, Name},
};

/// The options of `\text` and its font variants.
pub(super) fn text_options<'t>(font: Option<&str>, options: &Options<'t>) -> Options<'t> {
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
pub(super) fn symbol<'t>(
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
pub(super) fn source_text(name: Name<'_>) -> Cow<'_, str> {
    match name {
        Name::Char(text) => Cow::Borrowed(text),
        Name::Control(word) => Cow::Owned(format!("\\{word}")),
    }
}

/// What a symbol draws: the table's text for it, or the source's character.
pub(super) fn drawn_text<'t>(
    name: Option<Name<'t>>,
    text: &Cow<'t, str>,
    mode: Mode,
) -> Cow<'t, str> {
    match name.and_then(|name| symbols::lookup(mode, name)) {
        Some(symbol) => Cow::Borrowed(symbol.replace),
        None => text.clone(),
    }
}

/// Whether the face has metrics for the first character of `text`.
pub(super) fn has_metrics(text: &str, face: Face, mode: Mode) -> bool {
    text.chars()
        .next()
        .is_some_and(|c| metrics::glyph(c, face, mode).is_some())
}

/// An operator, relation, delimiter, or punctuation symbol in the main font or the AMS font, as
/// its table entry says (KaTeX's `mathsym`).
pub(super) fn math_symbol<'t>(name: Name<'t>, mode: Mode, options: &Options<'t>) -> Bx<'t> {
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
pub(super) fn ord<'t>(
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
pub(super) fn math_font_face(font: MathFont) -> Face {
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
pub(super) fn text_face(family: TextFamily, options: &Options<'_>) -> Result<Face, String> {
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
