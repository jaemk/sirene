//! TeX's eight math styles and the moves between them (LMATH-12, `style`).
//!
//! Adapted from KaTeX's `src/Style.ts` (MIT, Copyright (c) 2013-2020 Khan Academy and other
//! contributors): the style of a script, a fraction's parts, and a cramped style.

use super::ast::StyleName;

/// A math style: display, text, script, or scriptscript, each plain or cramped. A cramped style
/// raises superscripts less; it is used under a bar or a radical.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Style {
    Display,
    DisplayCramped,
    Text,
    TextCramped,
    Script,
    ScriptCramped,
    ScriptScript,
    ScriptScriptCramped,
}

use Style::*;

impl Style {
    /// The size class of the style: 0 display, 1 text, 2 script, 3 scriptscript.
    pub(crate) fn size(self) -> u8 {
        match self {
            Display | DisplayCramped => 0,
            Text | TextCramped => 1,
            Script | ScriptCramped => 2,
            ScriptScript | ScriptScriptCramped => 3,
        }
    }

    pub(crate) fn is_cramped(self) -> bool {
        matches!(
            self,
            DisplayCramped | TextCramped | ScriptCramped | ScriptScriptCramped
        )
    }

    /// The style of a superscript on a base in this style.
    pub(crate) fn sup(self) -> Self {
        match self {
            Display | Text => Script,
            DisplayCramped | TextCramped => ScriptCramped,
            Script | ScriptScript => ScriptScript,
            ScriptCramped | ScriptScriptCramped => ScriptScriptCramped,
        }
    }

    /// The style of a subscript on a base in this style.
    pub(crate) fn sub(self) -> Self {
        match self.size() {
            0 | 1 => ScriptCramped,
            _ => ScriptScriptCramped,
        }
    }

    /// The style of a fraction's numerator, the fraction being in this style.
    pub(crate) fn frac_num(self) -> Self {
        match self {
            Display => Text,
            DisplayCramped => TextCramped,
            Text => Script,
            TextCramped => ScriptCramped,
            Script | ScriptScript => ScriptScript,
            ScriptCramped | ScriptScriptCramped => ScriptScriptCramped,
        }
    }

    /// The style of a fraction's denominator, the fraction being in this style.
    pub(crate) fn frac_den(self) -> Self {
        self.frac_num().cramp()
    }

    /// The cramped form of this style.
    pub(crate) fn cramp(self) -> Self {
        match self {
            Display | DisplayCramped => DisplayCramped,
            Text | TextCramped => TextCramped,
            Script | ScriptCramped => ScriptCramped,
            ScriptScript | ScriptScriptCramped => ScriptScriptCramped,
        }
    }

    /// Display stays display, every other style becomes text; crampedness is kept.
    pub(crate) fn text(self) -> Self {
        match self {
            Display => Display,
            DisplayCramped => DisplayCramped,
            Text | Script | ScriptScript => Text,
            TextCramped | ScriptCramped | ScriptScriptCramped => TextCramped,
        }
    }

    /// Script and scriptscript, which are spaced tightly.
    pub(crate) fn is_tight(self) -> bool {
        self.size() >= 2
    }
}

impl From<StyleName> for Style {
    fn from(name: StyleName) -> Self {
        match name {
            StyleName::Display => Display,
            StyleName::Text => Text,
            StyleName::Script => Script,
            StyleName::ScriptScript => ScriptScript,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Style; 8] = [
        Display,
        DisplayCramped,
        Text,
        TextCramped,
        Script,
        ScriptCramped,
        ScriptScript,
        ScriptScriptCramped,
    ];

    /// KaTeX's lookup tables, indexed by its style ids (the order of `ALL`).
    fn katex(table: [usize; 8]) -> Vec<Style> {
        table.iter().map(|&i| ALL[i]).collect()
    }

    fn each(f: fn(Style) -> Style) -> Vec<Style> {
        ALL.iter().map(|&s| f(s)).collect()
    }

    #[test]
    fn moves_match_katex_tables() {
        assert_eq!(each(Style::sup), katex([4, 5, 4, 5, 6, 7, 6, 7]));
        assert_eq!(each(Style::sub), katex([5, 5, 5, 5, 7, 7, 7, 7]));
        assert_eq!(each(Style::frac_num), katex([2, 3, 4, 5, 6, 7, 6, 7]));
        assert_eq!(each(Style::frac_den), katex([3, 3, 5, 5, 7, 7, 7, 7]));
        assert_eq!(each(Style::cramp), katex([1, 1, 3, 3, 5, 5, 7, 7]));
        assert_eq!(each(Style::text), katex([0, 1, 2, 3, 2, 3, 2, 3]));
    }

    #[test]
    fn sizes_and_tightness() {
        let sizes: Vec<u8> = ALL.iter().map(|s| s.size()).collect();
        assert_eq!(sizes, [0, 0, 1, 1, 2, 2, 3, 3]);
        let cramped: Vec<bool> = ALL.iter().map(|s| s.is_cramped()).collect();
        assert_eq!(
            cramped,
            [false, true, false, true, false, true, false, true]
        );
        assert!(!Text.is_tight() && Script.is_tight() && ScriptScriptCramped.is_tight());
        assert_eq!(Style::from(StyleName::ScriptScript), ScriptScript);
    }
}
