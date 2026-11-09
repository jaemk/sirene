#![doc = include_str!("../README.md")]

pub mod ascii;
pub mod class;
pub mod diagnostic;
pub mod diagram;
pub mod er;
pub mod flowchart;
pub mod layout;
mod lines;
pub mod markdown;
pub mod scene;
pub mod state;
pub mod syntax;

pub use diagnostic::{Diagnostic, Severity};
pub use rowan::{TextRange, TextSize};
