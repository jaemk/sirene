#![doc = include_str!("../README.md")]

pub mod ascii;
pub mod class;
pub mod diagnostic;
pub mod diagram;
pub mod er;
pub mod flowchart;
pub mod gantt;
pub mod git;
pub mod layout;
mod lines;
pub mod markdown;
pub mod mindmap;
pub mod pie;
pub mod scene;
pub mod sequence;
pub mod state;
pub mod syntax;
pub mod timeline;

pub use diagnostic::{Diagnostic, Severity};
pub use rowan::{TextRange, TextSize};
