//! What layout knows at each node: style, size, font, and color (LMATH-12, `options` and
//! `units`).
//!
//! Adapted from KaTeX's `src/Options.ts` and `src/units.ts` (MIT, Copyright (c) 2013-2020 Khan
//! Academy and other contributors). `Options` is an immutable value: a change of style or font
//! builds a new one, as KaTeX's `having*` and `with*` methods do.

use std::ops::{Add, AddAssign, Div, Mul, Neg, Range, Sub, SubAssign};

use super::{
    MathError,
    ast::Measurement,
    limits::MAX_LENGTH_EM,
    metrics::{self, FontMetrics},
    style::Style,
};

/// A length in em of the current font. Layout works in these; outputs scale them.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub(crate) struct Em(pub f64);

impl Em {
    pub(crate) const ZERO: Em = Em(0.0);

    pub(crate) fn max(self, other: Em) -> Em {
        Em(self.0.max(other.0))
    }

    pub(crate) fn min(self, other: Em) -> Em {
        Em(self.0.min(other.0))
    }
}

impl Add for Em {
    type Output = Em;
    fn add(self, rhs: Em) -> Em {
        Em(self.0 + rhs.0)
    }
}

impl AddAssign for Em {
    fn add_assign(&mut self, rhs: Em) {
        self.0 += rhs.0;
    }
}

impl Sub for Em {
    type Output = Em;
    fn sub(self, rhs: Em) -> Em {
        Em(self.0 - rhs.0)
    }
}

impl SubAssign for Em {
    fn sub_assign(&mut self, rhs: Em) {
        self.0 -= rhs.0;
    }
}

impl Neg for Em {
    type Output = Em;
    fn neg(self) -> Em {
        Em(-self.0)
    }
}

impl Mul<f64> for Em {
    type Output = Em;
    fn mul(self, rhs: f64) -> Em {
        Em(self.0 * rhs)
    }
}

impl Div<f64> for Em {
    type Output = Em;
    fn div(self, rhs: f64) -> Em {
        Em(self.0 / rhs)
    }
}

/// The size index of `\normalsize`; indexes run from 1 (`\tiny`) to 11 (`\Huge`).
pub(crate) const BASE_SIZE: u8 = 6;

/// How much each size index scales the font, from `\tiny` to `\Huge`.
const SIZE_MULTIPLIERS: [f64; 11] = [0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.2, 1.44, 1.728, 2.074, 2.488];

/// For each size index, the index its script and scriptscript styles are set at, from TeX with
/// `\normalsize` at 10pt.
const SCRIPT_SIZES: [[u8; 2]; 11] = [
    [1, 1],
    [1, 1],
    [1, 1],
    [2, 1],
    [2, 1],
    [3, 1],
    [4, 2],
    [6, 3],
    [7, 6],
    [8, 7],
    [10, 9],
];

/// The multiplier of a size index, which is clamped to 1 to 11.
pub(crate) fn size_multiplier(size: u8) -> f64 {
    SIZE_MULTIPLIERS[usize::from(size.clamp(1, 11)) - 1]
}

/// The size index a style is set at, given the size index of the text around it.
fn size_at_style(text_size: u8, style: Style) -> u8 {
    match style.size() {
        0 | 1 => text_size,
        s => SCRIPT_SIZES[usize::from(text_size.clamp(1, 11)) - 1][usize::from(s) - 2],
    }
}

/// A math font command's font (`\mathbf`), which picks the face of every symbol in its body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MathFont {
    Rm,
    It,
    Bf,
    Normal,
    SfIt,
    Bb,
    Cal,
    Frak,
    Scr,
    Sf,
    Tt,
    BoldSymbol,
}

impl MathFont {
    /// The font a command name without its backslash stands for (`mathbf`), as the parse tree
    /// records it.
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "mathrm" => Self::Rm,
            "mathit" => Self::It,
            "mathbf" => Self::Bf,
            "mathnormal" => Self::Normal,
            "mathsfit" => Self::SfIt,
            "mathbb" => Self::Bb,
            "mathcal" => Self::Cal,
            "mathfrak" => Self::Frak,
            "mathscr" => Self::Scr,
            "mathsf" => Self::Sf,
            "mathtt" => Self::Tt,
            "boldsymbol" => Self::BoldSymbol,
            _ => return None,
        })
    }
}

/// The family of text-mode fonts (`\textsf`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextFamily {
    Rm,
    Sf,
    Tt,
    /// The AMS font, for the text-mode symbols only it has.
    Ams,
}

/// A text font's weight (`\textbf`, `\textmd`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Weight {
    Bold,
    Medium,
}

/// A text font's shape (`\textit`, `\textup`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shape {
    Italic,
    Upright,
}

/// The state layout passes down: style, size, color, and font. `'a` is the parse tree's, whose
/// color names it borrows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Options<'a> {
    pub style: Style,
    /// The size index, 1 to 11.
    pub size: u8,
    /// The size index of the text around a script, which a script's own size follows.
    pub text_size: u8,
    pub color: Option<&'a str>,
    /// Inside `\phantom`: everything takes space and nothing is drawn.
    pub phantom: bool,
    pub font: Option<MathFont>,
    pub family: Option<TextFamily>,
    pub weight: Option<Weight>,
    pub shape: Option<Shape>,
    /// The thinnest a rule is drawn, so a hairline stays visible.
    pub min_rule_thickness: Em,
}

impl<'a> Options<'a> {
    /// The options at the top of a formula: display or text style at `\normalsize`.
    pub(crate) fn new(display: bool) -> Self {
        Self {
            style: if display { Style::Display } else { Style::Text },
            size: BASE_SIZE,
            text_size: BASE_SIZE,
            color: None,
            phantom: false,
            font: None,
            family: None,
            weight: None,
            shape: None,
            min_rule_thickness: Em::ZERO,
        }
    }

    /// How much the font is scaled from `\normalsize`.
    pub(crate) fn size_multiplier(&self) -> f64 {
        size_multiplier(self.size)
    }

    /// The font parameters for this size.
    pub(crate) fn metrics(&self) -> &'static FontMetrics {
        metrics::for_size(self.size)
    }

    /// The color to draw in: none in a phantom.
    pub(crate) fn color(&self) -> Option<&'a str> {
        if self.phantom {
            Some("transparent")
        } else {
            self.color
        }
    }

    /// These options in another style, sized for it.
    pub(crate) fn having_style(self, style: Style) -> Self {
        if self.style == style {
            return self;
        }
        Self {
            style,
            size: size_at_style(self.text_size, style),
            ..self
        }
    }

    /// These options in the cramped form of their style.
    pub(crate) fn having_cramped_style(self) -> Self {
        self.having_style(self.style.cramp())
    }

    /// These options at a size index (`\large`), in at least text style.
    pub(crate) fn having_size(self, size: u8) -> Self {
        if self.size == size && self.text_size == size {
            return self;
        }
        Self {
            style: self.style.text(),
            size,
            text_size: size,
            ..self
        }
    }

    /// As `having_size(BASE_SIZE).having_style(style)`: a style at `\normalsize`, for delimiters
    /// and operators whose glyphs come in fixed sizes.
    pub(crate) fn having_base_style(self, style: Style) -> Self {
        let size = size_at_style(BASE_SIZE, style);
        if self.size == size && self.text_size == BASE_SIZE && self.style == style {
            return self;
        }
        Self {
            style,
            size,
            ..self
        }
    }

    /// Undoes a size command (`\Huge`) but keeps the style's own size.
    pub(crate) fn having_base_sizing(self) -> Self {
        let size = match self.style.size() {
            2 => 3,
            3 => 1,
            _ => BASE_SIZE,
        };
        Self {
            style: self.style.text(),
            size,
            ..self
        }
    }

    pub(crate) fn with_color(self, color: &'a str) -> Self {
        Self {
            color: Some(color),
            ..self
        }
    }

    pub(crate) fn with_phantom(self) -> Self {
        Self {
            phantom: true,
            ..self
        }
    }

    pub(crate) fn with_font(self, font: MathFont) -> Self {
        Self {
            font: Some(font),
            ..self
        }
    }

    /// A text family, which replaces any math font.
    pub(crate) fn with_family(self, family: TextFamily) -> Self {
        Self {
            family: Some(family),
            font: None,
            ..self
        }
    }

    pub(crate) fn with_weight(self, weight: Weight) -> Self {
        Self {
            weight: Some(weight),
            font: None,
            ..self
        }
    }

    pub(crate) fn with_shape(self, shape: Shape) -> Self {
        Self {
            shape: Some(shape),
            font: None,
            ..self
        }
    }

    /// A length from the source in em of these options' font, within `MAX_LENGTH_EM` either way.
    /// Points and the other absolute units do not scale with the size; `ex` and `em` are those of
    /// the text style at this size; `mu` is an eighteenth of a quad at this size. `span` is the
    /// length's place in the source, for the error of a unit KaTeX does not know.
    pub(crate) fn length(&self, length: &Measurement, span: Range<usize>) -> Result<Em, MathError> {
        let scale = if let Some(pt) = points_per_unit(&length.unit) {
            pt / self.metrics().pt_per_em / self.size_multiplier()
        } else if length.unit == "mu" {
            self.metrics().css_em_per_mu
        } else {
            let text = if self.style.is_tight() {
                self.having_style(self.style.text())
            } else {
                *self
            };
            let scale = match length.unit.as_str() {
                "ex" => text.metrics().x_height,
                "em" => text.metrics().quad,
                unit => return Err(MathError::new(format!("Invalid unit: '{unit}'"), span)),
            };
            scale * text.size_multiplier() / self.size_multiplier()
        };
        Ok(Em(
            (length.number * scale).clamp(-MAX_LENGTH_EM, MAX_LENGTH_EM)
        ))
    }
}

/// TeX points in one of an absolute unit.
fn points_per_unit(unit: &str) -> Option<f64> {
    Some(match unit {
        "pt" => 1.0,
        "mm" => 7227.0 / 2540.0,
        "cm" => 7227.0 / 254.0,
        "in" => 72.27,
        "bp" => 803.0 / 800.0,
        "pc" => 12.0,
        "dd" => 1238.0 / 1157.0,
        "cc" => 14856.0 / 1157.0,
        "nd" => 685.0 / 642.0,
        "nc" => 1370.0 / 107.0,
        "sp" => 1.0 / 65536.0,
        "px" => 803.0 / 800.0,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn len(number: f64, unit: &str) -> Measurement {
        Measurement {
            number,
            unit: unit.to_owned(),
        }
    }

    fn em(options: &Options<'_>, number: f64, unit: &str) -> f64 {
        options.length(&len(number, unit), 0..0).unwrap().0
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn styles_resize_from_the_text_size() {
        let text = Options::new(false);
        assert_eq!(
            (text.style, text.size, text.size_multiplier()),
            (Style::Text, 6, 1.0)
        );
        let script = text.having_style(text.style.sup());
        assert_eq!((script.style, script.size), (Style::Script, 3));
        let scriptscript = script.having_style(script.style.sup());
        assert_eq!((scriptscript.size, scriptscript.text_size), (1, 6));
        let large = text.having_size(7).having_style(Style::Script);
        assert_eq!((large.size, large.text_size), (4, 7));
        let display = Options::new(true).having_size(8);
        assert_eq!((display.style, display.size), (Style::Display, 8));
        let script_large = Options::new(false)
            .having_style(Style::Script)
            .having_size(9);
        assert_eq!(script_large.style, Style::Text);
    }

    #[test]
    fn base_style_and_base_sizing() {
        let huge = Options::new(false).having_size(11);
        let base = huge.having_base_style(Style::Script);
        assert_eq!(
            (base.style, base.size, base.text_size),
            (Style::Script, 3, 11)
        );
        let back = huge.having_style(Style::ScriptScript).having_base_sizing();
        assert_eq!((back.style, back.size), (Style::Text, 1));
        let plain = Options::new(true);
        assert_eq!(plain.having_base_style(Style::Display), plain);
    }

    #[test]
    fn fonts_and_colors() {
        let options = Options::new(false).with_font(MathFont::Bf);
        assert_eq!(options.with_weight(Weight::Bold).font, None);
        assert_eq!(
            options.with_family(TextFamily::Sf).family,
            Some(TextFamily::Sf)
        );
        assert_eq!(
            MathFont::from_name("boldsymbol"),
            Some(MathFont::BoldSymbol)
        );
        assert_eq!(MathFont::from_name("textbf"), None);
        let red = options.with_color("red");
        assert_eq!(red.color(), Some("red"));
        assert_eq!(red.with_phantom().color(), Some("transparent"));
    }

    #[test]
    fn absolute_lengths_do_not_scale() {
        let text = Options::new(false);
        assert!(close(em(&text, 10.0, "pt"), 1.0));
        assert!(close(em(&text, 1.0, "in"), 7.227));
        let small = text.having_size(1);
        assert!(close(em(&small, 10.0, "pt"), 2.0));
        assert!(close(em(&small, 10.0, "pt") * small.size_multiplier(), 1.0));
    }

    #[test]
    fn relative_lengths_follow_the_text_style() {
        let text = Options::new(false);
        assert!(close(em(&text, 2.0, "em"), 2.0));
        assert!(close(em(&text, 1.0, "ex"), 0.431));
        assert!(close(em(&text, 18.0, "mu"), 1.0));
        // In script style an em is the text style's, rescaled to the script's font.
        let script = text.having_style(Style::Script);
        assert!(close(em(&script, 1.0, "em"), 1.0 / 0.7));
        assert!(close(em(&script, 18.0, "mu"), 1.171));
    }

    #[test]
    fn lengths_are_capped_and_units_checked() {
        let text = Options::new(false);
        assert_eq!(em(&text, 1e9, "em"), MAX_LENGTH_EM);
        assert_eq!(em(&text, -1e9, "em"), -MAX_LENGTH_EM);
        let err = text.length(&len(1.0, "qq"), 3..7).unwrap_err();
        assert_eq!(err, MathError::new("Invalid unit: 'qq'", 3..7));
    }

    #[test]
    fn em_arithmetic() {
        let mut a = Em(1.0) + Em(0.5) - Em(0.25);
        a += Em(1.0);
        a -= Em(0.25);
        assert_eq!(a, Em(2.0));
        assert_eq!(-(a * 2.0) / 4.0, Em(-1.0));
        assert_eq!(a.max(Em(3.0)), Em(3.0));
        assert_eq!(a.min(Em(3.0)), a);
    }
}
