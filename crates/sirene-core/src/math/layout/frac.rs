//! Fractions and binomials, by the TeXbook's rules 15a to 15e.
//!
//! Adapted from KaTeX's `src/functions/genfrac.ts` (MIT, Copyright (c) 2013-2020 Khan Academy
//! and other contributors). The delimiters on each side come from `delimiter::frac_delimiter`.

use std::ops::Range;

use super::{Atom, Built, Class, build_in, delimiter};
use crate::math::{
    MathError,
    ast::{Measurement, Node},
    boxes::Bx,
    options::{Em, Options},
    style::Style,
    symbols::Mode,
};

/// A fraction: what `NodeKind::Frac` holds, and where it is.
pub(super) struct Fraction<'t, 'n> {
    pub numer: &'t Node<'n>,
    pub denom: &'t Node<'n>,
    pub continued: bool,
    pub bar: bool,
    pub bar_size: Option<&'t Measurement>,
    pub left: Option<&'t str>,
    pub right: Option<&'t str>,
    pub mode: Mode,
    pub span: Range<usize>,
}

pub(super) fn fraction<'t>(
    frac: Fraction<'t, '_>,
    options: &Options<'t>,
) -> Result<Built<'t>, MathError> {
    let style = options.style;
    let metrics = options.metrics();
    let numer_options = options.having_style(style.frac_num());
    let mut numer = build_in(frac.numer, &numer_options, options)?.into_box();
    if frac.continued {
        // `\cfrac` puts a strut in the numerator (TeXbook p. 353).
        numer.height = numer.height.max(Em(8.5 / metrics.pt_per_em));
        numer.depth = numer.depth.max(Em(3.5 / metrics.pt_per_em));
    }
    let denom_options = options.having_style(style.frac_den());
    let denom = build_in(frac.denom, &denom_options, options)?.into_box();

    let width = numer.width.max(denom.width);
    let rule = if frac.bar {
        let thickness = match frac.bar_size {
            Some(size) => options.length(size, frac.span.clone())?,
            None => Em(metrics.default_rule_thickness),
        };
        Some(Bx::line(width, thickness, options))
    } else {
        None
    };
    let (rule_width, rule_spacing) = match &rule {
        Some(rule) => (rule.height.0, rule.height.0),
        None => (0.0, metrics.default_rule_thickness),
    };

    let display = style.size() == Style::Display.size();
    let (mut num_shift, clearance, mut denom_shift) = match (display, rule_width > 0.0) {
        (true, true) => (metrics.num1, 3.0 * rule_spacing, metrics.denom1),
        (true, false) => (metrics.num1, 7.0 * rule_spacing, metrics.denom1),
        (false, true) => (metrics.num2, rule_spacing, metrics.denom2),
        (false, false) => (metrics.num3, 3.0 * rule_spacing, metrics.denom2),
    };

    let center = |bx: &Bx<'_>| (width - bx.width) / 2.0;
    let numer_dx = center(&numer);
    let denom_dx = center(&denom);
    let stack = match rule {
        None => {
            let candidate = (num_shift - numer.depth.0) - (denom.height.0 - denom_shift);
            if candidate < clearance {
                num_shift += 0.5 * (clearance - candidate);
                denom_shift += 0.5 * (clearance - candidate);
            }
            Bx::vlist_shifted(vec![
                (denom, Em(denom_shift), denom_dx),
                (numer, Em(-num_shift), numer_dx),
            ])
        }
        Some(rule) => {
            let axis = metrics.axis_height;
            let above = (num_shift - numer.depth.0) - (axis + 0.5 * rule_width);
            if above < clearance {
                num_shift += clearance - above;
            }
            let below = (axis - 0.5 * rule_width) - (denom.height.0 - denom_shift);
            if below < clearance {
                denom_shift += clearance - below;
            }
            let mid_shift = -(axis - 0.5 * rule_width);
            Bx::vlist_shifted(vec![
                (denom, Em(denom_shift), denom_dx),
                (rule, Em(mid_shift), Em::ZERO),
                (numer, Em(-num_shift), numer_dx),
            ])
        }
    };

    let left = delimiter::frac_delimiter(frac.left, frac.mode, options, frac.span.clone())?;
    // A continued fraction has nothing on its right, not even a null delimiter.
    let right = if frac.continued {
        Bx::kern(Em::ZERO)
    } else {
        delimiter::frac_delimiter(frac.right, frac.mode, options, frac.span)?
    };
    let bx = Bx::hlist(vec![left, stack, right]);
    Ok(Built::One(Atom::new(bx, Some(Class::Ord), options)))
}
