//! Roots: `\sqrt{x}` and `\sqrt[n]{x}`.
//!
//! To build from KaTeX's `src/functions/sqrt.ts` and `makeSqrtImage` in `src/delimiter.ts`
//! (MIT, Copyright (c) 2013-2020 Khan Academy and other contributors), with the radical paths
//! of `math::svg_paths`.

use std::ops::Range;

use super::{Built, not_yet};
use crate::math::{MathError, ast::Node, options::Options};

pub(super) fn sqrt<'t>(
    body: &'t Node<'_>,
    index: Option<&'t Node<'_>>,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (body, index, options);
    Err(not_yet("sqrt", span))
}
