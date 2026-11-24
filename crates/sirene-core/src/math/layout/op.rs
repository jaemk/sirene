//! Large and named operators: `\sum`, `\int`, `\lim`, `\mathop{...}`, `\operatorname{...}`, and
//! their limits.
//!
//! To build from KaTeX's `src/functions/op.ts`, `src/functions/operatorname.ts`, and
//! `src/functions/utils/assembleSupSub.ts` (MIT, Copyright (c) 2013-2020 Khan Academy and other
//! contributors).

use std::ops::Range;

use super::{Built, not_yet};
use crate::math::{
    MathError,
    ast::{Node, OpContent},
    options::Options,
    symbols::Mode,
};

/// An operator: what `NodeKind::Op` holds, and where it is.
pub(super) struct Operator<'t, 'n> {
    pub limits: bool,
    pub symbol: bool,
    pub suppress_base_shift: Option<bool>,
    pub content: &'t OpContent<'n>,
    pub mode: Mode,
    pub span: Range<usize>,
}

pub(super) fn op<'t>(op: Operator<'t, '_>, options: &Options<'t>) -> Result<Built<'t>, MathError> {
    let _ = (
        op.limits,
        op.symbol,
        op.suppress_base_shift,
        op.content,
        op.mode,
        options,
    );
    Err(not_yet("op", op.span))
}

pub(super) fn operator_name<'t>(
    body: &'t [Node<'_>],
    mode: Mode,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Built<'t>, MathError> {
    let _ = (body, mode, options);
    Err(not_yet("operatorname", span))
}

/// Scripts on an operator base that takes them as limits (an `Op` with `limits` in display
/// style or `always_handle_supsub`, or an `OperatorName` likewise), as KaTeX's
/// `htmlBuilderDelegate`; `None` for any other base, which `scripts::supsub` lays out.
pub(super) fn supsub<'t>(
    base: &'t Node<'_>,
    sup: Option<&'t Node<'_>>,
    sub: Option<&'t Node<'_>>,
    options: &Options<'t>,
) -> Result<Option<Built<'t>>, MathError> {
    let _ = (base, sup, sub, options);
    Ok(None)
}
