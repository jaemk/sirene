//! Nodes that change a box's size or place without new shapes: phantoms, `\smash`, the laps,
//! `\raisebox`, `\vcenter`, `\underline`, `\overline`, `\pmb`, `\reflectbox`, `\mathchoice`,
//! `\html@mathml`, `\hbox`, and `\verb`.
//!
//! To build from the HTML builders of KaTeX's `src/functions/phantom.ts`, `smash.ts`, `lap.ts`,
//! `raisebox.ts`, `vcenter.ts`, `underline.ts`, `overline.ts`, `pmb.ts`, `reflectbox.ts`,
//! `mathchoice.ts`, `htmlmathml.ts`, `hbox.ts`, and `verb.ts` (MIT, Copyright (c) 2013-2020 Khan
//! Academy and other contributors). `Bx::shadowed` and `Bx::mirrored` draw `\pmb` and
//! `\reflectbox`.

use std::ops::Range;

use super::{Built, not_yet};
use crate::math::{
    MathError,
    ast::{MathClass, Measurement, Node},
    options::Options,
    symbols::Mode,
};

pub(super) fn phantom<'t>(
    body: &'t [Node<'_>],
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (body, options);
    Err(not_yet("phantom", span))
}

pub(super) fn vphantom<'t>(
    body: &'t Node<'_>,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (body, options);
    Err(not_yet("vphantom", span))
}

/// `\smash`: `height` and `depth` say which of the two are set to zero.
pub(super) fn smash<'t>(
    body: &'t Node<'_>,
    (height, depth): (bool, bool),
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (body, height, depth, options);
    Err(not_yet("smash", span))
}

/// `\mathllap`, `\mathrlap`, `\mathclap` (and the text forms): `alignment` is `llap`, `rlap`,
/// or `clap`.
pub(super) fn lap<'t>(
    alignment: &'t str,
    body: &'t Node<'_>,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (alignment, body, options);
    Err(not_yet("lap", span))
}

pub(super) fn raise_box<'t>(
    dy: &'t Measurement,
    body: &'t Node<'_>,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (dy, body, options);
    Err(not_yet("raisebox", span))
}

pub(super) fn vcenter<'t>(
    body: &'t Node<'_>,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (body, options);
    Err(not_yet("vcenter", span))
}

pub(super) fn underline<'t>(
    body: &'t Node<'_>,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (body, options);
    Err(not_yet("underline", span))
}

pub(super) fn overline<'t>(
    body: &'t Node<'_>,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (body, options);
    Err(not_yet("overline", span))
}

pub(super) fn pmb<'t>(
    class: MathClass,
    body: &'t [Node<'_>],
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (class, body, options);
    Err(not_yet("pmb", span))
}

pub(super) fn reflect_box<'t>(
    body: &'t Node<'_>,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (body, options);
    Err(not_yet("reflectbox", span))
}

/// `\mathchoice`: the four bodies in display, text, script, and scriptscript order.
pub(super) fn math_choice<'t>(
    bodies: [&'t [Node<'_>]; 4],
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (bodies, options);
    Err(not_yet("mathchoice", span))
}

/// `\html@mathml`: the HTML branch is drawn.
pub(super) fn html_mathml<'t>(
    html: &'t [Node<'_>],
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (html, options);
    Err(not_yet("htmlmathml", span))
}

pub(super) fn hbox<'t>(
    body: &'t [Node<'_>],
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (body, options);
    Err(not_yet("hbox", span))
}

pub(super) fn verb<'t>(
    body: &'t str,
    star: bool,
    mode: Mode,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (body, star, mode, options);
    Err(not_yet("verb", span))
}
