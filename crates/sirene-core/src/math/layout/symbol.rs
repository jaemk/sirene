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
    let drawn = drawn_text(name, &text, mode);
    let one = |face: Face| {
        Built::One(Atom::new(
            Bx::glyph(drawn.clone(), face, mode, options),
            Some(Class::Ord),
            options,
        ))
    };
    // A mathematical alphanumeric character picks its own font. The table replaces it with its
    // plain letter or digit, which gives the metrics.
    if let Some(face) = text.chars().next().and_then(wide_character_face) {
        return Ok(one(face));
    }
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

/// The face of a mathematical alphanumeric character, U+1D400 to U+1D7FF (KaTeX's
/// `wideCharacterFont`). `None` for the ranges KaTeX has no font for (bold script, bold italic
/// sans serif, double-struck digits, lowercase script, and Greek) and for other characters.
pub(super) fn wide_character_face(c: char) -> Option<Face> {
    let code = u32::from(c);
    match code {
        // Latin letters, 26 per row (KaTeX's `wideLatinLetterData`).
        0x1d400..=0x1d6a3 => match (code - 0x1d400) / 26 {
            0 | 1 => Some(Face::MainBold),
            2 | 3 => Some(Face::MathItalic),
            4 | 5 => Some(Face::MainBoldItalic),
            6 => Some(Face::ScriptRegular),
            10 | 11 | 14 | 15 => Some(Face::FrakturRegular),
            12 | 13 => Some(Face::AmsRegular),
            16 | 17 => Some(Face::SansSerifRegular),
            18 | 19 => Some(Face::SansSerifBold),
            20 | 21 => Some(Face::SansSerifItalic),
            24 | 25 => Some(Face::TypewriterRegular),
            _ => None,
        },
        // Dotless i and j, in the first row (bold upright).
        0x1d6a5 | 0x1d6a6 => Some(Face::MainBold),
        0x1d7ce..=0x1d7ff => match (code - 0x1d7ce) / 10 {
            0 => Some(Face::MainBold),
            2 => Some(Face::SansSerifRegular),
            3 => Some(Face::SansSerifBold),
            4 => Some(Face::TypewriterRegular),
            _ => None,
        },
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{
        ast::{Node, NodeKind},
        boxes::Item,
        layout::layout,
        parser::{Settings, parse_with},
    };

    /// The faces of the glyphs `src` draws, in order.
    fn faces(src: &str) -> Vec<(String, Face)> {
        let nodes = parse_with(src, Settings::default()).unwrap();
        let bx = layout(&nodes, false).unwrap();
        bx.items()
            .into_iter()
            .filter_map(|item| match item {
                Item::Glyph { glyph, .. } => Some((glyph.text.to_string(), glyph.face)),
                _ => None,
            })
            .collect()
    }

    fn error(src: &str) -> String {
        let nodes = parse_with(src, Settings::default()).unwrap();
        match layout(&nodes, false) {
            Ok(_) => "no error".to_string(),
            Err(e) => e.message,
        }
    }

    #[test]
    fn emph_toggles_italic() {
        assert_eq!(
            faces(r"\text{\emph{x}}"),
            [("x".to_string(), Face::MainItalic)]
        );
        // Inside italic text it is upright, and inside that italic again.
        assert_eq!(
            faces(r"\textit{\emph{x}}"),
            [("x".to_string(), Face::MainRegular)]
        );
        assert_eq!(
            faces(r"\textit{\emph{\emph{x}}}"),
            [("x".to_string(), Face::MainItalic)]
        );
    }

    /// The pairs of family, weight and shape KaTeX has no font for fail with its message.
    #[test]
    fn a_text_face_with_no_font_is_an_error() {
        for (src, want) in [
            (r"\texttt{\textbf{x}}", "Typewriter-Bold"),
            (r"\texttt{\textit{x}}", "Typewriter-Italic"),
            (r"\textbf{\textsf{\textit{x}}}", "SansSerif-BoldItalic"),
            (r"\textbf{\checkmark}", "AMS-Bold"),
            (r"\textit{\checkmark}", "AMS-Italic"),
        ] {
            assert_eq!(
                error(src),
                format!("Font metrics not found for font: {want}."),
                "{src}"
            );
        }
    }

    #[test]
    fn every_text_face_kate_x_has_is_found() {
        let base = Options::new(false);
        let cases = [
            (TextFamily::Rm, None, None, Face::MainRegular),
            (TextFamily::Rm, Some(Weight::Bold), None, Face::MainBold),
            (TextFamily::Rm, None, Some(Shape::Italic), Face::MainItalic),
            (
                TextFamily::Rm,
                Some(Weight::Bold),
                Some(Shape::Italic),
                Face::MainBoldItalic,
            ),
            (TextFamily::Sf, None, None, Face::SansSerifRegular),
            (
                TextFamily::Sf,
                Some(Weight::Bold),
                None,
                Face::SansSerifBold,
            ),
            (
                TextFamily::Sf,
                None,
                Some(Shape::Italic),
                Face::SansSerifItalic,
            ),
            (TextFamily::Tt, None, None, Face::TypewriterRegular),
            (TextFamily::Ams, None, None, Face::AmsRegular),
            // Medium weight and upright shape are the regular face.
            (
                TextFamily::Tt,
                Some(Weight::Medium),
                Some(Shape::Upright),
                Face::TypewriterRegular,
            ),
        ];
        for (family, weight, shape, want) in cases {
            let options = Options {
                weight,
                shape,
                ..base
            };
            assert_eq!(text_face(family, &options), Ok(want), "{family:?}");
        }
        let bold_italic = Options {
            weight: Some(Weight::Bold),
            shape: Some(Shape::Italic),
            ..base
        };
        assert_eq!(
            text_face(TextFamily::Ams, &bold_italic),
            Err("Font metrics not found for font: AMS-BoldItalic.".to_string())
        );
    }

    /// An operator or accent reaches layout as its own node; a bare symbol of that group is an
    /// error, not a glyph.
    #[test]
    fn an_operator_or_accent_symbol_is_an_error() {
        for group in [AtomGroup::Op, AtomGroup::Accent] {
            let nodes = [Node {
                kind: NodeKind::Symbol {
                    group,
                    name: Name::Char("x"),
                },
                mode: Mode::Math,
                span: 2..3,
            }];
            let e = layout(&nodes, false).unwrap_err();
            assert_eq!(
                e.message, "an operator or accent symbol is not laid out yet",
                "{group:?}"
            );
            assert_eq!(e.span, 2..3);
        }
    }

    #[test]
    fn wide_characters_pick_their_face() {
        for (code, want) in [
            (0x1d400, Some(Face::MainBold)),
            (0x1d434, Some(Face::MathItalic)),
            (0x1d468, Some(Face::MainBoldItalic)),
            (0x1d49c, Some(Face::ScriptRegular)),
            // Bold script and Greek have no font.
            (0x1d4d0, None),
            (0x1d504, Some(Face::FrakturRegular)),
            (0x1d538, Some(Face::AmsRegular)),
            (0x1d5a0, Some(Face::SansSerifRegular)),
            (0x1d5d4, Some(Face::SansSerifBold)),
            (0x1d608, Some(Face::SansSerifItalic)),
            (0x1d670, Some(Face::TypewriterRegular)),
            (0x1d6a5, Some(Face::MainBold)),
            (0x1d6a8, None),
            (0x1d7ce, Some(Face::MainBold)),
            (0x1d7d8, None),
            (0x1d7e2, Some(Face::SansSerifRegular)),
            (0x1d7ec, Some(Face::SansSerifBold)),
            (0x1d7f6, Some(Face::TypewriterRegular)),
            (u32::from('x'), None),
        ] {
            let c = char::from_u32(code).unwrap();
            assert_eq!(wide_character_face(c), want, "U+{code:X}");
        }
    }
}
