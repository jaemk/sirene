//! `style`, `classDef`, and `linkStyle` declarations applied to drawing (DISPLA-6).

use super::Color;
use crate::flowchart::{ClassDef, Style};

/// Style overrides; `None` keeps the theme's value.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct Resolved {
    pub fill: Option<Color>,
    pub stroke: Option<Color>,
    pub stroke_width: Option<f32>,
    pub dash: Option<Option<(f32, f32)>>,
    pub color: Option<Color>,
}

/// Applies `base`, then the named classes in order, then the inline styles; later declarations
/// win.
pub(super) fn resolve(
    base: &[Style],
    classes: &[&str],
    styles: &[Style],
    defs: &[ClassDef],
) -> Resolved {
    let mut out = Resolved::default();
    apply_all(&mut out, base);
    for name in classes {
        for def in defs.iter().filter(|d| d.name == *name) {
            apply_all(&mut out, &def.styles);
        }
    }
    apply_all(&mut out, styles);
    out
}

fn apply_all(out: &mut Resolved, styles: &[Style]) {
    for s in styles {
        apply(out, &s.property, s.value.trim());
    }
}

fn apply(out: &mut Resolved, property: &str, value: &str) {
    match property.trim() {
        "fill" | "background" | "background-color" => {
            if let Some(c) = parse_color(value) {
                out.fill = Some(c);
            }
        }
        "stroke" => {
            if let Some(c) = parse_color(value) {
                out.stroke = Some(c);
            }
        }
        "color" => {
            if let Some(c) = parse_color(value) {
                out.color = Some(c);
            }
        }
        "stroke-width" => {
            if let Some(w) = parse_length(value) {
                out.stroke_width = Some(w);
            }
        }
        "stroke-dasharray" => {
            let lengths: Vec<f32> = value
                .split([',', ' '])
                .filter(|s| !s.is_empty())
                .map_while(parse_length)
                .collect();
            // All-zero lengths draw a solid line, as in CSS, and so does a period
            // too fine to see, which would otherwise cost the renderers one
            // segment per dash.
            let dashed = |d: f32, g: f32| Some((d + g >= MIN_DASH_PERIOD).then_some((d, g)));
            out.dash = match lengths[..] {
                _ if value == "none" => Some(None),
                [d] => dashed(d, d),
                [d, g, ..] => dashed(d, g),
                [] => out.dash,
            };
        }
        _ => {}
    }
}

/// The shortest dash plus gap drawn as dashes; shorter patterns are solid.
const MIN_DASH_PERIOD: f32 = 1.0;

/// A non-negative length in pixels, with an optional `px` unit.
fn parse_length(value: &str) -> Option<f32> {
    let n = value.trim().strip_suffix("px").unwrap_or(value).trim();
    n.parse::<f32>().ok().filter(|v| v.is_finite() && *v >= 0.0)
}

const NAMED: &[(&str, u32)] = &[
    ("black", 0x000000),
    ("white", 0xffffff),
    ("red", 0xff0000),
    ("green", 0x008000),
    ("lime", 0x00ff00),
    ("blue", 0x0000ff),
    ("yellow", 0xffff00),
    ("orange", 0xffa500),
    ("purple", 0x800080),
    ("pink", 0xffc0cb),
    ("gray", 0x808080),
    ("grey", 0x808080),
    ("lightgray", 0xd3d3d3),
    ("lightgrey", 0xd3d3d3),
    ("darkgray", 0xa9a9a9),
    ("darkgrey", 0xa9a9a9),
    ("silver", 0xc0c0c0),
    ("navy", 0x000080),
    ("teal", 0x008080),
    ("cyan", 0x00ffff),
    ("aqua", 0x00ffff),
    ("magenta", 0xff00ff),
    ("fuchsia", 0xff00ff),
    ("maroon", 0x800000),
    ("olive", 0x808000),
    ("brown", 0xa52a2a),
    ("gold", 0xffd700),
    ("coral", 0xff7f50),
    ("salmon", 0xfa8072),
    ("tomato", 0xff6347),
    ("violet", 0xee82ee),
    ("indigo", 0x4b0082),
    ("lightblue", 0xadd8e6),
    ("lightgreen", 0x90ee90),
    ("darkblue", 0x00008b),
    ("darkgreen", 0x006400),
    ("darkred", 0x8b0000),
];

/// Parses a CSS color: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, a common
/// name, or `none`/`transparent`.
pub fn parse_color(value: &str) -> Option<Color> {
    let v = value.trim().to_ascii_lowercase();
    if let Some(hex) = v.strip_prefix('#') {
        return parse_hex(hex);
    }
    if v == "none" || v == "transparent" {
        return Some(Color::rgb(0).with_alpha(0));
    }
    if let Some(args) = v
        .strip_prefix("rgba(")
        .or_else(|| v.strip_prefix("rgb("))
        .and_then(|a| a.strip_suffix(')'))
    {
        return parse_rgb_args(args);
    }
    NAMED
        .iter()
        .find(|(name, _)| *name == v)
        .map(|&(_, hex)| Color::rgb(hex))
}

fn parse_hex(hex: &str) -> Option<Color> {
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let digit = |i: usize| u8::from_str_radix(&hex[i..=i], 16).ok();
    let pair = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    let (r, g, b, a) = match hex.len() {
        3 | 4 => (
            digit(0)? * 17,
            digit(1)? * 17,
            digit(2)? * 17,
            if hex.len() == 4 { digit(3)? * 17 } else { 255 },
        ),
        6 | 8 => (
            pair(0)?,
            pair(2)?,
            pair(4)?,
            if hex.len() == 8 { pair(6)? } else { 255 },
        ),
        _ => return None,
    };
    Some(Color { r, g, b, a })
}

fn parse_rgb_args(args: &str) -> Option<Color> {
    let parts: Vec<&str> = args.split(',').map(str::trim).collect();
    let channel = |s: &str| {
        s.parse::<f32>()
            .ok()
            .map(|v| v.clamp(0.0, 255.0).round() as u8)
    };
    let (r, g, b) = match parts[..] {
        [r, g, b] | [r, g, b, _] => (channel(r)?, channel(g)?, channel(b)?),
        _ => return None,
    };
    let a = match parts.get(3) {
        Some(a) => (a.parse::<f32>().ok()?.clamp(0.0, 1.0) * 255.0).round() as u8,
        None => 255,
    };
    Some(Color { r, g, b, a })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn style(property: &str, value: &str) -> Style {
        Style {
            property: property.into(),
            value: value.into(),
        }
    }

    #[test]
    fn colors() {
        let c = |r, g, b, a| Some(Color { r, g, b, a });
        assert_eq!(parse_color("#f96"), c(0xff, 0x99, 0x66, 255));
        assert_eq!(parse_color("#f968"), c(0xff, 0x99, 0x66, 0x88));
        assert_eq!(parse_color(" #FF9966 "), c(0xff, 0x99, 0x66, 255));
        assert_eq!(parse_color("#ff996680"), c(0xff, 0x99, 0x66, 0x80));
        assert_eq!(parse_color("rgb(1, 2, 300)"), c(1, 2, 255, 255));
        assert_eq!(parse_color("rgba(1,2,3,0.5)"), c(1, 2, 3, 128));
        assert_eq!(parse_color("Red"), c(255, 0, 0, 255));
        assert_eq!(parse_color("none"), c(0, 0, 0, 0));
        assert_eq!(parse_color("transparent"), c(0, 0, 0, 0));
        for bad in [
            "#ff",
            "#fffff",
            "#ggg",
            "#é12",
            "rgb(1,2)",
            "rgb(a,2,3)",
            "rgba(1,2,3,x)",
            "nope",
        ] {
            assert_eq!(parse_color(bad), None, "{bad}");
        }
    }

    #[test]
    fn later_declarations_win() {
        let defs = vec![
            ClassDef {
                name: "a".into(),
                styles: vec![style("fill", "red"), style("stroke", "blue")],
            },
            ClassDef {
                name: "b".into(),
                styles: vec![style("fill", "green"), style("stroke-width", "3px")],
            },
        ];
        let r = resolve(
            &[style("fill", "white"), style("color", "red")],
            &["a", "b", "missing"],
            &[style("stroke", "#000"), style("color", "white")],
            &defs,
        );
        assert_eq!(r.fill, parse_color("green"));
        assert_eq!(r.stroke, parse_color("#000"));
        assert_eq!(r.stroke_width, Some(3.0));
        assert_eq!(r.color, parse_color("white"));
        assert_eq!(r.dash, None);
    }

    #[test]
    fn invalid_values_are_ignored() {
        let r = resolve(
            &[],
            &[],
            &[
                style("fill", "red"),
                style("fill", "bogus"),
                style("stroke", "bogus"),
                style("color", "bogus"),
                style("stroke-width", "-1"),
                style("stroke-width", "wide"),
                style("font-size", "20px"),
            ],
            &[],
        );
        assert_eq!(
            r,
            Resolved {
                fill: parse_color("red"),
                ..Resolved::default()
            }
        );
    }

    #[test]
    fn dash_arrays() {
        let dash = |v: &str| resolve(&[], &[], &[style("stroke-dasharray", v)], &[]).dash;
        assert_eq!(dash("5 3"), Some(Some((5.0, 3.0))));
        assert_eq!(dash("5px,3px,1"), Some(Some((5.0, 3.0))));
        assert_eq!(dash("4"), Some(Some((4.0, 4.0))));
        assert_eq!(dash("none"), Some(None));
        assert_eq!(dash("0"), Some(None));
        assert_eq!(dash("0 0"), Some(None));
        assert_eq!(dash("x"), None);
        // Too fine to see: solid.
        assert_eq!(dash("0.0001"), Some(None));
        assert_eq!(dash("0.4 0.5"), Some(None));
        assert_eq!(dash("0.5 0.5"), Some(Some((0.5, 0.5))));
        assert_eq!(dash("0.5"), Some(Some((0.5, 0.5))));
    }
}
