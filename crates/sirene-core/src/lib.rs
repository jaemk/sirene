#![doc = include_str!("../README.md")]

pub mod ascii;
pub mod diagnostic;
pub mod flowchart;
pub mod layout;
pub mod scene;
pub mod syntax;

pub use diagnostic::{Diagnostic, Severity};
pub use rowan::{TextRange, TextSize};
