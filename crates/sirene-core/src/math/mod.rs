//! The LaTeX math typesetter (LMATH, `spec/latex-math.md`).
//!
//! Source text goes through the lexer, a macro expander, and a parser to a tree, then through
//! layout to a box tree that every output reads (LMATH-2). This module holds the stages built so
//! far; the rest follow the order of LMATH-9. Nothing here is public until the typesetter draws
//! a formula, so the items are used by tests only for now.
//!
//! Behavior follows KaTeX (LMATH-11), whose test cases are the baseline (LMATH-13). The design is
//! Rust's: values and `Result`s with byte ranges in the source, no global registries, no panics.
#![allow(dead_code)]

mod ast;
#[cfg(test)]
mod baseline;
mod boxes;
mod error;
mod expander;
#[cfg(feature = "math")]
mod fonts;
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

pub(crate) use error::MathError;
