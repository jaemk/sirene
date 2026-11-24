//! Stretchy shapes: the arrows, braces, wide accents, and enclosure lines that grow to a width,
//! as `Bx::path` boxes over the paths of `math::svg_data`.
//!
//! Adapted from KaTeX's `src/stretchy.ts` (`stretchySvg`, `stretchyEnclose`; MIT, Copyright (c)
//! 2013-2020 Khan Academy and other contributors). Used by `accent`, `arrow`, `horiz_brace`,
//! and `enclose`; it lays out no node of its own.
//!
//! KaTeX draws an arrow or a brace as one or more 400em-wide paths in a window that shows only
//! the part that fits, so the heads and corners never distort. Here each window is a `Bx::path`
//! with a `Fit::Crop` of its alignment, so a box of any width has the same few path boxes.

use std::ops::Range;

use crate::math::{
    MathError,
    boxes::{Align, Bx, Fit},
    options::{Em, Options},
    svg_data,
};

/// The width of the view box of an arrow or brace path, in the path's units (1000 per em).
const VIEW_WIDTH: f64 = 400_000.0;

/// What KaTeX's `katexImagesData` holds for a label.
struct Image {
    /// One path (an arrow, cropped to its alignment), two (halves), or three (a brace).
    paths: &'static [&'static str],
    /// The least width in em.
    min_width: f64,
    /// The height of the view box: the height in thousandths of an em.
    view_height: f64,
    /// For one path, which end of it stays in the box.
    align: Option<Align>,
}

const fn image(
    paths: &'static [&'static str],
    min_width: f64,
    view_height: f64,
    align: Option<Align>,
) -> Image {
    Image {
        paths,
        min_width,
        view_height,
        align,
    }
}

/// The image of a label without its backslash.
fn katex_image(label: &str) -> Option<Image> {
    use Align::{Max, Min};
    Some(match label {
        "overrightarrow" | "underrightarrow" => image(&["rightarrow"], 0.888, 522.0, Some(Max)),
        "overleftarrow" | "underleftarrow" => image(&["leftarrow"], 0.888, 522.0, Some(Min)),
        "xrightarrow" => image(&["rightarrow"], 1.469, 522.0, Some(Max)),
        "\\cdrightarrow" => image(&["rightarrow"], 3.0, 522.0, Some(Max)),
        "xleftarrow" => image(&["leftarrow"], 1.469, 522.0, Some(Min)),
        "\\cdleftarrow" => image(&["leftarrow"], 3.0, 522.0, Some(Min)),
        "Overrightarrow" => image(&["doublerightarrow"], 0.888, 560.0, Some(Max)),
        "xRightarrow" => image(&["doublerightarrow"], 1.526, 560.0, Some(Max)),
        "xLeftarrow" => image(&["doubleleftarrow"], 1.526, 560.0, Some(Min)),
        "overleftharpoon" | "xleftharpoonup" => image(&["leftharpoon"], 0.888, 522.0, Some(Min)),
        "xleftharpoondown" => image(&["leftharpoondown"], 0.888, 522.0, Some(Min)),
        "overrightharpoon" | "xrightharpoonup" => image(&["rightharpoon"], 0.888, 522.0, Some(Max)),
        "xrightharpoondown" => image(&["rightharpoondown"], 0.888, 522.0, Some(Max)),
        "xlongequal" => image(&["longequal"], 0.888, 334.0, Some(Min)),
        "\\cdlongequal" => image(&["longequal"], 3.0, 334.0, Some(Min)),
        "xtwoheadleftarrow" => image(&["twoheadleftarrow"], 0.888, 334.0, Some(Min)),
        "xtwoheadrightarrow" => image(&["twoheadrightarrow"], 0.888, 334.0, Some(Max)),

        "overleftrightarrow" | "underleftrightarrow" => {
            image(&["leftarrow", "rightarrow"], 0.888, 522.0, None)
        }
        "overbrace" => image(&["leftbrace", "midbrace", "rightbrace"], 1.6, 548.0, None),
        "underbrace" => image(
            &["leftbraceunder", "midbraceunder", "rightbraceunder"],
            1.6,
            548.0,
            None,
        ),
        "xleftrightarrow" => image(&["leftarrow", "rightarrow"], 1.75, 522.0, None),
        "xLeftrightarrow" => image(&["doubleleftarrow", "doublerightarrow"], 1.75, 560.0, None),
        "xrightleftharpoons" => image(
            &["leftharpoondownplus", "rightharpoonplus"],
            1.75,
            716.0,
            None,
        ),
        "xleftrightharpoons" => image(
            &["leftharpoonplus", "rightharpoondownplus"],
            1.75,
            716.0,
            None,
        ),
        "xhookleftarrow" => image(&["leftarrow", "righthook"], 1.08, 522.0, None),
        "xhookrightarrow" => image(&["lefthook", "rightarrow"], 1.08, 522.0, None),
        "overlinesegment" | "underlinesegment" => {
            image(&["leftlinesegment", "rightlinesegment"], 0.888, 522.0, None)
        }
        "overbracket" => image(&["leftbracketover", "rightbracketover"], 1.6, 440.0, None),
        "underbracket" => image(&["leftbracketunder", "rightbracketunder"], 1.6, 410.0, None),
        "overgroup" => image(&["leftgroup", "rightgroup"], 0.888, 342.0, None),
        "undergroup" => image(&["leftgroupunder", "rightgroupunder"], 0.888, 342.0, None),
        "xmapsto" => image(&["leftmapsto", "rightarrow"], 1.5, 522.0, None),
        "xtofrom" => image(&["leftToFrom", "rightToFrom"], 1.75, 528.0, None),

        // The mhchem arrows.
        "xrightleftarrows" => image(
            &["baraboveleftarrow", "rightarrowabovebar"],
            1.75,
            901.0,
            None,
        ),
        "xrightequilibrium" => image(
            &["baraboveshortleftharpoon", "rightharpoonaboveshortbar"],
            1.75,
            716.0,
            None,
        ),
        "xleftequilibrium" => image(
            &["shortbaraboveleftharpoon", "shortrightharpoonabovebar"],
            1.75,
            716.0,
            None,
        ),
        _ => return None,
    })
}

/// The path data of a name, or KaTeX's error for a label without any.
fn path_data(name: &str, span: &Range<usize>) -> Result<&'static str, MathError> {
    svg_data::path(name)
        .ok_or_else(|| MathError::new(format!("No SVG data for \"{name}\"."), span.clone()))
}

/// The wide accents, whose image depends on how many characters they cover.
fn is_wide_accent(label: &str) -> bool {
    matches!(label, "widehat" | "widecheck" | "widetilde" | "utilde")
}

/// The image of a wide accent over `chars` characters: its path, the view box's width and height,
/// and the height in em (KaTeX's four sizes of each).
fn wide_accent(label: &str, chars: usize) -> (&'static str, f64, f64, f64) {
    let hat = label != "widetilde" && label != "utilde";
    let check = label == "widecheck";
    if chars > 5 {
        return match (hat, check) {
            (true, true) => ("widecheck4", 2364.0, 420.0, 0.42),
            (true, false) => ("widehat4", 2364.0, 420.0, 0.42),
            (false, _) => ("tilde4", 2340.0, 312.0, 0.34),
        };
    }
    // Wider bases take taller images.
    let index = match chars {
        0 | 1 => 1,
        2 | 3 => 2,
        _ => 3,
    };
    match (hat, check, index) {
        (true, true, 1) => ("widecheck1", 1062.0, 239.0, 0.24),
        (true, true, 2) => ("widecheck2", 2364.0, 300.0, 0.3),
        (true, true, _) => ("widecheck3", 2364.0, 360.0, 0.3),
        (true, false, 1) => ("widehat1", 1062.0, 239.0, 0.24),
        (true, false, 2) => ("widehat2", 2364.0, 300.0, 0.3),
        (true, false, _) => ("widehat3", 2364.0, 360.0, 0.3),
        (false, _, 1) => ("tilde1", 600.0, 260.0, 0.26),
        (false, _, 2) => ("tilde2", 1033.0, 286.0, 0.286),
        (false, _, _) => ("tilde3", 2339.0, 306.0, 0.3),
    }
}

/// A stretchy shape of a label (without its backslash, as KaTeX's `label.slice(1)`) over a
/// container `width` wide, as KaTeX's `stretchySvg`: its height from the image, no depth, and at
/// least the image's minimum width.
///
/// `accent_chars` is the number of characters a wide accent covers (KaTeX's `numChars`, one for
/// a base that is not a group); `None` for anything else, which looks the label up as an arrow,
/// a brace, or a bracket. A label with no image is an error, as KaTeX's.
pub(super) fn svg_span<'t>(
    label: &str,
    accent_chars: Option<usize>,
    width: Em,
    options: &Options<'t>,
    span: Range<usize>,
) -> Result<Bx<'t>, MathError> {
    let name = label.strip_prefix('\\').unwrap_or(label);
    if let Some(chars) = accent_chars
        && is_wide_accent(name)
    {
        let (path, view_width, view_height, height) = wide_accent(name, chars);
        let data = path_data(path, &span)?;
        return Ok(Bx::path(
            data,
            [0.0, 0.0, view_width, view_height],
            Fit::Stretch,
            (width, Em(height), Em::ZERO),
            options,
        ));
    }
    let Some(image) = katex_image(name) else {
        return Err(MathError::new(format!("No SVG data for \"{name}\"."), span));
    };
    let height = Em(image.view_height / 1000.0);
    let width = width.max(Em(image.min_width));
    let view_box = [0.0, 0.0, VIEW_WIDTH, image.view_height];
    // One window per path: its alignment, and where it starts and how wide it is as a fraction
    // of the box (KaTeX's `hide-tail`, `halfarrow-*`, and `brace-*` classes).
    let windows: &[(Align, f64, f64)] = match image.paths.len() {
        1 => &[(image.align.unwrap_or(Align::Min), 0.0, 1.0)],
        2 => &[(Align::Min, 0.0, 0.502), (Align::Max, 0.498, 0.502)],
        _ => &[
            (Align::Min, 0.0, 0.251),
            (Align::Mid, 0.25, 0.5),
            (Align::Max, 0.749, 0.251),
        ],
    };
    let mut pieces = Vec::with_capacity(windows.len());
    for (path, &(align, start, share)) in image.paths.iter().zip(windows) {
        let data = path_data(path, &span)?;
        let piece = Bx::path(
            data,
            view_box,
            Fit::Crop(align),
            (width * share, height, Em::ZERO),
            options,
        );
        pieces.push((piece, Em::ZERO, width * start));
    }
    if pieces.len() == 1
        && let Some((only, ..)) = pieces.pop()
    {
        return Ok(only);
    }
    // The windows overlap a little, on one baseline.
    Ok(Bx::vlist_shifted(pieces))
}

/// What `enclose_span` draws around a box: its size, the padding around the content, the
/// border and the colors.
#[allow(dead_code)]
pub(super) struct Enclosure<'t> {
    /// The width of the enclosed content with its horizontal padding.
    pub width: Em,
    /// The height plus depth of the enclosed content.
    pub content: Em,
    pub top_pad: Em,
    pub bottom_pad: Em,
    /// The border's thickness: `\fbox`, `\fcolorbox`, and the top and right of `\angl`.
    pub rule: Em,
    /// The fill behind the content (`\colorbox`, `\fcolorbox`).
    pub background: Option<&'t str>,
    /// The border's color, where it is not the options' (`\fcolorbox`).
    pub border: Option<&'t str>,
}

/// A number for path data, zero if it is not finite.
fn coord(x: f64) -> f64 {
    if x.is_finite() { x } else { 0.0 }
}

/// A rectangle's path data, from the origin.
fn rectangle(w: f64, h: f64) -> String {
    format!("M0 0H{w}V{h}H0z")
}

/// A path box of `data` over `width` by `height` em, whose view box is in em.
fn em_path<'t>(data: String, (width, height): (f64, f64), options: &Options<'t>) -> Bx<'t> {
    Bx::path(
        data,
        [0.0, 0.0, width, height],
        Fit::Stretch,
        (Em(width), Em(height), Em::ZERO),
        options,
    )
}

/// The image of `\cancel`, `\bcancel`, `\xcancel`, `\fbox`, `\colorbox`, `\fcolorbox`, or `\angl`
/// (the label without its backslash), as KaTeX's `stretchyEnclose`: a box as wide as the content
/// and as high as its height and padding, with no depth.
///
/// KaTeX draws the cancel lines as SVG strokes and the boxes with CSS borders and backgrounds;
/// the box tree has only filled paths, so a stroke is the thin parallelogram it covers (in a
/// view box of em, so a stroke has its thickness at any shape) and a border is a ring. A box of
/// any other label is empty.
#[allow(dead_code)]
pub(super) fn enclose_span<'t>(label: &str, spec: &Enclosure<'t>, options: &Options<'t>) -> Bx<'t> {
    let w = coord(spec.width.0).max(0.0);
    let h = coord((spec.content + spec.top_pad + spec.bottom_pad).0).max(0.0);
    let size = (w, h);
    let rule = coord(spec.rule.0).max(0.0);
    let colored = |color: Option<&'t str>| match color {
        Some(color) => options.with_color(color),
        None => *options,
    };
    match label {
        "cancel" | "bcancel" | "xcancel" => {
            // A stroke of 0.046em from corner to corner, with flat ends.
            let half = 0.023;
            let length = w.hypot(h);
            if length == 0.0 {
                return Bx::space(Em(w), Em(h), Em::ZERO);
            }
            let mut data = String::new();
            let mut stroke = |(ax, ay): (f64, f64), (bx, by): (f64, f64)| {
                // Half the thickness along the normal, which turns the same way for each line
                // so the two fill alike where they cross.
                let (nx, ny) = (-(by - ay) / length * half, (bx - ax) / length * half);
                data.push_str(&format!(
                    "M{} {}L{} {}L{} {}L{} {}z",
                    ax + nx,
                    ay + ny,
                    bx + nx,
                    by + ny,
                    bx - nx,
                    by - ny,
                    ax - nx,
                    ay - ny
                ));
            };
            if label != "cancel" {
                stroke((0.0, 0.0), (w, h));
            }
            if label != "bcancel" {
                stroke((0.0, h), (w, 0.0));
            }
            em_path(data, size, options)
        }
        "fbox" | "colorbox" | "fcolorbox" => {
            let mut layers = Vec::with_capacity(2);
            if let Some(background) = spec.background {
                let fill = colored(Some(background));
                layers.push((em_path(rectangle(w, h), size, &fill), Em::ZERO, Em::ZERO));
            }
            if rule > 0.0 && label != "colorbox" {
                let ring = if w > 2.0 * rule && h > 2.0 * rule {
                    // The inner rectangle runs the other way, which leaves a hole.
                    format!(
                        "{}M{rule} {rule}V{}H{}V{rule}z",
                        rectangle(w, h),
                        h - rule,
                        w - rule
                    )
                } else {
                    rectangle(w, h)
                };
                let border = colored(spec.border);
                layers.push((em_path(ring, size, &border), Em::ZERO, Em::ZERO));
            }
            if layers.is_empty() {
                return Bx::space(Em(w), Em(h), Em::ZERO);
            }
            Bx::vlist_shifted(layers)
        }
        "angl" => {
            // A border along the top and one down the right side.
            let data = format!("M0 0H{w}V{h}H{}V{}H0z", (w - rule).max(0.0), rule.min(h));
            em_path(data, size, options)
        }
        _ => Bx::space(Em(w), Em(h), Em::ZERO),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::boxes::Item;

    fn opts() -> Options<'static> {
        Options::new(false)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    /// The rectangle, view box, fit, and data of each path in a box.
    fn paths(bx: &Bx<'_>) -> Vec<([f64; 4], [f64; 4], Fit, String)> {
        bx.items()
            .into_iter()
            .filter_map(|item| match item {
                Item::Path {
                    x,
                    y,
                    width,
                    height,
                    path,
                    ..
                } => Some((
                    [x, y, width, height],
                    path.view_box,
                    path.fit,
                    path.data.into_owned(),
                )),
                _ => None,
            })
            .collect()
    }

    fn span(label: &str, chars: Option<usize>, width: f64) -> Bx<'static> {
        svg_span(label, chars, Em(width), &opts(), 0..0).unwrap()
    }

    #[test]
    fn one_path_is_cropped_to_its_end() {
        let bx = span("\\overrightarrow", None, 2.0);
        assert_eq!((bx.width.0, bx.height.0, bx.depth.0), (2.0, 0.522, 0.0));
        let drawn = paths(&bx);
        let [(rect, view_box, fit, data)] = drawn.as_slice() else {
            panic!("one path: {drawn:?}");
        };
        assert_eq!(*rect, [0.0, -0.522, 2.0, 0.522]);
        assert_eq!(*view_box, [0.0, 0.0, 400_000.0, 522.0]);
        assert_eq!(*fit, Fit::Crop(Align::Max));
        assert_eq!(data, svg_data::path("rightarrow").unwrap());
        let left = paths(&span("\\overleftarrow", None, 2.0));
        assert_eq!(left[0].2, Fit::Crop(Align::Min));
    }

    #[test]
    fn a_narrow_container_gets_the_minimum_width() {
        let bx = span("\\xrightarrow", None, 0.2);
        assert_eq!(bx.width.0, 1.469);
        assert_eq!(span("\\xlongequal", None, 0.2).width.0, 0.888);
        assert_eq!(span("\\overbrace", None, 0.2).width.0, 1.6);
        // The height is that of the image: 0.334em for the equals sign.
        assert_eq!(span("\\xlongequal", None, 0.2).height.0, 0.334);
    }

    #[test]
    fn two_paths_are_halves_that_overlap() {
        let bx = span("\\overbracket", None, 3.0);
        assert_eq!((bx.width.0, bx.height.0, bx.depth.0), (3.0, 0.44, 0.0));
        let drawn = paths(&bx);
        let [
            (left, view_box, left_fit, left_data),
            (right, _, right_fit, right_data),
        ] = drawn.as_slice()
        else {
            panic!("two paths: {drawn:?}");
        };
        assert!(close(left[0], 0.0) && close(left[2], 3.0 * 0.502));
        assert!(close(right[0], 3.0 * 0.498) && close(right[2], 3.0 * 0.502));
        assert!(close(left[1], -0.44) && close(right[1], -0.44));
        assert_eq!(*view_box, [0.0, 0.0, 400_000.0, 440.0]);
        assert_eq!(*left_fit, Fit::Crop(Align::Min));
        assert_eq!(*right_fit, Fit::Crop(Align::Max));
        assert_eq!(left_data, svg_data::path("leftbracketover").unwrap());
        assert_eq!(right_data, svg_data::path("rightbracketover").unwrap());
    }

    #[test]
    fn three_paths_are_the_ends_and_the_middle_of_a_brace() {
        let bx = span("\\underbrace", None, 4.0);
        assert_eq!((bx.width.0, bx.height.0, bx.depth.0), (4.0, 0.548, 0.0));
        let drawn = paths(&bx);
        let [(left, ..), (mid, ..), (right, ..)] = drawn.as_slice() else {
            panic!("three paths: {drawn:?}");
        };
        assert!(close(left[0], 0.0) && close(left[2], 4.0 * 0.251));
        assert!(close(mid[0], 1.0) && close(mid[2], 2.0));
        assert!(close(right[0], 4.0 * 0.749) && close(right[2], 4.0 * 0.251));
        assert_eq!(
            [drawn[0].2, drawn[1].2, drawn[2].2],
            [
                Fit::Crop(Align::Min),
                Fit::Crop(Align::Mid),
                Fit::Crop(Align::Max)
            ]
        );
        assert_eq!(drawn[1].3, svg_data::path("midbraceunder").unwrap());
    }

    #[test]
    fn wide_accents_choose_an_image_by_the_characters_they_cover() {
        let one = span("\\widehat", Some(1), 0.5);
        assert_eq!((one.width.0, one.height.0, one.depth.0), (0.5, 0.24, 0.0));
        let drawn = paths(&one);
        assert_eq!(drawn[0].1, [0.0, 0.0, 1062.0, 239.0]);
        assert_eq!(drawn[0].2, Fit::Stretch);
        assert_eq!(drawn[0].3, svg_data::path("widehat1").unwrap());
        assert_eq!(span("\\widehat", Some(3), 1.0).height.0, 0.3);
        assert_eq!(span("\\widecheck", Some(5), 1.0).height.0, 0.3);
        assert_eq!(span("\\widehat", Some(9), 1.0).height.0, 0.42);
        assert_eq!(paths(&span("\\widecheck", Some(9), 1.0))[0].1[2], 2364.0);
        let tilde = span("\\widetilde", Some(2), 1.0);
        assert_eq!(tilde.height.0, 0.286);
        assert_eq!(paths(&tilde)[0].3, svg_data::path("tilde2").unwrap());
        let long = span("\\utilde", Some(7), 1.0);
        assert_eq!(long.height.0, 0.34);
        assert_eq!(paths(&long)[0].1, [0.0, 0.0, 2340.0, 312.0]);
        // Wide accents have no minimum width, and a base of no characters takes the smallest.
        assert_eq!(span("\\widehat", Some(0), 0.1).width.0, 0.1);
        assert_eq!(span("\\widehat", Some(0), 0.1).height.0, 0.24);
    }

    #[test]
    fn every_image_has_its_path_data() {
        for label in [
            "overrightarrow",
            "overleftarrow",
            "underrightarrow",
            "underleftarrow",
            "xrightarrow",
            "xleftarrow",
            "Overrightarrow",
            "xRightarrow",
            "xLeftarrow",
            "overleftharpoon",
            "xleftharpoonup",
            "xleftharpoondown",
            "overrightharpoon",
            "xrightharpoonup",
            "xrightharpoondown",
            "xlongequal",
            "xtwoheadleftarrow",
            "xtwoheadrightarrow",
            "overleftrightarrow",
            "overbrace",
            "underbrace",
            "underleftrightarrow",
            "xleftrightarrow",
            "xLeftrightarrow",
            "xrightleftharpoons",
            "xleftrightharpoons",
            "xhookleftarrow",
            "xhookrightarrow",
            "overlinesegment",
            "underlinesegment",
            "overbracket",
            "underbracket",
            "overgroup",
            "undergroup",
            "xmapsto",
            "xtofrom",
            "xrightleftarrows",
            "xrightequilibrium",
            "xleftequilibrium",
        ] {
            let bx = svg_span(label, None, Em(1.0), &opts(), 0..0)
                .unwrap_or_else(|e| panic!("{label}: {e:?}"));
            assert!(bx.height.0 > 0.0 && !paths(&bx).is_empty(), "{label}");
        }
    }

    #[test]
    fn an_unknown_label_is_an_error() {
        let err = svg_span("\\xnothing", None, Em(1.0), &opts(), 3..9).unwrap_err();
        assert_eq!(err.message, "No SVG data for \"xnothing\".");
        assert_eq!(err.span, 3..9);
        // A wide accent without a base is looked up as an arrow and has no image.
        assert!(svg_span("\\widehat", None, Em(1.0), &opts(), 0..0).is_err());
    }

    #[test]
    fn a_huge_width_gives_the_same_boxes() {
        for (label, count) in [
            ("\\xrightarrow", 1),
            ("\\xleftrightarrow", 2),
            ("\\overbrace", 3),
        ] {
            let bx = span(label, None, 1000.0);
            assert_eq!(bx.width.0, 1000.0);
            assert_eq!(paths(&bx).len(), count, "{label}");
        }
    }

    fn enclosure(width: f64, content: f64, pad: f64) -> Enclosure<'static> {
        Enclosure {
            width: Em(width),
            content: Em(content),
            top_pad: Em(pad),
            bottom_pad: Em(pad),
            rule: Em(0.04),
            background: None,
            border: None,
        }
    }

    #[test]
    fn cancel_lines_are_strokes_across_the_box() {
        let spec = enclosure(2.0, 1.0, 0.2);
        let bx = enclose_span("cancel", &spec, &opts());
        assert_eq!((bx.width.0, bx.height.0, bx.depth.0), (2.0, 1.4, 0.0));
        let one = paths(&bx);
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].1, [0.0, 0.0, 2.0, 1.4]);
        assert_eq!(one[0].2, Fit::Stretch);
        // One subpath for `\cancel` and `\bcancel`, two for `\xcancel`.
        let subpaths = |label: &str| {
            paths(&enclose_span(label, &spec, &opts()))[0]
                .3
                .matches('z')
                .count()
        };
        assert_eq!(
            (subpaths("cancel"), subpaths("bcancel"), subpaths("xcancel")),
            (1, 1, 2)
        );
    }

    #[test]
    fn boxes_are_rings_and_fills() {
        let spec = enclosure(2.0, 1.0, 0.1);
        let ring = paths(&enclose_span("fbox", &spec, &opts()));
        assert_eq!(ring.len(), 1);
        assert_eq!(ring[0].3.matches('M').count(), 2, "{}", ring[0].3);
        assert!(close(ring[0].0[1], -1.2) && close(ring[0].0[3], 1.2));
        assert_eq!((ring[0].0[0], ring[0].0[2]), (0.0, 2.0));
        // A background goes behind the border, in its own color.
        let both = Enclosure {
            background: Some("red"),
            border: Some("blue"),
            ..spec
        };
        let filled = enclose_span("fcolorbox", &both, &opts());
        let items = filled.items();
        let colors: Vec<_> = items
            .iter()
            .filter_map(|item| match item {
                Item::Path { path, .. } => Some(path.color),
                _ => None,
            })
            .collect();
        assert_eq!(colors, [Some("red"), Some("blue")]);
        // `\colorbox` has the fill alone.
        let fill = enclose_span(
            "colorbox",
            &Enclosure {
                rule: Em::ZERO,
                ..both
            },
            &opts(),
        );
        assert_eq!(paths(&fill).len(), 1);
        // A degenerate box fills instead of inverting the ring.
        let thin = paths(&enclose_span("fbox", &enclosure(0.05, 0.0, 0.0), &opts()));
        assert_eq!(thin[0].3.matches('M').count(), 1);
    }

    #[test]
    fn an_actuarial_angle_has_a_top_and_a_right_border() {
        let bx = enclose_span("angl", &enclosure(1.0, 0.5, 0.0), &opts());
        let drawn = paths(&bx);
        assert_eq!(drawn.len(), 1);
        assert_eq!(drawn[0].3, "M0 0H1V0.5H0.96V0.04H0z");
    }

    #[test]
    fn nothing_non_finite_reaches_the_path_data() {
        let bad = Enclosure {
            width: Em(f64::NAN),
            content: Em(f64::INFINITY),
            ..enclosure(1.0, 1.0, 0.0)
        };
        for label in ["cancel", "xcancel", "fbox", "angl", "nothing"] {
            let bx = enclose_span(label, &bad, &opts());
            assert!(bx.width.0.is_finite() && bx.height.0.is_finite(), "{label}");
            assert!(
                paths(&bx)
                    .iter()
                    .all(|p| !p.3.contains("NaN") && !p.3.contains("inf")),
                "{label}"
            );
        }
    }
}
