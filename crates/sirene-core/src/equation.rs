//! The model of a document that is one formula: PlantUML `@startlatex` and `@startmath`
//! (PCREO-3).

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

    /// `src` as text in `style`.
    pub(crate) fn ascii(
        self,
        src: &str,
        style: crate::math::MathStyle,
    ) -> Result<String, crate::math::MathError> {
        match self {
            Flavor::Tex => crate::math::ascii_formula(src, style),
            Flavor::AsciiMath => crate::math::ascii_asciimath(src, style),
        }
    }

    /// `src` as rows of text and the baseline row, as [`Flavor::typeset`] sets it.
    pub(crate) fn ascii_rows(
        self,
        src: &str,
        style: crate::math::MathStyle,
    ) -> Result<(Vec<String>, usize), crate::math::MathError> {
        #[cfg(test)]
        count::bump();
        match self {
            Flavor::Tex => crate::math::ascii_formula_rows(src, style),
            Flavor::AsciiMath => crate::math::ascii_asciimath_rows(src, style),
        }
    }
}

#[cfg(feature = "math")]
impl Equation {
    /// The formula typeset in display style.
    pub(crate) fn typeset(&self) -> Result<crate::math::Formula, crate::math::MathError> {
        self.flavor
            .typeset(&self.source, crate::math::MathStyle::Display)
    }

    /// The formula as text, in display style.
    pub(crate) fn ascii(&self) -> Result<String, crate::math::MathError> {
        self.flavor
            .ascii(&self.source, crate::math::MathStyle::Display)
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
