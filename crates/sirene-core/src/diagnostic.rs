//! Parse and validation diagnostics.

use rowan::TextRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Mermaid would fail to render the diagram.
    Error,
    /// Mermaid would ignore or guess; the diagram still renders.
    Warning,
}

/// A problem found in the source, located by byte range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub range: TextRange,
    pub message: String,
    pub severity: Severity,
}

impl Diagnostic {
    /// An error diagnostic.
    pub fn new(range: TextRange, message: impl Into<String>) -> Self {
        Self {
            range,
            message: message.into(),
            severity: Severity::Error,
        }
    }

    pub fn warning(range: TextRange, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            ..Self::new(range, message)
        }
    }
}
