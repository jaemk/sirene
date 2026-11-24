//! Braces and brackets over or under a base: `\overbrace`, `\underbrace`, `\overbracket`,
//! `\underbracket`.
//!
//! To build from KaTeX's `src/functions/horizBrace.ts` (MIT, Copyright (c) 2013-2020 Khan
//! Academy and other contributors), with `stretchy` for the brace shapes.

use std::ops::Range;

use super::{Built, not_yet};
use crate::math::{MathError, ast::Node, options::Options};

pub(super) fn horiz_brace<'t>(
    label: &'t str,
    is_over: bool,
    base: &'t Node<'_>,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (label, is_over, base, options);
    Err(not_yet("horizBrace", span))
}

/// A brace's label: a superscript on an over brace or a subscript on an under brace, which
/// KaTeX's brace builder places itself (`htmlBuilderDelegate`); `None` for any other base.
pub(super) fn supsub<'t>(
    base: &'t Node<'_>,
    sup: Option<&'t Node<'_>>,
    sub: Option<&'t Node<'_>>,
    options: &Options<'t>,
) -> Result<Option<Built<'t>>, MathError> {
    let _ = (base, sup, sub, options);
    Ok(None)
}
