#![doc = include_str!("../README.md")]

pub mod architecture;
pub mod ascii;
pub mod block;
pub mod c4;
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
pub mod radar;
pub mod requirement;
pub mod sankey;
pub mod scene;
pub mod sequence;
pub mod state;
pub mod syntax;
pub mod timeline;
pub mod treemap;
pub mod xychart;
pub mod zenuml;

pub use diagnostic::{Diagnostic, Severity};
pub use rowan::{TextRange, TextSize};
