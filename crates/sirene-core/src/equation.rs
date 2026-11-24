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

#[cfg(feature = "math")]
impl Equation {
    /// The formula typeset in display style.
    pub(crate) fn typeset(&self) -> Result<crate::math::Formula, crate::math::MathError> {
        use crate::math::{MathStyle, typeset, typeset_asciimath};
        match self.flavor {
            Flavor::Tex => typeset(&self.source, MathStyle::Display),
            Flavor::AsciiMath => typeset_asciimath(&self.source, MathStyle::Display),
        }
    }

    /// The formula as text, in display style.
    pub(crate) fn ascii(&self) -> Result<String, crate::math::MathError> {
        use crate::math::{MathStyle, ascii_asciimath, ascii_formula};
        match self.flavor {
            Flavor::Tex => ascii_formula(&self.source, MathStyle::Display),
            Flavor::AsciiMath => ascii_asciimath(&self.source, MathStyle::Display),
        }
    }
}
