#![doc = include_str!("../README.md")]

pub mod ascii;
pub mod class;
pub mod diagnostic;
pub mod diagram;
pub mod er;
pub mod flowchart;
pub mod gantt;
pub mod git;
pub mod journey;
pub mod kanban;
pub mod layout;
mod lines;
pub mod markdown;
pub mod mindmap;
pub mod packet;
pub mod pie;
pub mod quadrant;
pub mod requirement;
pub mod scene;
pub mod sequence;
pub mod state;
pub mod syntax;
pub mod timeline;
pub mod xychart;

pub use diagnostic::{Diagnostic, Severity};
pub use rowan::{TextRange, TextSize};
