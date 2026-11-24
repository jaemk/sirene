//! Extensible arrows: `\xrightarrow[below]{above}` and the other `\x...` arrows.
//!
//! To build from KaTeX's `src/functions/arrow.ts` (MIT, Copyright (c) 2013-2020 Khan Academy
//! and other contributors), with `stretchy` for the arrow shapes.

use std::ops::Range;

use super::{Built, not_yet};
use crate::math::{MathError, ast::Node, options::Options};

pub(super) fn x_arrow<'t>(
    label: &'t str,
    body: &'t Node<'_>,
    below: Option<&'t Node<'_>>,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (label, body, below, options);
    Err(not_yet("xArrow", span))
}
