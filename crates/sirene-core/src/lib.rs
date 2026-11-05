//! Mermaid parsing, diagram model, and layout for sirene.

pub mod diagnostic;
pub mod flowchart;
pub mod syntax;

pub use diagnostic::{Diagnostic, Severity};
pub use rowan::{TextRange, TextSize};
