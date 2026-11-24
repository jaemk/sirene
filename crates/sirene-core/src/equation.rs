//! The model of a document that is one formula: PlantUML `@startlatex` (PCREO-3).

/// One formula in TeX math mode, set in display style.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Equation {
    /// The document's body: its lines trimmed and joined with a space each.
    pub source: String,
}
