//! The LaTeX math typesetter (LMATH, `spec/latex-math.md`).
//!
//! Source text goes through the lexer, a macro expander, and a parser to a tree, then through
//! layout to a box tree that every output reads (LMATH-2). This module holds the stages built so
//! far; the rest follow the order of LMATH-9. The public entry point is `typeset`, which gives a
//! `Formula` that draws itself as scene primitives (LEMB-4).
//!
//! Behavior follows KaTeX (LMATH-11), whose test cases are the baseline (LMATH-13). The design is
//! Rust's: values and `Result`s with byte ranges in the source, no global registries, no panics.
// `typeset`, which uses every stage, needs the fonts; without the `math` feature the stages are
// built (and tested) but nothing calls them.
#![cfg_attr(
    not(feature = "math"),
    allow(dead_code, reason = "the entry point is behind the `math` feature")
)]

#[cfg(feature = "math")]
mod ascii;
mod asciimath;
mod ast;
#[cfg(test)]
mod baseline;
mod boxes;
#[cfg(feature = "math")]
mod draw;
mod error;
mod expander;
#[cfg(feature = "math")]
mod fonts;
#[cfg(feature = "math")]
mod formula;
mod functions;
#[rustfmt::skip]
mod functions_data;
mod layout;
mod lexer;
mod limits;
#[rustfmt::skip]
mod macros_data;
mod metrics;
#[rustfmt::skip]
mod metrics_data;
mod options;
mod parser;
mod style;
#[rustfmt::skip]
mod svg_data;
mod svg_paths;
mod symbols;
#[rustfmt::skip]
mod symbols_data;
#[rustfmt::skip]
mod unicode_data;

#[cfg(feature = "math")]
pub use ascii::{ascii_formula, ascii_formula_rows};
pub use asciimath::{Translation, to_tex, translate};
#[cfg(feature = "math")]
pub use asciimath::{ascii_asciimath, ascii_asciimath_rows, typeset_asciimath};
pub use error::MathError;
#[cfg(feature = "math")]
pub use formula::{Formula, MathStyle, typeset};
