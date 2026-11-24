//! Accents over and under a base: `\hat{x}`, `\widehat{xy}`, `\vec`, text accents, and
//! `\underrightarrow` and its kind.
//!
//! To build from KaTeX's `src/functions/accent.ts` and `src/functions/accentunder.ts` (MIT,
//! Copyright (c) 2013-2020 Khan Academy and other contributors), with `stretchy` for the wide
//! forms.

use std::ops::Range;

use super::{Built, not_yet};
use crate::math::{MathError, ast::Node, options::Options, symbols::Mode};

/// An accent: what `NodeKind::Accent` holds, and where it is.
pub(super) struct Accent<'t, 'n> {
    pub label: &'t str,
    pub stretchy: bool,
    pub shifty: bool,
    pub base: &'t Node<'n>,
    pub mode: Mode,
    pub span: Range<usize>,
}

pub(super) fn accent<'t>(
    accent: Accent<'t, '_>,
    options: &Options<'t>,
) -> Result<Built<'t>, MathError> {
    let _ = (
        accent.label,
        accent.stretchy,
        accent.shifty,
        accent.base,
        accent.mode,
        options,
    );
    Err(not_yet("accent", accent.span))
}

pub(super) fn accent_under<'t>(
    label: &'t str,
    base: &'t Node<'_>,
    mode: Mode,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (label, base, mode, options);
    Err(not_yet("accentUnder", span))
}

/// Scripts on an accented base whose base is one character, which KaTeX's accent builder
/// places itself (`htmlBuilderDelegate`); `None` for any other base.
pub(super) fn supsub<'t>(
    base: &'t Node<'_>,
    sup: Option<&'t Node<'_>>,
    sub: Option<&'t Node<'_>>,
    options: &Options<'t>,
) -> Result<Option<Built<'t>>, MathError> {
    let _ = (base, sup, sub, options);
    Ok(None)
}
