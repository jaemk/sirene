//! Superscripts and subscripts, by the TeXbook's rules 18a to 18f.
//!
//! Adapted from KaTeX's `src/functions/supsub.ts` (MIT, Copyright (c) 2013-2020 Khan Academy
//! and other contributors). A base that places its own scripts (an operator with limits, an
//! accent over one character, a brace) is laid out by its own module, as KaTeX's
//! `htmlBuilderDelegate` does.

use super::{Atom, Built, Class, accent, build, build_in, horiz_brace, op};
use crate::math::{
    MathError,
    ast::Node,
    boxes::{BoxKind, Bx, Part, Position},
    options::{Em, Options},
    style::Style,
};

/// A box with space after it (KaTeX's `marginRight` on a stack's child).
pub(super) fn with_space_after(bx: Bx<'_>, space: Em) -> Bx<'_> {
    Bx::hlist(vec![bx, Bx::kern(space)])
}

/// A base with a superscript, a subscript, or both.
pub(super) fn supsub<'t>(
    base: Option<&'t Node<'_>>,
    sup: Option<&'t Node<'_>>,
    sub: Option<&'t Node<'_>>,
    options: &Options<'t>,
) -> Result<Built<'t>, MathError> {
    if let Some(base) = base {
        let delegated = match op::supsub(base, sup, sub, options)? {
            Some(built) => Some(built),
            None => match accent::supsub(base, sup, sub, options)? {
                Some(built) => Some(built),
                None => horiz_brace::supsub(base, sup, sub, options)?,
            },
        };
        if let Some(built) = delegated {
            return Ok(built);
        }
    }
    let built = match base {
        Some(base) => build(base, options)?,
        None => Built::One(Atom::new(Bx::hlist(Vec::new()), None, options)),
    };
    let class = built.right_class().unwrap_or(Class::Ord);
    // A subscript does not move right by a lone symbol's italic correction.
    let italic = match &built {
        Built::One(Atom {
            bx: Bx {
                kind: BoxKind::Glyph(glyph),
                ..
            },
            ..
        }) => glyph.italic,
        _ => Em::ZERO,
    };
    let base_box = built.into_box();
    let metrics = options.metrics();
    let char_box = base.is_some_and(Node::is_character_box);

    let mut sup_shift = 0.0;
    let mut sub_shift = 0.0;
    let mut sup_box = None;
    let mut sub_box = None;
    if let Some(sup) = sup {
        let inner = options.having_style(options.style.sup());
        let bx = build_in(sup, &inner, options)?.into_box();
        if !char_box {
            sup_shift = base_box.height.0
                - inner.metrics().sup_drop * inner.size_multiplier() / options.size_multiplier();
        }
        sup_box = Some(bx);
    }
    if let Some(sub) = sub {
        let inner = options.having_style(options.style.sub());
        let bx = build_in(sub, &inner, options)?.into_box();
        if !char_box {
            sub_shift = base_box.depth.0
                + inner.metrics().sub_drop * inner.size_multiplier() / options.size_multiplier();
        }
        sub_box = Some(bx);
    }

    let min_sup_shift = if options.style == Style::Display {
        metrics.sup1
    } else if options.style.is_cramped() {
        metrics.sup3
    } else {
        metrics.sup2
    };
    // TeX's \scriptspace, 0.5pt at any size.
    let script_space = Em(0.5 / metrics.pt_per_em / options.size_multiplier());
    let margin_left = -italic;

    let scripts = match (sup_box, sub_box) {
        (Some(supm), Some(subm)) => {
            sup_shift = sup_shift
                .max(min_sup_shift)
                .max(supm.depth.0 + 0.25 * metrics.x_height);
            sub_shift = sub_shift.max(metrics.sub2);
            let max_width = 4.0 * metrics.default_rule_thickness;
            if (sup_shift - supm.depth.0) - (subm.height.0 - sub_shift) < max_width {
                sub_shift = max_width - (sup_shift - supm.depth.0) + subm.height.0;
                let psi = 0.8 * metrics.x_height - (sup_shift - supm.depth.0);
                if psi > 0.0 {
                    sup_shift += psi;
                    sub_shift -= psi;
                }
            }
            Bx::vlist_shifted(vec![
                (
                    with_space_after(subm, script_space),
                    Em(sub_shift),
                    margin_left,
                ),
                (
                    with_space_after(supm, script_space),
                    Em(-sup_shift),
                    Em::ZERO,
                ),
            ])
        }
        (None, Some(subm)) => {
            sub_shift = sub_shift
                .max(metrics.sub1)
                .max(subm.height.0 - 0.8 * metrics.x_height);
            Bx::vlist(
                Position::Shift(Em(sub_shift)),
                vec![Part::Box {
                    child: with_space_after(subm, script_space),
                    dx: margin_left,
                }],
            )
        }
        (Some(supm), None) => {
            sup_shift = sup_shift
                .max(min_sup_shift)
                .max(supm.depth.0 + 0.25 * metrics.x_height);
            Bx::vlist(
                Position::Shift(Em(-sup_shift)),
                vec![Part::of(with_space_after(supm, script_space))],
            )
        }
        (None, None) => Bx::hlist(Vec::new()),
    };
    let bx = Bx::hlist(vec![base_box, scripts]);
    Ok(Built::One(Atom::new(bx, Some(class), options)))
}
