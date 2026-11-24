//! Delimiters: the null delimiter, `\big` and its sizes, `\left`...`\right` with `\middle`, and a
//! fraction's delimiters.
//!
//! To build from KaTeX's `src/delimiter.ts` (`makeSmallDelim`, `makeLargeDelim`,
//! `makeStackedDelim`, `sizedDelim`, `customSizedDelim`, `leftRightDelim`),
//! `src/functions/delimsizing.ts`, and the delimiter part of `src/functions/genfrac.ts` (MIT,
//! Copyright (c) 2013-2020 Khan Academy and other contributors).

use std::ops::Range;

use super::{Built, not_yet};
use crate::math::{
    MathError,
    ast::{MathClass, Node},
    boxes::Bx,
    options::{Em, Options},
    symbols::Mode,
};

/// The empty space where a delimiter is left out: `\nulldelimiterspace`, 1.2pt of the base
/// size (KaTeX's `makeNullDelimiter`).
pub(super) fn null_delimiter<'t>(options: &Options<'t>) -> Bx<'t> {
    Bx::kern(Em(0.12 / options.size_multiplier()))
}

/// The delimiter on one side of a fraction (`\binom`, `\genfrac`): `None` is a null delimiter.
pub(super) fn frac_delimiter<'t>(
    delim: Option<&'t str>,
    mode: Mode,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Bx<'t>, MathError> {
    let _ = mode;
    match delim {
        None => Ok(null_delimiter(options)),
        Some(_) => Err(not_yet("a fraction with delimiters", span)),
    }
}

/// `\big(` and the other fixed sizes: `size` is 1 to 4, spaced as `class`.
pub(super) fn delim_sizing<'t>(
    size: u8,
    class: MathClass,
    delim: &'t str,
    mode: Mode,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (size, class, delim, mode, options);
    Err(not_yet("delimsizing", span))
}

/// `\left ... \right` around `body`, with any `\middle` delimiters in it grown to match.
pub(super) fn left_right<'t>(
    body: &'t [Node<'_>],
    (left, right): (&'t str, &'t str),
    right_color: Option<&'t str>,
    mode: Mode,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (body, left, right, right_color, mode, options);
    Err(not_yet("leftright", span))
}

/// `\middle|` on its own; inside `\left ... \right`, `left_right` sizes it.
pub(super) fn middle<'t>(
    delim: &'t str,
    mode: Mode,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (delim, mode, options);
    Err(not_yet("middle", span))
}
