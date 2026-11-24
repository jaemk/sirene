//! The box tree layout builds and every output reads (LMATH-2, step 5; LMATH-13).
//!
//! A box has a width, a height above its baseline, and a depth below it, in em of the font it
//! was built in, as KaTeX's spans do. Where the font size changes, a `Scaled` box holds the
//! child built at the new size and gives its size in the outer font's em. `VList` stacking
//! follows KaTeX's `makeVList` (`src/buildCommon.ts`, MIT, Copyright (c) 2013-2020 Khan Academy
//! and other contributors), so the layout code can be ported with the same numbers.
//! `Bx::items` flattens a tree into glyphs and rules at absolute positions, the display list
//! of LMATH-4.

use std::borrow::Cow;

use super::{
    metrics::{self, CharMetrics},
    metrics_data::Face,
    options::{Em, MathFont, Options},
    symbols::Mode,
};

/// A laid out box. `'a` is the parse tree's, whose text and color names it borrows.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Bx<'a> {
    pub width: Em,
    pub height: Em,
    pub depth: Em,
    pub kind: BoxKind<'a>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum BoxKind<'a> {
    /// Text set in one face.
    Glyph(Glyph<'a>),
    /// A filled rectangle over the whole box: a fraction bar, an overline, `\rule`.
    Rule { color: Option<&'a str> },
    /// Empty space: a kern, a strut, a phantom's room.
    Space,
    /// Boxes side by side on one baseline, left to right.
    HList(Vec<Bx<'a>>),
    /// Boxes placed at given heights, each at its own horizontal offset.
    VList(Vec<Placed<'a>>),
    /// A box built at another font size, `factor` times this one's (`\scriptstyle`, `\large`).
    Scaled { factor: f64, child: Box<Bx<'a>> },
}

/// One run of text in one face.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Glyph<'a> {
    pub text: Cow<'a, str>,
    pub face: Face,
    /// How far the glyph leans past its width; a superscript moves right by it.
    pub italic: Em,
    /// How far right an accent over it moves.
    pub skew: Em,
    pub color: Option<&'a str>,
}

/// A child of a `VList`: its baseline `raise` above the list's baseline and its `dx` from the
/// list's left edge.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Placed<'a> {
    pub raise: Em,
    pub dx: Em,
    pub child: Bx<'a>,
}

/// A part of a vertical stack, bottom to top: a box with a horizontal offset, or a kern.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Part<'a> {
    Box { child: Bx<'a>, dx: Em },
    Kern(Em),
}

impl<'a> Part<'a> {
    /// A box at the left edge.
    pub(crate) fn of(child: Bx<'a>) -> Self {
        Part::Box {
            child,
            dx: Em::ZERO,
        }
    }
}

/// Where a stack sits against the baseline, as KaTeX's `positionType`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Position {
    /// The top of the stack is this far above the baseline.
    Top(Em),
    /// The bottom of the stack is this far below the baseline.
    Bottom(Em),
    /// The first box's baseline is this far below the stack's.
    Shift(Em),
    /// The first box's baseline is the stack's.
    FirstBaseline,
}

impl<'a> Bx<'a> {
    /// Empty space of a width.
    pub(crate) fn kern(width: Em) -> Self {
        Self::space(width, Em::ZERO, Em::ZERO)
    }

    /// Empty space of a given size.
    pub(crate) fn space(width: Em, height: Em, depth: Em) -> Self {
        Self {
            width,
            height,
            depth,
            kind: BoxKind::Space,
        }
    }

    /// A filled rectangle in the options' color.
    pub(crate) fn rule(width: Em, height: Em, depth: Em, options: &Options<'a>) -> Self {
        Self {
            width,
            height,
            depth,
            kind: BoxKind::Rule {
                color: options.color(),
            },
        }
    }

    /// A horizontal line `thickness` high (at least the options' thinnest rule) from the
    /// baseline up, as KaTeX's `makeLineSpan`; its width is the box it is placed over.
    pub(crate) fn line(width: Em, thickness: Em, options: &Options<'a>) -> Self {
        Self::rule(
            width,
            thickness.max(options.min_rule_thickness),
            Em::ZERO,
            options,
        )
    }

    /// Text in a face, sized by the metrics of its first character, as KaTeX's `makeSymbol`
    /// does. A character without metrics has no size, as there (KaTeX warns).
    pub(crate) fn glyph(
        text: impl Into<Cow<'a, str>>,
        face: Face,
        mode: Mode,
        options: &Options<'a>,
    ) -> Self {
        let text = text.into();
        let metrics = text
            .chars()
            .next()
            .and_then(|c| metrics::glyph(c, face, mode));
        let CharMetrics {
            depth,
            height,
            italic,
            skew,
            width,
        } = metrics.unwrap_or(CharMetrics {
            depth: 0.0,
            height: 0.0,
            italic: 0.0,
            skew: 0.0,
            width: 0.0,
        });
        // Text and `\mathit` have no italic correction, so scripts do not move for it.
        let italic = if mode == Mode::Text || options.font == Some(MathFont::It) {
            0.0
        } else {
            italic
        };
        Self {
            // KaTeX draws a positive italic correction as a margin after the glyph.
            width: Em(width + italic.max(0.0)),
            height: Em(height),
            depth: Em(depth),
            kind: BoxKind::Glyph(Glyph {
                text,
                face,
                italic: Em(italic),
                skew: Em(skew),
                color: options.color(),
            }),
        }
    }

    /// Boxes side by side: as wide as all of them, as high and deep as the highest and
    /// deepest, and never less than zero either way (KaTeX's `sizeElementFromChildren`).
    pub(crate) fn hlist(children: Vec<Bx<'a>>) -> Self {
        let mut width = Em::ZERO;
        let mut height = Em::ZERO;
        let mut depth = Em::ZERO;
        for child in &children {
            width += child.width;
            height = height.max(child.height);
            depth = depth.max(child.depth);
        }
        Self {
            width,
            height,
            depth,
            kind: BoxKind::HList(children),
        }
    }

    /// A box built at another size, `factor` times this one's, sized in this one's em.
    pub(crate) fn scaled(factor: f64, child: Bx<'a>) -> Self {
        if factor == 1.0 {
            return child;
        }
        Self {
            width: child.width * factor,
            height: child.height * factor,
            depth: child.depth * factor,
            kind: BoxKind::Scaled {
                factor,
                child: Box::new(child),
            },
        }
    }

    /// Boxes and kerns stacked bottom to top, placed against the baseline as `position` says.
    /// The stack is as high as its highest point and as deep as its lowest, kerns included,
    /// as KaTeX's `makeVList`.
    pub(crate) fn vlist(position: Position, parts: Vec<Part<'a>>) -> Self {
        let size = |part: &Part<'_>| match part {
            Part::Kern(k) => *k,
            Part::Box { child, .. } => child.height + child.depth,
        };
        let first_depth = match parts.first() {
            Some(Part::Box { child, .. }) => child.depth,
            _ => Em::ZERO,
        };
        let bottom = match position {
            Position::Top(top) => parts.iter().fold(top, |at, part| at - size(part)),
            Position::Bottom(depth) => -depth,
            Position::Shift(shift) => -first_depth - shift,
            Position::FirstBaseline => -first_depth,
        };
        Self::stack(bottom, parts)
    }

    /// Boxes each with its own baseline `drop` below the stack's (positive is down), listed
    /// bottom to top, as KaTeX's `individualShift`.
    pub(crate) fn vlist_shifted(children: Vec<(Bx<'a>, Em, Em)>) -> Self {
        let Some((first, first_drop, _)) = children.first() else {
            return Self::stack(Em::ZERO, Vec::new());
        };
        let bottom = -*first_drop - first.depth;
        let mut parts = Vec::with_capacity(children.len() * 2);
        // A kern from the top of each box to the bottom of the next.
        let mut previous_top: Option<Em> = None;
        for (child, drop, dx) in children {
            let bottom = -drop - child.depth;
            if let Some(top) = previous_top {
                parts.push(Part::Kern(bottom - top));
            }
            previous_top = Some(bottom + child.height + child.depth);
            parts.push(Part::Box { child, dx });
        }
        Self::stack(bottom, parts)
    }

    fn stack(bottom: Em, parts: Vec<Part<'a>>) -> Self {
        let mut at = bottom;
        let mut lowest = bottom;
        let mut highest = bottom;
        let mut width = Em::ZERO;
        let mut placed = Vec::new();
        for part in parts {
            match part {
                Part::Kern(k) => at += k,
                Part::Box { child, dx } => {
                    let raise = at + child.depth;
                    at += child.height + child.depth;
                    width = width.max(dx + child.width);
                    placed.push(Placed { raise, dx, child });
                }
            }
            lowest = lowest.min(at);
            highest = highest.max(at);
        }
        Self {
            width,
            height: highest,
            depth: -lowest,
            kind: BoxKind::VList(placed),
        }
    }

    /// The glyphs and rules of the tree at absolute positions, in em of the root's font, with
    /// the root's left edge at x = 0 and its baseline at y = 0, y growing down.
    pub(crate) fn items(&self) -> Vec<Item<'a>> {
        let mut out = Vec::new();
        self.collect(0.0, 0.0, 1.0, &mut out);
        out
    }

    fn collect(&self, x: f64, y: f64, scale: f64, out: &mut Vec<Item<'a>>) {
        match &self.kind {
            BoxKind::Glyph(glyph) => out.push(Item::Glyph {
                x,
                y,
                scale,
                glyph: glyph.clone(),
            }),
            BoxKind::Rule { color } => out.push(Item::Rule {
                x,
                y: y - self.height.0 * scale,
                width: self.width.0 * scale,
                height: (self.height.0 + self.depth.0) * scale,
                color: *color,
            }),
            BoxKind::Space => {}
            BoxKind::HList(children) => {
                let mut x = x;
                for child in children {
                    child.collect(x, y, scale, out);
                    x += child.width.0 * scale;
                }
            }
            BoxKind::VList(children) => {
                for Placed { raise, dx, child } in children {
                    child.collect(x + dx.0 * scale, y - raise.0 * scale, scale, out);
                }
            }
            BoxKind::Scaled { factor, child } => child.collect(x, y, scale * factor, out),
        }
    }
}

/// One drawn thing at an absolute position: a glyph run on its baseline, or a rectangle from
/// its top-left corner.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Item<'a> {
    Glyph {
        x: f64,
        y: f64,
        /// The font size, in em of the root's font.
        scale: f64,
        glyph: Glyph<'a>,
    },
    Rule {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: Option<&'a str>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options<'static> {
        Options::new(false)
    }

    fn sized(b: &Bx<'_>) -> (f64, f64, f64) {
        (b.width.0, b.height.0, b.depth.0)
    }

    fn close(a: (f64, f64, f64), b: (f64, f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9 && (a.2 - b.2).abs() < 1e-9
    }

    fn rule(width: f64, height: f64, depth: f64) -> Bx<'static> {
        Bx::rule(Em(width), Em(height), Em(depth), &opts())
    }

    #[test]
    fn glyphs_take_their_metrics() {
        let x = Bx::glyph("x", Face::MathItalic, Mode::Math, &opts());
        assert_eq!(sized(&x), (0.57153, 0.43056, 0.0));
        let BoxKind::Glyph(g) = &x.kind else {
            unreachable!("a glyph")
        };
        assert_eq!((g.skew, g.italic, g.color), (Em(0.02778), Em(0.0), None));
        // The italic correction widens the glyph; text mode and `\mathit` drop it.
        let italic = |b: &Bx<'_>| match &b.kind {
            BoxKind::Glyph(g) => g.italic.0,
            _ => f64::NAN,
        };
        let math = Bx::glyph("f", Face::MainItalic, Mode::Math, &opts());
        let text = Bx::glyph("f", Face::MainItalic, Mode::Text, &opts());
        let mathit = Bx::glyph(
            "f",
            Face::MainItalic,
            Mode::Math,
            &opts().with_font(MathFont::It),
        );
        assert!(italic(&math) > 0.0);
        assert_eq!((italic(&text), italic(&mathit)), (0.0, 0.0));
        assert_eq!(text.width, mathit.width);
        assert_eq!(math.width, text.width + Em(italic(&math)));
        // No metrics: no size.
        let unknown = Bx::glyph("\u{1d400}", Face::MainRegular, Mode::Math, &opts());
        assert_eq!(sized(&unknown), (0.0, 0.0, 0.0));
        let red = Bx::glyph("x", Face::MathItalic, Mode::Math, &opts().with_color("red"));
        let BoxKind::Glyph(g) = &red.kind else {
            unreachable!("a glyph")
        };
        assert_eq!(g.color, Some("red"));
    }

    #[test]
    fn hlists_add_widths_and_take_extremes() {
        let list = Bx::hlist(vec![
            rule(1.0, 0.5, 0.25),
            Bx::kern(Em(0.5)),
            rule(2.0, 0.75, -1.0),
        ]);
        assert_eq!(sized(&list), (3.5, 0.75, 0.25));
        // Never less than zero.
        let low = Bx::hlist(vec![rule(1.0, -0.5, -0.25)]);
        assert_eq!(sized(&low), (1.0, 0.0, 0.0));
        assert_eq!(sized(&Bx::hlist(Vec::new())), (0.0, 0.0, 0.0));
    }

    #[test]
    fn lines_are_at_least_the_thinnest_rule() {
        let mut options = opts();
        let thin = Bx::line(Em(2.0), Em(0.01), &options);
        assert_eq!(sized(&thin), (2.0, 0.01, 0.0));
        options.min_rule_thickness = Em(0.05);
        assert_eq!(
            sized(&Bx::line(Em(2.0), Em(0.01), &options)),
            (2.0, 0.05, 0.0)
        );
    }

    #[test]
    fn vlists_stack_from_the_bottom() {
        // A fraction-like stack: denominator, kern, bar, kern, numerator; bottom 1em down.
        let parts = vec![
            Part::of(rule(1.0, 0.5, 0.25)),
            Part::Kern(Em(0.25)),
            Part::of(rule(1.0, 0.1, 0.0)),
            Part::Kern(Em(0.25)),
            Part::Box {
                child: rule(2.0, 0.5, 0.25),
                dx: Em(0.5),
            },
        ];
        let stack = Bx::vlist(Position::Bottom(Em(1.0)), parts.clone());
        // From -1: denominator to -0.25, kern to 0, bar to 0.1, kern to 0.35, numerator to 1.1.
        assert!(close(sized(&stack), (2.5, 1.1, 1.0)));
        let BoxKind::VList(placed) = &stack.kind else {
            unreachable!("a vlist")
        };
        let raises: Vec<f64> = placed.iter().map(|p| p.raise.0).collect();
        assert!(
            raises
                .iter()
                .zip([-0.75, 0.0, 0.6])
                .all(|(a, b)| (a - b).abs() < 1e-9)
        );
        // The same stack placed by its top, its first baseline, and a shift.
        let top = Bx::vlist(Position::Top(Em(1.1)), parts.clone());
        assert!(close(sized(&top), (2.5, 1.1, 1.0)));
        let first = Bx::vlist(Position::FirstBaseline, parts.clone());
        assert!(close(sized(&first), (2.5, 1.85, 0.25)));
        let shifted = Bx::vlist(Position::Shift(Em(0.5)), parts);
        assert!(close(sized(&shifted), (2.5, 1.35, 0.75)));
    }

    #[test]
    fn kerns_count_toward_a_vlists_extent() {
        let stack = Bx::vlist(
            Position::FirstBaseline,
            vec![Part::of(rule(1.0, 0.5, 0.0)), Part::Kern(Em(0.5))],
        );
        assert_eq!(sized(&stack), (1.0, 1.0, 0.0));
        let below = Bx::vlist(
            Position::Bottom(Em(0.0)),
            vec![Part::Kern(Em(-0.5)), Part::of(rule(1.0, 0.25, 0.0))],
        );
        assert_eq!(sized(&below), (1.0, 0.0, 0.5));
    }

    #[test]
    fn individually_shifted_children() {
        // A subscript 0.2 down and a superscript 0.4 up, as supsub places them.
        let stack = Bx::vlist_shifted(vec![
            (rule(0.5, 0.3, 0.1), Em(0.2), Em(-0.05)),
            (rule(0.4, 0.3, 0.1), Em(-0.4), Em::ZERO),
        ]);
        let BoxKind::VList(placed) = &stack.kind else {
            unreachable!("a vlist")
        };
        let raises: Vec<f64> = placed.iter().map(|p| p.raise.0).collect();
        assert!((raises[0] + 0.2).abs() < 1e-9 && (raises[1] - 0.4).abs() < 1e-9);
        assert!(close(sized(&stack), (0.45, 0.7, 0.3)));
        assert_eq!(sized(&Bx::vlist_shifted(Vec::new())), (0.0, 0.0, 0.0));
    }

    #[test]
    fn scaled_boxes_report_outer_ems() {
        let inner = rule(1.0, 0.5, 0.25);
        let small = Bx::scaled(0.7, inner.clone());
        assert!(close(sized(&small), (0.7, 0.35, 0.175)));
        assert_eq!(Bx::scaled(1.0, inner.clone()), inner);
    }

    #[test]
    fn items_are_placed_absolutely() {
        let x = Bx::glyph("x", Face::MathItalic, Mode::Math, &opts());
        let tree = Bx::hlist(vec![
            Bx::kern(Em(1.0)),
            Bx::scaled(
                0.5,
                Bx::vlist(
                    Position::Bottom(Em(0.0)),
                    vec![
                        Part::Kern(Em(1.0)),
                        Part::Box {
                            child: x,
                            dx: Em(0.2),
                        },
                    ],
                ),
            ),
            rule(1.0, 0.5, 0.25),
        ]);
        let items = tree.items();
        let [
            Item::Glyph {
                x: gx,
                y: gy,
                scale,
                glyph,
            },
            Item::Rule {
                x: rx,
                y: ry,
                width,
                height,
                color,
            },
        ] = items.as_slice()
        else {
            unreachable!("a glyph and a rule: {items:?}")
        };
        assert_eq!(
            (*gx, *gy, *scale, glyph.text.as_ref()),
            (1.1, -0.5, 0.5, "x")
        );
        // The rule follows the scaled box: 1 + (0.2 + 0.57153) * 0.5.
        assert!((rx - (1.0 + (0.2 + 0.57153) * 0.5)).abs() < 1e-9);
        assert_eq!((*ry, *width, *height, *color), (-0.5, 1.0, 0.75, None));
    }
}
