//! The font-wide parameters TeX lays math out by (LMATH-12, `metrics`).
//!
//! Adapted from KaTeX's `src/fontMetrics.ts` (MIT, Copyright (c) 2013-2020 Khan Academy and other
//! contributors): the sigma parameters of cmsy10, cmsy7, and cmsy5, the xi parameters of cmex10,
//! and KaTeX's own constants, in em. The per-glyph metrics are a separate generated table.

/// One set of the font parameters, for text, script, or scriptscript sizes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FontMetrics {
    pub slant: f64,
    pub space: f64,
    pub stretch: f64,
    pub shrink: f64,
    pub x_height: f64,
    pub quad: f64,
    pub extra_space: f64,
    pub num1: f64,
    pub num2: f64,
    pub num3: f64,
    pub denom1: f64,
    pub denom2: f64,
    pub sup1: f64,
    pub sup2: f64,
    pub sup3: f64,
    pub sub1: f64,
    pub sub2: f64,
    pub sup_drop: f64,
    pub sub_drop: f64,
    pub delim1: f64,
    pub delim2: f64,
    pub axis_height: f64,
    pub default_rule_thickness: f64,
    pub big_op_spacing1: f64,
    pub big_op_spacing2: f64,
    pub big_op_spacing3: f64,
    pub big_op_spacing4: f64,
    pub big_op_spacing5: f64,
    /// The rule of `\sqrt`, from the height of the surd; it does not scale.
    pub sqrt_rule_thickness: f64,
    /// TeX points in one em, for lengths given in points.
    pub pt_per_em: f64,
    /// The gap between the rules of `||` in an array's columns.
    pub double_rule_sep: f64,
    /// The width of an array's rules.
    pub array_rule_width: f64,
    pub fboxsep: f64,
    pub fboxrule: f64,
    /// One math unit: a quad over 18.
    pub css_em_per_mu: f64,
}

/// Builds the three sets from KaTeX's table of `[text, script, scriptscript]` rows.
macro_rules! metrics {
    ($($field:ident: [$t:expr, $s:expr, $ss:expr],)*) => {
        const TEXT: FontMetrics = FontMetrics {
            $($field: $t,)*
            css_em_per_mu: QUAD[0] / 18.0,
        };
        const SCRIPT: FontMetrics = FontMetrics {
            $($field: $s,)*
            css_em_per_mu: QUAD[1] / 18.0,
        };
        const SCRIPTSCRIPT: FontMetrics = FontMetrics {
            $($field: $ss,)*
            css_em_per_mu: QUAD[2] / 18.0,
        };
    };
}

const QUAD: [f64; 3] = [1.000, 1.171, 1.472];

metrics! {
    slant: [0.250, 0.250, 0.250],
    space: [0.000, 0.000, 0.000],
    stretch: [0.000, 0.000, 0.000],
    shrink: [0.000, 0.000, 0.000],
    x_height: [0.431, 0.431, 0.431],
    quad: [QUAD[0], QUAD[1], QUAD[2]],
    extra_space: [0.000, 0.000, 0.000],
    num1: [0.677, 0.732, 0.925],
    num2: [0.394, 0.384, 0.387],
    num3: [0.444, 0.471, 0.504],
    denom1: [0.686, 0.752, 1.025],
    denom2: [0.345, 0.344, 0.532],
    sup1: [0.413, 0.503, 0.504],
    sup2: [0.363, 0.431, 0.404],
    sup3: [0.289, 0.286, 0.294],
    sub1: [0.150, 0.143, 0.200],
    sub2: [0.247, 0.286, 0.400],
    sup_drop: [0.386, 0.353, 0.494],
    sub_drop: [0.050, 0.071, 0.100],
    delim1: [2.390, 1.700, 1.980],
    delim2: [1.010, 1.157, 1.420],
    axis_height: [0.250, 0.250, 0.250],
    default_rule_thickness: [0.04, 0.049, 0.049],
    big_op_spacing1: [0.111, 0.111, 0.111],
    big_op_spacing2: [0.166, 0.166, 0.166],
    big_op_spacing3: [0.2, 0.2, 0.2],
    big_op_spacing4: [0.6, 0.611, 0.611],
    big_op_spacing5: [0.1, 0.143, 0.143],
    sqrt_rule_thickness: [0.04, 0.04, 0.04],
    pt_per_em: [10.0, 10.0, 10.0],
    double_rule_sep: [0.2, 0.2, 0.2],
    array_rule_width: [0.04, 0.04, 0.04],
    fboxsep: [0.3, 0.3, 0.3],
    fboxrule: [0.04, 0.04, 0.04],
}

/// The parameters for a size index (1 to 11, `\tiny` to `\Huge`): text for 5 and up, script
/// for 3 and 4, scriptscript below.
pub(crate) fn for_size(size: u8) -> &'static FontMetrics {
    match size {
        5.. => &TEXT,
        3 | 4 => &SCRIPT,
        _ => &SCRIPTSCRIPT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_pick_katex_columns() {
        assert_eq!(for_size(6).quad, 1.0);
        assert_eq!(for_size(5).num1, 0.677);
        assert_eq!(for_size(4).quad, 1.171);
        assert_eq!(for_size(3).default_rule_thickness, 0.049);
        assert_eq!(for_size(2).quad, 1.472);
        assert_eq!(for_size(1).sub2, 0.400);
        assert_eq!(for_size(11).big_op_spacing4, 0.6);
    }

    #[test]
    fn a_mu_is_a_quad_over_18() {
        assert_eq!(for_size(6).css_em_per_mu, 1.0 / 18.0);
        assert_eq!(for_size(3).css_em_per_mu, 1.171 / 18.0);
        assert_eq!(for_size(1).css_em_per_mu, 1.472 / 18.0);
    }
}
