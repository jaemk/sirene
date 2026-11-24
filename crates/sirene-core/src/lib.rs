#![doc = include_str!("../README.md")]
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]

pub mod architecture;
pub mod ascii;
pub mod block;
pub mod c4;
pub mod class;
pub mod cynefin;
pub mod diagnostic;
pub mod diagram;
mod directive;
pub mod dot;
mod edits;
pub mod equation;
pub mod er;
pub mod error;
pub mod eventmodel;
pub mod flowchart;
pub mod gantt;
pub mod git;
pub mod info;
pub mod ishikawa;
pub mod items;
pub mod journey;
pub mod kanban;
pub mod layout;
mod lines;
pub mod markdown;
pub mod math;
pub mod mindmap;
pub mod packet;
pub mod pie;
pub mod plantuml;
pub mod profile;
pub mod quadrant;
pub mod radar;
pub mod railroad;
pub mod requirement;
pub mod sankey;
pub mod scene;
pub mod sequence;
pub mod source;
pub mod state;
pub mod syntax;
pub mod theme;
pub mod timeline;
pub mod timing;
pub mod treemap;
pub mod treeview;
pub mod usecase;
pub mod venn;
pub mod wardley;
pub mod xychart;
pub mod zenuml;

pub use diagnostic::{Diagnostic, Severity};
pub use error::{Error, Result};
pub use rowan::{TextRange, TextSize};
