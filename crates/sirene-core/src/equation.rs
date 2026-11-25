//! The model of a document that is one formula: PlantUML `@startlatex` and `@startmath`
//! (PCREO-3), a TeX file, and a formula in Markdown (LEMB-3).

use crate::diagnostic::Diagnostic;

mod tex;

/// Whether this build typesets formulas (the `math` feature); without it a formula is drawn as
/// its source with one warning.
pub const MATH: bool = cfg!(feature = "math");

/// The language an equation is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Flavor {
    /// TeX math mode (`@startlatex`).
    #[default]
    Tex,
    /// AsciiMath (`@startmath`), read by translating it to TeX (PCREO-2).
    AsciiMath,
}

/// One formula, set in display style.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Equation {
    /// The document's body: its lines trimmed and joined with a space each.
    pub source: String,
    /// The language `source` is in.
    pub flavor: Flavor,
    /// Set in text style (a Markdown `$...$` formula) rather than display style.
    pub inline: bool,
}

impl Equation {
    /// The formula of a TeX document (LEMB-3): a bare formula, one wrapped in `$$`, `$`, `\[`,
    /// `\(`, or a `displaymath` or `math` environment, or the one math block of a
    /// `\begin{document}`. `source` is the formula without its wrapper. The diagnostics' ranges
    /// are in `src`, and a full document is refused with one error on its first line.
    pub fn tex(src: &str) -> (Equation, Vec<Diagnostic>) {
        tex::lower(src)
    }

    /// A formula `src` as written, in text style when `inline`. The diagnostics' ranges are in
    /// `src`.
    pub fn formula(src: &str, inline: bool) -> (Equation, Vec<Diagnostic>) {
        tex::bare(src, inline)
    }
}

/// The one place a formula is read in its language: every consumer (labels, equations, ascii)
/// goes through these.
#[cfg(feature = "math")]
impl Flavor {
    /// `src` typeset in `style`.
    pub(crate) fn typeset(
        self,
        src: &str,
        style: crate::math::MathStyle,
    ) -> Result<crate::math::Formula, crate::math::MathError> {
        #[cfg(test)]
        count::bump();
        match self {
            Flavor::Tex => crate::math::typeset(src, style),
            Flavor::AsciiMath => crate::math::typeset_asciimath(src, style),
        }
    }

    /// `src` as text in `style`, drawn with `charset`.
    pub(crate) fn ascii(
        self,
        src: &str,
        style: crate::math::MathStyle,
        charset: crate::ascii::Charset,
    ) -> Result<String, crate::math::MathError> {
        match self {
            Flavor::Tex => crate::math::ascii_formula(src, style, charset),
            Flavor::AsciiMath => crate::math::ascii_asciimath(src, style, charset),
        }
    }

    /// `src` as rows of text and the baseline row, as [`Flavor::typeset`] sets it.
    pub(crate) fn ascii_rows(
        self,
        src: &str,
        style: crate::math::MathStyle,
        charset: crate::ascii::Charset,
    ) -> Result<(Vec<String>, usize), crate::math::MathError> {
        #[cfg(test)]
        count::bump();
        match self {
            Flavor::Tex => crate::math::ascii_formula_rows(src, style, charset),
            Flavor::AsciiMath => crate::math::ascii_asciimath_rows(src, style, charset),
        }
    }
}

#[cfg(feature = "math")]
impl Equation {
    fn style(&self) -> crate::math::MathStyle {
        if self.inline {
            crate::math::MathStyle::Text
        } else {
            crate::math::MathStyle::Display
        }
    }

    /// The formula typeset in its style.
    pub(crate) fn typeset(&self) -> Result<crate::math::Formula, crate::math::MathError> {
        self.flavor.typeset(&self.source, self.style())
    }

    /// The formula as text, in its style, drawn with `charset`.
    pub(crate) fn ascii(
        &self,
        charset: crate::ascii::Charset,
    ) -> Result<String, crate::math::MathError> {
        self.flavor.ascii(&self.source, self.style(), charset)
    }
}

/// How many times a formula was typeset on this thread, for tests that bound the work a render
/// does.
#[cfg(all(test, feature = "math"))]
pub(crate) mod count {
    use std::cell::Cell;

    thread_local! {
        static TYPESET: Cell<usize> = const { Cell::new(0) };
    }

    pub(crate) fn bump() {
        TYPESET.with(|c| c.set(c.get() + 1));
    }

    pub(crate) fn reset() {
        TYPESET.with(|c| c.set(0));
    }

    pub(crate) fn get() -> usize {
        TYPESET.with(Cell::get)
    }
}
