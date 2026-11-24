//! The font parameters TeX lays math out by, and each glyph's size (LMATH-12, `metrics`).
//!
//! Adapted from KaTeX's `src/fontMetrics.ts` and `src/unicodeScripts.ts` (MIT, Copyright (c)
//! 2013-2020 Khan Academy and other contributors): the sigma parameters of cmsy10, cmsy7, and
//! cmsy5, the xi parameters of cmex10, and KaTeX's own constants, in em. The per-glyph metrics
//! are KaTeX's table, generated into `metrics_data.rs` by `scripts/katex-metrics.mjs`.

use super::{metrics_data::Face, symbols::Mode};

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

/// The size of one glyph, in em.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CharMetrics {
    pub depth: f64,
    pub height: f64,
    /// The italic correction: how far the glyph leans past its width.
    pub italic: f64,
    /// How far right an accent over the glyph moves.
    pub skew: f64,
    pub width: f64,
}

/// The metrics of a character in a face. A Latin-1 or Cyrillic letter the face lacks borrows a
/// similar Latin letter's, and in text mode a character of a script KaTeX supports without
/// metrics (CJK, Devanagari, ...) borrows `M`'s, as KaTeX's `getCharacterMetrics` does.
pub(crate) fn glyph(c: char, face: Face, mode: Mode) -> Option<CharMetrics> {
    let glyphs = face.glyphs();
    let find = |c: char| {
        glyphs
            .binary_search_by_key(&u32::from(c), |&(cp, _)| cp)
            .ok()
            .and_then(|i| glyphs.get(i))
    };
    let found = find(c)
        .or_else(|| similar_letter(c).and_then(find))
        .or_else(|| (mode == Mode::Text && supported_script(c)).then(|| find('M'))?)?;
    let [depth, height, italic, skew, width] = found.1;
    Some(CharMetrics {
        depth,
        height,
        italic,
        skew,
        width,
    })
}

/// KaTeX's stand-ins for Latin-1 and Cyrillic letters that have no metrics, chosen with
/// ascenders so a bar or radical line above clears them.
fn similar_letter(c: char) -> Option<char> {
    let i = SIMILAR.binary_search_by_key(&c, |&(c, _)| c).ok()?;
    SIMILAR.get(i).map(|&(_, s)| s)
}

const SIMILAR: [(char, char); 70] = [
    ('\u{c5}', 'A'),
    ('\u{d0}', 'D'),
    ('\u{de}', 'o'),
    ('\u{e5}', 'a'),
    ('\u{f0}', 'd'),
    ('\u{fe}', 'o'),
    ('\u{410}', 'A'),
    ('\u{411}', 'B'),
    ('\u{412}', 'B'),
    ('\u{413}', 'F'),
    ('\u{414}', 'A'),
    ('\u{415}', 'E'),
    ('\u{416}', 'K'),
    ('\u{417}', '3'),
    ('\u{418}', 'N'),
    ('\u{419}', 'N'),
    ('\u{41a}', 'K'),
    ('\u{41b}', 'N'),
    ('\u{41c}', 'M'),
    ('\u{41d}', 'H'),
    ('\u{41e}', 'O'),
    ('\u{41f}', 'N'),
    ('\u{420}', 'P'),
    ('\u{421}', 'C'),
    ('\u{422}', 'T'),
    ('\u{423}', 'y'),
    ('\u{424}', 'O'),
    ('\u{425}', 'X'),
    ('\u{426}', 'U'),
    ('\u{427}', 'h'),
    ('\u{428}', 'W'),
    ('\u{429}', 'W'),
    ('\u{42a}', 'B'),
    ('\u{42b}', 'X'),
    ('\u{42c}', 'B'),
    ('\u{42d}', '3'),
    ('\u{42e}', 'X'),
    ('\u{42f}', 'R'),
    ('\u{430}', 'a'),
    ('\u{431}', 'b'),
    ('\u{432}', 'a'),
    ('\u{433}', 'r'),
    ('\u{434}', 'y'),
    ('\u{435}', 'e'),
    ('\u{436}', 'm'),
    ('\u{437}', 'e'),
    ('\u{438}', 'n'),
    ('\u{439}', 'n'),
    ('\u{43a}', 'n'),
    ('\u{43b}', 'n'),
    ('\u{43c}', 'm'),
    ('\u{43d}', 'n'),
    ('\u{43e}', 'o'),
    ('\u{43f}', 'n'),
    ('\u{440}', 'p'),
    ('\u{441}', 'c'),
    ('\u{442}', 'o'),
    ('\u{443}', 'y'),
    ('\u{444}', 'b'),
    ('\u{445}', 'x'),
    ('\u{446}', 'n'),
    ('\u{447}', 'n'),
    ('\u{448}', 'w'),
    ('\u{449}', 'w'),
    ('\u{44a}', 'a'),
    ('\u{44b}', 'm'),
    ('\u{44c}', 'a'),
    ('\u{44d}', 'e'),
    ('\u{44e}', 'm'),
    ('\u{44f}', 'r'),
];

/// The Unicode blocks of the scripts KaTeX allows in text without metrics of their own: Latin
/// Extended-A and B, combining marks, Cyrillic, Armenian, the Brahmic scripts through Myanmar,
/// Georgian, CJK, and Hangul.
const SCRIPT_BLOCKS: [(u32, u32); 10] = [
    (0x0100, 0x024f),
    (0x0300, 0x036f),
    (0x0400, 0x04ff),
    (0x0530, 0x058f),
    (0x0900, 0x109f),
    (0x10a0, 0x10ff),
    (0x3000, 0x30ff),
    (0x4e00, 0x9faf),
    (0xac00, 0xd7af),
    (0xff00, 0xff60),
];

/// Whether a character is in one of `SCRIPT_BLOCKS`.
pub(crate) fn supported_script(c: char) -> bool {
    let cp = u32::from(c);
    SCRIPT_BLOCKS
        .iter()
        .any(|&(start, end)| (start..=end).contains(&cp))
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

    fn sizes(c: char, face: Face, mode: Mode) -> Option<[f64; 5]> {
        glyph(c, face, mode).map(|m| [m.depth, m.height, m.italic, m.skew, m.width])
    }

    #[test]
    fn faces_are_sorted_and_named() {
        let mut total = 0;
        for face in Face::ALL {
            let glyphs = face.glyphs();
            assert!(
                glyphs.windows(2).all(|w| w[0].0 < w[1].0),
                "{} is not sorted",
                face.name()
            );
            total += glyphs.len();
        }
        assert_eq!((Face::ALL.len(), total), (18, 2038));
        assert_eq!(Face::AmsRegular.name(), "AMS-Regular");
        assert!(SIMILAR.windows(2).all(|w| w[0].0 < w[1].0));
    }

    #[test]
    fn glyphs_read_katex_values() {
        let text = Mode::Text;
        let math = Mode::Math;
        assert_eq!(
            sizes('x', Face::MainRegular, math),
            Some([0.0, 0.43056, 0.0, 0.0, 0.52778])
        );
        assert_eq!(
            sizes('x', Face::MathItalic, math),
            Some([0.0, 0.43056, 0.0, 0.02778, 0.57153])
        );
        assert_eq!(
            sizes('\u{2211}', Face::Size1Regular, math),
            Some([0.25001, 0.75, 0.0, 0.0, 1.05556])
        );
        // Letters without metrics borrow a similar letter's, in either mode.
        assert_eq!(
            sizes('\u{416}', Face::MainRegular, math),
            sizes('K', Face::MainRegular, math)
        );
        assert_eq!(
            sizes('\u{d0}', Face::MainRegular, text),
            sizes('D', Face::MainRegular, text)
        );
        // A supported script borrows `M` in text mode only.
        let m = sizes('M', Face::MainRegular, text);
        assert_eq!(sizes('\u{4e2d}', Face::MainRegular, text), m);
        assert_eq!(sizes('\u{4e2d}', Face::MainRegular, math), None);
        assert_eq!(sizes('\u{1d400}', Face::MainRegular, text), None);
        assert!(supported_script('\u{ac00}') && !supported_script('\u{d7b0}'));
    }

    #[test]
    fn a_mu_is_a_quad_over_18() {
        assert_eq!(for_size(6).css_em_per_mu, 1.0 / 18.0);
        assert_eq!(for_size(3).css_em_per_mu, 1.171 / 18.0);
        assert_eq!(for_size(1).css_em_per_mu, 1.472 / 18.0);
    }
}
