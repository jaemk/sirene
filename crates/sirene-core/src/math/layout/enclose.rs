//! Enclosures: `\fbox`, `\colorbox`, `\fcolorbox`, `\boxed`, `\cancel`, `\bcancel`,
//! `\xcancel`, `\sout`, `\angl`, and `\phase`.
//!
//! To build from KaTeX's `src/functions/enclose.ts` (MIT, Copyright (c) 2013-2020 Khan Academy
//! and other contributors), with `stretchy::encloseSpan` for the lines and backgrounds.

use std::ops::Range;

use super::{Built, not_yet};
use crate::math::{MathError, ast::Node, options::Options, symbols::Mode};

/// An enclosure: what `NodeKind::Enclose` holds, and where it is.
pub(super) struct Enclosure<'t, 'n> {
    pub label: &'t str,
    pub background_color: Option<&'t str>,
    pub border_color: Option<&'t str>,
    pub body: &'t Node<'n>,
    pub mode: Mode,
    pub span: Range<usize>,
}

pub(super) fn enclose<'t>(
    enclosure: Enclosure<'t, '_>,
    options: &Options<'t>,
) -> Result<Built<'t>, MathError> {
    let _ = (
        enclosure.label,
        enclosure.background_color,
        enclosure.border_color,
        enclosure.body,
        enclosure.mode,
        options,
    );
    Err(not_yet("enclose", enclosure.span))
}
