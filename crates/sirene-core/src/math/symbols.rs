//! The symbol table: every command and character that stands for one glyph (LRES-3).
//!
//! The data is KaTeX's own table, generated into `symbols_data.rs` by `scripts/katex-symbols.mjs`.
//! Each entry gives the atom group TeX spaces it by, the font face family its glyph comes from,
//! and the text to draw, which is a Unicode scalar for a symbol and the plain letter for a
//! mathematical alphanumeric character (the font style is picked at layout).

use super::symbols_data::{MATH_CHARS, MATH_CONTROLS, TEXT_CHARS, TEXT_CONTROLS};

/// Math mode or text mode, which have different symbols (`\alpha` is math only).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Math,
    Text,
}

/// The family of KaTeX fonts a glyph comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Font {
    /// The main, math italic, and size fonts.
    Main,
    /// The AMS symbol fonts.
    Ams,
}

/// TeX's atom classes, which decide the space around a symbol (spacing is its own class).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AtomGroup {
    Accent,
    Bin,
    Close,
    Inner,
    MathOrd,
    Op,
    Open,
    Punct,
    Rel,
    Spacing,
    TextOrd,
}

/// A row of the generated tables: key, font, group, and the text to draw.
pub(super) type Entry = (&'static str, Font, AtomGroup, &'static str);

/// What the table knows about a symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Symbol {
    pub font: Font,
    pub group: AtomGroup,
    /// The text to draw.
    pub replace: &'static str,
}

/// A symbol's name as it appears in the source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Name<'a> {
    /// A character, with any combining marks, as `TokenKind::Char` holds it.
    Char(&'a str),
    /// A control word or symbol without its backslash (`alpha`, `,`).
    Control(&'a str),
}

/// The symbol called `name` in `mode`, if there is one.
pub(crate) fn lookup(mode: Mode, name: Name<'_>) -> Option<Symbol> {
    let (table, key) = match (mode, name) {
        (Mode::Math, Name::Char(key)) => (MATH_CHARS, key),
        (Mode::Math, Name::Control(key)) => (MATH_CONTROLS, key),
        (Mode::Text, Name::Char(key)) => (TEXT_CHARS, key),
        (Mode::Text, Name::Control(key)) => (TEXT_CONTROLS, key),
    };
    let at = table.binary_search_by(|entry| entry.0.cmp(key)).ok()?;
    let (_, font, group, replace) = table[at];
    Some(Symbol {
        font,
        group,
        replace,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use AtomGroup::*;
    use Font::*;

    #[test]
    fn the_tables_are_sorted_with_unique_keys() {
        for (name, table) in [
            ("math characters", MATH_CHARS),
            ("math controls", MATH_CONTROLS),
            ("text characters", TEXT_CHARS),
            ("text controls", TEXT_CONTROLS),
        ] {
            assert!(
                table.windows(2).all(|w| w[0].0 < w[1].0),
                "{name} is not strictly sorted"
            );
        }
    }

    #[test]
    fn the_tables_have_the_size_of_katex_s() {
        // As generated from KaTeX 0.19.1; a change here is a regeneration.
        assert_eq!(MATH_CHARS.len(), 998);
        assert_eq!(MATH_CONTROLS.len(), 490);
        assert_eq!(TEXT_CHARS.len(), 682);
        assert_eq!(TEXT_CONTROLS.len(), 68);
    }

    #[test]
    fn lookups() {
        let symbol = |mode, name| lookup(mode, name).unwrap();
        assert_eq!(
            symbol(Mode::Math, Name::Control("alpha")),
            Symbol {
                font: Main,
                group: MathOrd,
                replace: "\u{3b1}"
            }
        );
        assert_eq!(symbol(Mode::Math, Name::Control("equiv")).group, Rel);
        assert_eq!(symbol(Mode::Math, Name::Control("pm")).group, Bin);
        assert_eq!(symbol(Mode::Math, Name::Control("nless")).font, Ams);
        assert_eq!(symbol(Mode::Math, Name::Char("a")).group, MathOrd);
        assert_eq!(symbol(Mode::Math, Name::Char("+")).group, Bin);
        assert_eq!(symbol(Mode::Math, Name::Char("(")).group, Open);
        assert_eq!(symbol(Mode::Math, Name::Char(",")).group, Punct);
        assert_eq!(symbol(Mode::Text, Name::Char("a")).group, TextOrd);
        // A control symbol is keyed by its character.
        assert_eq!(symbol(Mode::Math, Name::Control("#")).replace, "#");
    }

    #[test]
    fn modes_and_namespaces_are_separate() {
        // `\alpha` is not a text symbol, and the letter `a` is not a control word.
        assert_eq!(lookup(Mode::Text, Name::Control("alpha")), None);
        assert_eq!(lookup(Mode::Math, Name::Control("a")), None);
        assert_eq!(lookup(Mode::Math, Name::Char("alpha")), None);
        assert_eq!(lookup(Mode::Math, Name::Control("notacommand")), None);
        assert_eq!(lookup(Mode::Math, Name::Char("")), None);
    }

    #[test]
    fn mathematical_alphanumerics_read_as_their_letters() {
        // U+1D41A is a bold small a; it is the letter `a`, drawn in a bold style by layout.
        let bold_a = lookup(Mode::Math, Name::Char("\u{1d41a}")).unwrap();
        assert_eq!(bold_a.group, MathOrd);
        assert_eq!(bold_a.replace, "a");
    }
}
