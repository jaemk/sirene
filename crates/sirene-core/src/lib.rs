//! Mermaid parsing, diagram model, and layout for sirene.

pub mod ascii;
pub mod diagnostic;
pub mod flowchart;
pub mod layout;
pub mod syntax;

pub use diagnostic::{Diagnostic, Severity};
pub use rowan::{TextRange, TextSize};
