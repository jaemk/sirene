//! Tables and line breaks: `array`, the matrices, `cases`, the aligned environments, `\\`
//! outside a table, and equation tags.
//!
//! To build from the HTML builder of KaTeX's `src/environments/array.ts`, `src/functions/cr.ts`,
//! and the tag handling of `src/buildHTML.ts` (MIT, Copyright (c) 2013-2020 Khan Academy and
//! other contributors).

use std::ops::Range;

use super::{Built, not_yet};
use crate::math::{
    MathError,
    ast::{Array, Measurement, Node},
    options::Options,
    symbols::Mode,
};

pub(super) fn array<'t>(
    array: &'t Array<'_>,
    mode: Mode,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (array, mode, options);
    Err(not_yet("array", span))
}

/// `\\` outside a table: a line break, with `size` of extra space below it.
pub(super) fn cr<'t>(
    new_line: bool,
    size: Option<&'t Measurement>,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (new_line, size, options);
    Err(not_yet("cr", span))
}

/// A display formula with an equation tag at its right.
pub(super) fn tag<'t>(
    body: &'t [Node<'_>],
    tag: &'t [Node<'_>],
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (body, tag, options);
    Err(not_yet("tag", span))
}
