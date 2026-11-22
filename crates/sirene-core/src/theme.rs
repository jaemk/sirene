//! A diagram's own theme (MTHEME-1): `theme` and `themeVariables` from its frontmatter `config:`
//! or an `%%{init: ...}%%` directive, resolved to drawing colors as Mermaid 12.3's themes resolve
//! them. The palettes and the color derivations are ported from Mermaid's `src/themes/`
//! (Copyright (c) 2014-2022 Knut Sveidqvist; MIT) and khroma's `adjust`, `lighten`, `darken`,
//! and `invert`; see THIRD-PARTY-NOTICES.

use std::ops::Range;

use rowan::{TextRange, TextSize};

use crate::diagnostic::Diagnostic;
use crate::directive::Part::Key;
use crate::scene::{Color, Theme, parse_color};
use crate::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

/// Mermaid's theme names.
pub const NAMES: &[&str] = &[
    "default",
    "base",
    "dark",
    "forest",
    "neutral",
    "neo",
    "neo-dark",
    "redux",
    "redux-dark",
    "redux-color",
    "redux-dark-color",
];

/// The theme settings a diagram's source gives.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ThemeConfig {
    /// The theme name and its byte range in the source.
    pub name: Option<(String, Range<usize>)>,
    /// `themeVariables` entries, in source order; later ones win.
    pub variables: Vec<(String, String)>,
    /// The byte ranges of `init` directives that could not be read, malformed or nested too
    /// deep (LANGS-8); their settings are ignored.
    pub unread: Vec<Range<usize>>,
}

impl ThemeConfig {
    /// Reads the frontmatter `config:` and then every `%%{init}%%` directive, which win.
    pub fn read(src: &str) -> ThemeConfig {
        crate::diagram::read_theme(src)
    }

    pub(crate) fn from_syntax(root: &SyntaxNode) -> ThemeConfig {
        let mut out = ThemeConfig::default();
        for token in root
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
        {
            match token.kind() {
                SyntaxKind::FRONTMATTER => {
                    frontmatter(
                        token.text(),
                        usize::from(token.text_range().start()),
                        &mut out,
                    );
                }
                SyntaxKind::DIRECTIVE => directive(&token, &mut out),
                _ => {}
            }
        }
        out
    }

    /// Warnings for settings Mermaid ignores: an unknown theme name, and `init` directives that
    /// could not be read.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let text_range = |r: &Range<usize>| {
            TextRange::new(TextSize::new(r.start as u32), TextSize::new(r.end as u32))
        };
        let mut out: Vec<Diagnostic> = self
            .unread
            .iter()
            .map(|r| {
                Diagnostic::warning(
                    text_range(r),
                    "this `init` directive could not be read; its settings are ignored",
                )
            })
            .collect();
        if let Some((name, range)) = &self.name
            && !NAMES.contains(&name.as_str())
        {
            out.push(Diagnostic::warning(
                text_range(range),
                format!(
                    "`{}` is not a Mermaid theme; expected one of {}",
                    name.escape_debug(),
                    NAMES.join(", ")
                ),
            ));
        }
        out
    }

    /// `base` with this diagram's theme and variables applied. With no theme name the variables
    /// apply to `base`'s colors as Mermaid's `default` theme applies them to its own.
    pub fn apply(&self, base: &Theme) -> Theme {
        let name = self.name.as_ref().map(|(n, _)| n.as_str());
        if name.is_none_or(|n| !NAMES.contains(&n)) && self.variables.is_empty() {
            return *base;
        }
        let var = |key: &str| {
            self.variables
                .iter()
                .rev()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str())
        };
        let color = |key: &str| var(key).and_then(parse_color);
        let dark = var("darkMode") == Some("true");
        let name = name.filter(|n| NAMES.contains(n));
        let p = match name {
            None => classic(&color, &from_theme(base)),
            Some(n) => resolve(n, &color, dark),
        };
        // Without a theme name, chart colors change only when the diagram sets one; the rest
        // are then the `default` theme's, as in Mermaid.
        let chart_var = |k: &str| {
            let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
            k.strip_prefix("cScale").is_some_and(digits)
                || k.strip_prefix("pie").is_some_and(digits)
        };
        let charts = match name {
            Some(n) => Some(charts(n, &color, dark, p.node_fill)),
            None if self.variables.iter().any(|(k, _)| chart_var(k)) => {
                Some(charts("default", &color, dark, p.node_fill))
            }
            None => None,
        };
        let mut t = Theme {
            background: p.background,
            text: p.text,
            node_fill: p.node_fill,
            node_stroke: p.node_stroke,
            edge: p.edge,
            edge_label_fill: p.edge_label_fill,
            cluster_fill: p.cluster_fill,
            cluster_stroke: p.cluster_stroke,
            scale: charts.as_ref().map(|c| c.scale).or(base.scale),
            pie: charts.as_ref().map(|c| c.pie).or(base.pie),
            ..*base
        };
        if let Some(size) = var("fontSize").and_then(font_size) {
            t.line_height = base.line_height * size / base.font_size;
            t.font_size = size;
        }
        t
    }
}

fn font_size(v: &str) -> Option<f32> {
    let v = v.trim();
    let n = v.strip_suffix("px").unwrap_or(v).trim();
    n.parse::<f32>().ok().filter(|s| (1.0..=200.0).contains(s))
}

/// The colors sirene draws with, by Mermaid variable.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Palette {
    background: Color,
    /// `nodeTextColor`, or `textColor`.
    text: Color,
    /// `mainBkg`.
    node_fill: Color,
    /// `nodeBorder`.
    node_stroke: Color,
    /// `lineColor`.
    edge: Color,
    /// `edgeLabelBackground`.
    edge_label_fill: Color,
    /// `clusterBkg`.
    cluster_fill: Color,
    /// `clusterBorder`.
    cluster_stroke: Color,
}

/// The starting values of one of Mermaid's classic themes (`default`, `forest`, `neutral`,
/// `dark`), whose flowchart colors come from `mainBkg`, `border1`, `secondBkg`, and `border2`.
struct Classic {
    background: Color,
    text: Color,
    main_bkg: Color,
    border1: Color,
    second_bkg: Color,
    border2: Color,
    line: Color,
    label_bkg: Color,
}

fn from_theme(t: &Theme) -> Classic {
    Classic {
        background: t.background,
        text: t.text,
        main_bkg: t.node_fill,
        border1: t.node_stroke,
        second_bkg: t.cluster_fill,
        border2: t.cluster_stroke,
        line: t.edge,
        label_bkg: t.edge_label_fill,
    }
}

/// A classic theme with `color`'s overrides, as its `updateColors` derives the flowchart colors.
fn classic(color: &dyn Fn(&str) -> Option<Color>, c: &Classic) -> Palette {
    let v = |key: &str, default: Color| color(key).unwrap_or(default);
    Palette {
        background: v("background", c.background),
        text: color("nodeTextColor").unwrap_or(v("textColor", c.text)),
        node_fill: v("mainBkg", c.main_bkg),
        node_stroke: color("nodeBorder").unwrap_or(v("border1", c.border1)),
        edge: v("lineColor", c.line),
        edge_label_fill: color("edgeLabelBackground").unwrap_or(v("labelBackground", c.label_bkg)),
        cluster_fill: color("clusterBkg").unwrap_or(v("secondBkg", c.second_bkg)),
        cluster_stroke: color("clusterBorder").unwrap_or(v("border2", c.border2)),
    }
}

/// The values a `base`-style theme's constructor sets before `updateColors` fills the rest.
#[derive(Default)]
struct Seed {
    background: Option<Color>,
    primary: Option<Color>,
    secondary: Option<Color>,
    tertiary: Option<Color>,
    primary_border: Option<Color>,
    primary_text: Option<Color>,
    main_bkg: Option<Color>,
    border1: Option<Color>,
    node_border: Option<Color>,
    cluster_bkg: Option<Color>,
    cluster_border: Option<Color>,
    /// `primaryTextColor` when nothing sets it and `darkMode` is off.
    text_fallback: Option<Color>,
    /// `nodeBorder` falls back to `border1` rather than `primaryBorderColor`.
    border_from_border1: bool,
}

/// A `base`-style theme: every color not set is derived from `primaryColor` and `background`.
fn derived(color: &dyn Fn(&str) -> Option<Color>, s: &Seed, dark: bool) -> Palette {
    let v = |key: &str, seed: Option<Color>| color(key).or(seed);
    let background = v("background", s.background).unwrap_or(rgb(0xf4f4f4));
    let primary = v("primaryColor", s.primary).unwrap_or(rgb(0xfff4dd));
    let primary_text = v("primaryTextColor", s.primary_text).unwrap_or(if dark {
        rgb(0xeeeeee)
    } else {
        s.text_fallback.unwrap_or(rgb(0x333333))
    });
    let secondary = v("secondaryColor", s.secondary).unwrap_or(adjust(primary, -120.0, 0.0, 0.0));
    let tertiary = v("tertiaryColor", s.tertiary).unwrap_or(adjust(primary, 180.0, 0.0, 5.0));
    let primary_border = v("primaryBorderColor", s.primary_border).unwrap_or(border(primary, dark));
    let tertiary_border = color("tertiaryBorderColor").unwrap_or(border(tertiary, dark));
    // Labels take `nodeTextColor`, which these themes always derive, so `textColor` goes unused.
    let node_border = v("nodeBorder", s.node_border).unwrap_or(if s.border_from_border1 {
        v("border1", s.border1).unwrap_or(primary_border)
    } else {
        primary_border
    });
    Palette {
        background,
        text: color("nodeTextColor").unwrap_or(primary_text),
        node_fill: v("mainBkg", s.main_bkg).unwrap_or(primary),
        node_stroke: node_border,
        edge: color("lineColor").unwrap_or(invert(background)),
        edge_label_fill: color("edgeLabelBackground").unwrap_or(if dark {
            lighten(secondary, -30.0)
        } else {
            secondary
        }),
        cluster_fill: v("clusterBkg", s.cluster_bkg).unwrap_or(tertiary),
        cluster_stroke: v("clusterBorder", s.cluster_border).unwrap_or(tertiary_border),
    }
}

fn resolve(name: &str, color: &dyn Fn(&str) -> Option<Color>, dark: bool) -> Palette {
    let white = rgb(0xffffff);
    match name {
        "forest" => classic(
            color,
            &Classic {
                background: white,
                text: rgb(0x000000),
                main_bkg: rgb(0xcde498),
                border1: rgb(0x13540c),
                second_bkg: rgb(0xcdffb2),
                border2: rgb(0x6eaa49),
                line: rgb(0x000000),
                label_bkg: rgb(0xe8e8e8),
            },
        ),
        "neutral" => {
            let contrast = color("contrast").unwrap_or(rgb(0x707070));
            let mut p = classic(
                color,
                &Classic {
                    background: white,
                    text: rgb(0x000000),
                    main_bkg: rgb(0xeeeeee),
                    border1: rgb(0x999999),
                    second_bkg: lighten(contrast, 55.0),
                    border2: contrast,
                    line: rgb(0x666666),
                    label_bkg: white,
                },
            );
            p.edge_label_fill = color("edgeLabelBackground").unwrap_or(white);
            p
        }
        "dark" => {
            let main_bkg = color("mainBkg").unwrap_or(rgb(0x1f2020));
            let label_bkg = color("labelBackground").unwrap_or(rgb(0x181818));
            let mut p = classic(
                color,
                &Classic {
                    background: rgb(0x333333),
                    text: rgb(0xcccccc),
                    main_bkg,
                    border1: rgb(0xcccccc),
                    second_bkg: lighten(main_bkg, 16.0),
                    border2: rgb(0xffffff).with_alpha(64),
                    line: color("mainContrastColor").unwrap_or(rgb(0xd3d3d3)),
                    label_bkg,
                },
            );
            p.edge_label_fill = color("edgeLabelBackground").unwrap_or(lighten(label_bkg, 25.0));
            p
        }
        "base" => derived(color, &Seed::default(), dark),
        "neo" => derived(
            color,
            &Seed {
                background: Some(white),
                primary: Some(rgb(0xcccccc)),
                main_bkg: Some(white),
                primary_border: Some(border(rgb(0xcccccc), false)),
                node_border: Some(rgb(0x000000)),
                tertiary: Some(white),
                ..Seed::default()
            },
            dark,
        ),
        "redux" | "redux-color" => derived(
            color,
            &Seed {
                background: Some(white),
                primary: Some(rgb(0xcccccc)),
                main_bkg: Some(white),
                primary_border: Some(border(rgb(0x28253d), false)),
                node_border: Some(rgb(0x28253d)),
                tertiary: Some(white),
                cluster_bkg: Some(rgb(0xf9f9fb)),
                cluster_border: Some(rgb(0xbdbccc)),
                text_fallback: Some(rgb(0x28253d)),
                ..Seed::default()
            },
            dark,
        ),
        "neo-dark" | "redux-dark" | "redux-dark-color" => {
            let primary = rgb(0x1f2020);
            let background = rgb(0x333333);
            let redux = name != "neo-dark";
            derived(
                color,
                &Seed {
                    background: Some(background),
                    primary: Some(primary),
                    secondary: Some(lighten(primary, 16.0)),
                    tertiary: Some(adjust(primary, -160.0, 0.0, 0.0)),
                    primary_border: Some(invert(background)),
                    primary_text: Some(invert(primary)),
                    main_bkg: Some(if redux { rgb(0x111113) } else { rgb(0x2a2020) }),
                    border1: Some(rgb(0xcccccc)),
                    node_border: redux.then_some(white),
                    cluster_bkg: redux.then_some(rgb(0x1e1a2e)),
                    cluster_border: redux.then_some(rgb(0xbdbccc)),
                    border_from_border1: true,
                    ..Seed::default()
                },
                dark,
            )
        }
        // `default`.
        _ => classic(
            color,
            &Classic {
                background: white,
                text: rgb(0x333333),
                main_bkg: rgb(0xececff),
                border1: rgb(0x9370db),
                second_bkg: rgb(0xffffde),
                border2: rgb(0xaaaa33),
                line: rgb(0x333333),
                label_bkg: Color {
                    r: 232,
                    g: 232,
                    b: 232,
                    a: 204,
                },
            },
        ),
    }
}

const fn rgb(hex: u32) -> Color {
    Color::rgb(hex)
}

/// A theme's chart colors: `cScale0` to `cScale11` and `pie1` to `pie12`.
struct Charts {
    scale: [Color; 12],
    pie: [Color; 12],
}

/// `primaryColor`, `secondaryColor`, and `tertiaryColor` as theme `name` sets them.
fn primaries(name: &str, color: &dyn Fn(&str) -> Option<Color>) -> (Color, Color, Color) {
    let rotate = |c: Color, h: f32| adjust(c, h, 0.0, 0.0);
    let dark = rgb(0x1f2020);
    let (p, s, t) = match name {
        "default" => (
            rgb(0xececff),
            Some(rgb(0xffffde)),
            Some(rotate(rgb(0xececff), -160.0)),
        ),
        "forest" => (
            rgb(0xcde498),
            Some(rgb(0xcdffb2)),
            Some(lighten(rgb(0xcde498), 10.0)),
        ),
        "neutral" => {
            let contrast = color("contrast").unwrap_or(rgb(0x707070));
            (
                rgb(0xeeeeee),
                Some(lighten(contrast, 55.0)),
                Some(rotate(rgb(0xeeeeee), -160.0)),
            )
        }
        "dark" | "neo-dark" | "redux-dark" | "redux-dark-color" => {
            (dark, Some(lighten(dark, 16.0)), Some(rotate(dark, -160.0)))
        }
        "neo" | "redux" | "redux-color" => (rgb(0xcccccc), None, Some(rgb(0xffffff))),
        // `base`.
        _ => (rgb(0xfff4dd), None, None),
    };
    let p = color("primaryColor").unwrap_or(p);
    let s = color("secondaryColor")
        .or(s)
        .unwrap_or_else(|| rotate(p, -120.0));
    let t = color("tertiaryColor")
        .or(t)
        .unwrap_or_else(|| adjust(p, 180.0, 0.0, 5.0));
    (p, s, t)
}

/// The chart colors of theme `name`, as its `updateColors` derives them; `main_bkg` is the
/// resolved `mainBkg`.
fn charts(
    name: &str,
    color: &dyn Fn(&str) -> Option<Color>,
    dark: bool,
    main_bkg: Color,
) -> Charts {
    let (p, s, t) = primaries(name, color);
    let a = |c: Color, h: f32, l: f32| adjust(c, h, 0.0, l);
    // `cScale3` on: turns of the primary color; `l8` is `cScale8`'s lightness change.
    let hues = |p: Color, s: Color, t: Color, l8: f32| {
        [
            p,
            s,
            t,
            a(p, 30.0, 0.0),
            a(p, 60.0, 0.0),
            a(p, 90.0, 0.0),
            a(p, 120.0, 0.0),
            a(p, 150.0, 0.0),
            a(p, 210.0, l8),
            a(p, 270.0, 0.0),
            a(p, 300.0, 0.0),
            a(p, 330.0, 0.0),
        ]
    };
    let darkened = |c: [Color; 12], by: f32| c.map(|c| lighten(c, -by));
    let base_darken = if dark { 75.0 } else { 25.0 };
    // `neo` and `redux` color charts from fixed colors rather than their `primaryColor`.
    let fixed = (
        rgb(0xececfe),
        rgb(0xe9e9f1),
        adjust(rgb(0xececfe), 180.0, 0.0, 5.0),
    );
    let mut scale = match name {
        "default" | "forest" => darkened(hues(p, s, t, 0.0), 10.0),
        "dark" => {
            let mut c = [
                0, 0x0b0000, 0x4d1037, 0x3f5258, 0x4f2f1b, 0x6e0a0a, 0x3b0048, 0x995a01, 0x154706,
                0x161722, 0x00296f, 0x01629c,
            ]
            .map(rgb);
            c[0] = p;
            c
        }
        "neutral" => [
            0x555555, 0xf4f4f4, 0x555555, 0xbbbbbb, 0x777777, 0x999999, 0xdddddd, 0xffffff,
            0xdddddd, 0xbbbbbb, 0x999999, 0x777777,
        ]
        .map(rgb),
        "neo" => darkened(hues(fixed.0, fixed.1, fixed.2, 150.0), base_darken),
        "redux" => darkened([main_bkg; 12], base_darken),
        "redux-color" | "redux-dark-color" => [
            0xf4a8ff, 0x46ecd5, 0xffb86a, 0xdab2ff, 0x7bf1a8, 0xc4b4ff, 0xffa2a2, 0xffdf20,
            0xa3b3ff, 0xbbf451, 0x74d4ff, 0xffa1ad,
        ]
        .map(rgb),
        // `base`, `neo-dark`, and `redux-dark`.
        _ => darkened(hues(p, s, t, 150.0), base_darken),
    };
    for (i, c) in scale.iter_mut().enumerate() {
        if let Some(v) = color(&format!("cScale{i}")) {
            *c = v;
        }
    }
    let base_pie = |p: Color, s: Color, t: Color| {
        [
            p,
            s,
            t,
            a(p, 0.0, -10.0),
            a(s, 0.0, -10.0),
            a(t, 0.0, -10.0),
            a(p, 60.0, -10.0),
            a(p, -60.0, -10.0),
            a(p, 120.0, 0.0),
            a(p, 60.0, -20.0),
            a(p, -60.0, -20.0),
            a(p, 120.0, -10.0),
        ]
    };
    let mut pie = match name {
        "default" => [
            p,
            s,
            a(t, 0.0, -40.0),
            a(p, 0.0, -10.0),
            a(s, 0.0, -30.0),
            a(t, 0.0, -20.0),
            a(p, 60.0, -20.0),
            a(p, -60.0, -40.0),
            a(p, 120.0, -40.0),
            a(p, 60.0, -40.0),
            a(p, -90.0, -40.0),
            a(p, 120.0, -30.0),
        ],
        "forest" => [
            p,
            s,
            t,
            a(p, 0.0, -30.0),
            a(s, 0.0, -30.0),
            a(t, 40.0, -40.0),
            a(p, 60.0, -10.0),
            a(p, -60.0, -10.0),
            a(p, 120.0, 0.0),
            a(p, 60.0, -50.0),
            a(p, -60.0, -50.0),
            a(p, 120.0, -50.0),
        ],
        // `pieN` is `cScaleN`, so `pie12`, which no `cScale` sets, falls back to the first.
        "dark" | "neutral" => std::array::from_fn(|i| scale[(i + 1) % 12]),
        "redux-color" | "redux-dark-color" => scale,
        "neo" | "redux" => base_pie(fixed.0, fixed.1, fixed.2),
        _ => base_pie(p, s, t),
    };
    for (i, c) in pie.iter_mut().enumerate() {
        if let Some(v) = color(&format!("pie{}", i + 1)) {
            *c = v;
        }
    }
    Charts { scale, pie }
}

/// khroma's `invert`.
fn invert(c: Color) -> Color {
    Color {
        r: 255 - c.r,
        g: 255 - c.g,
        b: 255 - c.b,
        a: c.a,
    }
}

/// khroma's `lighten` (or `darken`, with a negative amount), in lightness percent.
fn lighten(c: Color, amount: f32) -> Color {
    adjust(c, 0.0, 0.0, amount)
}

/// Mermaid's `mkBorder`.
fn border(c: Color, dark: bool) -> Color {
    adjust(c, 0.0, -40.0, if dark { 10.0 } else { -10.0 })
}

/// khroma's `adjust`: hue in degrees, saturation and lightness in percent.
fn adjust(c: Color, dh: f32, ds: f32, dl: f32) -> Color {
    let (h, s, l) = to_hsl(c);
    let (r, g, b) = from_hsl(
        (h + dh).rem_euclid(360.0),
        (s + ds).clamp(0.0, 100.0),
        (l + dl).clamp(0.0, 100.0),
    );
    Color { r, g, b, a: c.a }
}

fn to_hsl(c: Color) -> (f32, f32, f32) {
    let (r, g, b) = (
        f32::from(c.r) / 255.0,
        f32::from(c.g) / 255.0,
        f32::from(c.b) / 255.0,
    );
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    if max == min {
        return (0.0, 0.0, l * 100.0);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s * 100.0, l * 100.0)
}

fn from_hsl(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let (h, s, l) = (h / 360.0, s / 100.0, l / 100.0);
    let byte = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    if s == 0.0 {
        return (byte(l), byte(l), byte(l));
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let hue = |t: f32| {
        let t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    (
        byte(hue(h + 1.0 / 3.0)),
        byte(hue(h)),
        byte(hue(h - 1.0 / 3.0)),
    )
}

/// The `config:` settings of a frontmatter token that the parser accepted.
fn frontmatter(src: &str, mut at: usize, out: &mut ThemeConfig) {
    let mut lines = src.split_inclusive('\n');
    if let Some(opener) = lines.next() {
        at += opener.len();
    }
    let indent = |line: &str| line.len() - line.trim_start().len();
    let mut in_config = false;
    let mut config_child = None;
    let mut in_vars = false;
    let mut vars_child = None;
    for line in lines {
        let start = at;
        at += line.len();
        let text = line.trim_end_matches(['\n', '\r']);
        if crate::lines::is_frontmatter_marker(text) {
            break;
        }
        if text.trim().is_empty() || text.trim_start().starts_with('#') {
            continue;
        }
        let ind = indent(text);
        if ind == 0 {
            in_config = text.trim_end() == "config:";
            config_child = None;
            in_vars = false;
            vars_child = None;
            continue;
        }
        if !in_config {
            continue;
        }
        let config_child = *config_child.get_or_insert(ind);
        if ind == config_child {
            in_vars = false;
            vars_child = None;
        }
        let Some((key, value)) = text[ind..].split_once(':') else {
            continue;
        };
        let value_start = start + ind + key.len() + 1;
        let key = unquote(key);
        if ind != config_child {
            if in_vars
                && ind > config_child
                && ind == *vars_child.get_or_insert(ind)
                && !value.trim().is_empty()
            {
                out.variables.push((key, unquote(value)));
            }
            continue;
        }
        match key.as_str() {
            "theme" => {
                let raw = value.trim();
                let offset = value_start + value.len() - value.trim_start().len();
                let name = unquote(raw);
                out.name = Some((name, offset..offset + raw.len()));
            }
            "themeVariables" if value.trim().is_empty() => in_vars = true,
            _ => {}
        }
    }
}

/// An accepted directive token. Only a complete init directive commits settings.
fn directive(token: &SyntaxToken, out: &mut ThemeConfig) {
    let range = token.text_range();
    let span = usize::from(range.start())..usize::from(range.end());
    let directive = token.text();
    let mut next = ThemeConfig::default();
    let parsed = crate::directive::read(directive, |path, value, range| match path {
        [Key(init), Key(key)] if crate::directive::is_init(init) && key == "theme" => {
            next.name = Some((
                value.to_owned(),
                span.start + range.start..span.start + range.end,
            ));
        }
        [Key(init), Key(section), Key(key)]
            if crate::directive::is_init(init) && section == "themeVariables" =>
        {
            next.variables.push((key.to_string(), value.to_owned()));
        }
        _ => {}
    });
    if parsed.is_some() {
        if next.name.is_some() {
            out.name = next.name;
        }
        out.variables.extend(next.variables);
    } else if crate::directive::starts_init(directive) {
        out.unread.push(span);
    }
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    for q in ['"', '\''] {
        if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
            return s[1..s.len() - 1].to_owned();
        }
    }
    s.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lines::MAX_DEPTH;

    fn hex(c: Color) -> String {
        format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
    }

    #[test]
    fn khroma_helpers() {
        // hsl(41, 100%, 93%) -> hsl(41, 60%, 83%).
        assert_eq!(hex(border(rgb(0xfff4dd), false)), "#eedebb");
        // A hue turn keeps saturation and lightness.
        assert_eq!(hex(adjust(rgb(0xff0000), -120.0, 0.0, 0.0)), "#0000ff");
        assert_eq!(hex(adjust(rgb(0xff0000), 120.0, 0.0, 0.0)), "#00ff00");
        assert_eq!(hex(lighten(rgb(0x707070), 55.0)), "#fcfcfc");
        assert_eq!(hex(lighten(rgb(0x1f2020), 16.0)), "#474949");
        assert_eq!(hex(lighten(rgb(0x808080), -30.0)), "#343434");
        assert_eq!(hex(invert(rgb(0xf4f4f4))), "#0b0b0b");
        assert_eq!(hex(border(rgb(0xffffff), false)), "#e6e6e6");
    }

    #[test]
    fn reads_frontmatter() {
        let src = "---\ntitle: T\nconfig:\n  theme: forest\n  themeVariables:\n    \
                   primaryColor: \"#ff0000\"\n    lineColor: '#00ff00'\n  flowchart:\n    \
                   curve: basis\n---\nflowchart\nA\n";
        let c = ThemeConfig::read(src);
        let (name, range) = c.name.clone().unwrap();
        assert_eq!(name, "forest");
        assert_eq!(&src[range], "forest");
        assert_eq!(
            c.variables,
            [
                ("primaryColor".into(), "#ff0000".into()),
                ("lineColor".into(), "#00ff00".into())
            ]
        );
    }

    #[test]
    fn frontmatter_theme_range_with_trailing_space() {
        for newline in ["\n", "\r\n", "\r"] {
            let src = ["\u{feff}---", "config:", "  theme: é   ", "---", "pie"].join(newline);
            let lowered = crate::diagram::lower(&src);
            let (name, range) = lowered.theme.name.unwrap();
            assert_eq!(name, "é");
            assert_eq!(src.get(range), Some("é"));
            let range = lowered.diagnostics[0].range;
            let range = usize::from(range.start())..usize::from(range.end());
            assert_eq!(src.get(range), Some("é"));
        }
    }

    #[test]
    fn frontmatter_settings_respect_indentation() {
        let cfg = ThemeConfig::read(
            "---\nconfig:\n  theme: dark\n  themeVariables:\n    background: '#fff'\n    other:\n      lineColor: red\n  flowchart:\n    theme: forest\n    themeVariables:\n      fontSize: 60px\n---\nflowchart\n",
        );
        assert_eq!(cfg.name.unwrap().0, "dark");
        assert_eq!(cfg.variables, [("background".into(), "#fff".into())]);
    }

    #[test]
    fn reads_directives() {
        let src = "%%{init: {'theme': 'neutral', \"themeVariables\": {\"fontSize\": \"20px\", \
                   'lineColor': red}, 'flowchart': {'curve': 'linear'}, 'list': [1, [2]]}}%%\n\
                   flowchart\nA\n";
        let c = ThemeConfig::read(src);
        let (name, range) = c.name.clone().unwrap();
        assert_eq!(name, "neutral");
        assert_eq!(&src[range], "neutral");
        assert_eq!(
            c.variables,
            [
                ("fontSize".into(), "20px".into()),
                ("lineColor".into(), "red".into())
            ]
        );
        // A directive wins over the frontmatter.
        let both =
            "---\nconfig:\n  theme: forest\n---\n%%{initialize: {\"theme\": \"dark\"}}%%\nA\n";
        assert_eq!(ThemeConfig::read(both).name.unwrap().0, "dark");
        assert_eq!(ThemeConfig::read("%%{wrap}%%\nA\n"), ThemeConfig::default());
        assert_eq!(
            ThemeConfig::read("%%{init: {broken\nA\n"),
            ThemeConfig::default()
        );
    }

    /// An `init` directive that cannot be read, malformed or nested past `MAX_DEPTH`, is one
    /// warning over the directive, and the others still apply.
    #[test]
    fn unread_directives() {
        let src = "%%{init: {'theme': }}%%\n%%{init: {'theme': 'dark'}}%%\nA\n";
        let c = ThemeConfig::read(src);
        assert_eq!(c.name.as_ref().unwrap().0, "dark");
        assert_eq!(c.unread.len(), 1);
        assert_eq!(&src[c.unread[0].clone()], "%%{init: {'theme': }}%%");
        let d = c.diagnostics();
        assert_eq!(d.len(), 1);
        assert!(
            d[0].message.contains("could not be read"),
            "{}",
            d[0].message
        );
        let deep = format!(
            "%%{{\"initialize\": {}{}}}%%\nA\n",
            "{'a': ".repeat(MAX_DEPTH + 1),
            "}".repeat(MAX_DEPTH + 1)
        );
        assert_eq!(ThemeConfig::read(&deep).unread.len(), 1);
        let shallow = format!(
            "%%{{init: {}1{}}}%%\nA\n",
            "{'a': ".repeat(MAX_DEPTH - 2),
            "}".repeat(MAX_DEPTH - 2)
        );
        assert!(ThemeConfig::read(&shallow).unread.is_empty());
    }

    #[test]
    fn directive_settings_respect_structure_and_nesting() {
        let src = r#"%%{init: {'list': ["]", ["["], {"x":"}%%"}], 'theme': 'dark'}}%%
%%{'init.theme': 'forest', 'init': {'themeVariables.lineColor': red}}%%
%%{init: {'theme': 'neutral'}}}%%
%%{initialize: {'theme': 'base', 'list': [1, {'x': 2]}}}%%
%%{init: {'theme': 'forest', 'list': [1 2]}}%%
flowchart
"#;
        let cfg = ThemeConfig::read(src);
        assert_eq!(cfg.name.unwrap().0, "dark");
        assert!(cfg.variables.is_empty());
        assert_eq!(cfg.unread.len(), 3);

        for count in [MAX_DEPTH - 1, MAX_DEPTH] {
            let src = format!(
                "%%{{init: {{theme: dark, list: {}0{}}}}}%%\nflowchart\n",
                "[".repeat(count),
                "]".repeat(count)
            );
            let cfg = ThemeConfig::read(&src);
            let within_limit = count < MAX_DEPTH;
            assert_eq!(cfg.unread.is_empty(), within_limit, "{src}");
            assert_eq!(cfg.name.is_some(), within_limit, "{src}");
        }
    }

    #[test]
    fn directive_strings_decode_escapes_and_keep_source_ranges() {
        let src = r#"%%{"\u0069nit": {"th\u0065me": "\u0064ark",
          "themeVariables": {"fontFamily": 'Joe\'s \ud83d\ude00'}}}%%
flowchart
"#;
        let cfg = ThemeConfig::read(src);
        let (name, range) = cfg.name.unwrap();
        assert_eq!(name, "dark");
        assert_eq!(&src[range], r"\u0064ark");
        assert_eq!(cfg.variables, [("fontFamily".into(), "Joe's 😀".into())]);
        assert!(cfg.unread.is_empty());

        for invalid in [r"\q", r"\uD834\u0000", r"\uDD1E", r"\u12", "\t"] {
            let src = format!(
                "%%{{\"\\u0069nit\": {{theme: dark, ignored: \"{invalid}\"}}}}%%\nflowchart\n"
            );
            let cfg = ThemeConfig::read(&src);
            assert!(cfg.name.is_none(), "{src:?}");
            assert_eq!(cfg.unread.len(), 1, "{src:?}");
        }
    }

    #[test]
    fn escaped_theme_name_diagnostic_keeps_controls_printable() {
        let src = r#"%%{init: {theme: "\u001b"}}%%
flowchart
"#;
        let lowered = crate::diagram::lower(src);
        assert_eq!(lowered.diagnostics.len(), 1);
        assert!(!lowered.diagnostics[0].message.chars().any(char::is_control));
        assert!(lowered.diagnostics[0].message.contains(r"\u{1b}"));
    }

    #[test]
    fn themes() {
        let apply = |src: &str| ThemeConfig::read(src).apply(&Theme::DARK);
        let t = apply("%%{init: {'theme': 'forest'}}%%\nA\n");
        assert_eq!(hex(t.node_fill), "#cde498");
        assert_eq!(hex(t.node_stroke), "#13540c");
        assert_eq!(hex(t.cluster_fill), "#cdffb2");
        assert_eq!(hex(t.background), "#ffffff");
        assert_eq!(t.font_size, Theme::DARK.font_size);
        let t = apply("%%{init: {'theme': 'default'}}%%\nA\n");
        assert_eq!(t.node_fill, Theme::LIGHT.node_fill);
        assert_eq!(t.node_stroke, Theme::LIGHT.node_stroke);
        assert_eq!(t.cluster_stroke, Theme::LIGHT.cluster_stroke);
        let t = apply(
            "%%{init: {'theme': 'base', 'themeVariables': {'primaryColor': '#BB2528'}}}%%\nA\n",
        );
        assert_eq!(hex(t.node_fill), "#bb2528");
        assert_eq!(hex(t.node_stroke), hex(border(rgb(0xbb2528), false)));
        assert_eq!(hex(t.text), "#333333");
        assert_eq!(hex(t.edge), "#0b0b0b");
        let t = apply("%%{init: {'theme': 'dark'}}%%\nA\n");
        assert_eq!(hex(t.node_fill), "#1f2020");
        assert_eq!(hex(t.edge), "#d3d3d3");
        assert_eq!(t.cluster_stroke.a, 64);
        let t = apply("%%{init: {'theme': 'neutral'}}%%\nA\n");
        assert_eq!(hex(t.cluster_fill), "#fcfcfc");
        assert_eq!(hex(t.cluster_stroke), "#707070");
        let t = apply("%%{init: {'theme': 'redux-dark'}}%%\nA\n");
        assert_eq!(hex(t.node_fill), "#111113");
        assert_eq!(hex(t.node_stroke), "#ffffff");
        let t = apply("%%{init: {'theme': 'neo'}}%%\nA\n");
        assert_eq!(hex(t.node_stroke), "#000000");
        assert_eq!(hex(t.cluster_stroke), "#e6e6e6");
        for name in NAMES {
            let src = format!("%%{{init: {{'theme': '{name}'}}}}%%\nA\n");
            assert!(ThemeConfig::read(&src).diagnostics().is_empty(), "{name}");
        }
    }

    #[test]
    fn chart_colors() {
        let apply = |src: &str| ThemeConfig::read(src).apply(&Theme::LIGHT);
        let named = |name: &str| apply(&format!("%%{{init: {{'theme': '{name}'}}}}%%\nA\n"));
        // Forest's first slice is its primary color, and its sections are its turns, darkened.
        let t = named("forest");
        let (scale, pie) = (t.scale.unwrap(), t.pie.unwrap());
        assert_eq!(hex(pie[0]), "#cde498");
        assert_eq!(hex(pie[1]), "#cdffb2");
        assert_eq!(scale[0], lighten(rgb(0xcde498), -10.0));
        assert_eq!(
            scale[3],
            lighten(adjust(rgb(0xcde498), 30.0, 0.0, 0.0), -10.0)
        );
        // The dark theme's sections are fixed after the first, and its slices follow them.
        let t = named("dark");
        let (scale, pie) = (t.scale.unwrap(), t.pie.unwrap());
        assert_eq!(hex(scale[0]), "#1f2020");
        assert_eq!(hex(scale[1]), "#0b0000");
        assert_eq!(pie[0], scale[1]);
        assert_eq!(pie[11], scale[0]);
        assert_eq!(hex(named("neutral").scale.unwrap()[0]), "#555555");
        let t = named("redux-color");
        assert_eq!(hex(t.scale.unwrap()[0]), "#f4a8ff");
        assert_eq!(t.pie, t.scale);
        // `redux` sections are its `mainBkg`, darkened.
        assert_eq!(
            named("redux").scale.unwrap()[5],
            lighten(rgb(0xffffff), -25.0)
        );
        // `base` takes its colors from `primaryColor`.
        let t = apply(
            "%%{init: {'theme': 'base', 'themeVariables': {'primaryColor': '#BB2528', \
             'cScale2': '#00ff00', 'pie3': '#0000ff'}}}%%\nA\n",
        );
        assert_eq!(hex(t.pie.unwrap()[0]), "#bb2528");
        assert_eq!(t.scale.unwrap()[0], lighten(rgb(0xbb2528), -25.0));
        assert_eq!(hex(t.scale.unwrap()[2]), "#00ff00");
        assert_eq!(hex(t.pie.unwrap()[2]), "#0000ff");
        // Without a name, chart colors change only when the diagram sets one.
        assert_eq!(
            apply("%%{init: {'themeVariables': {'fontSize': 16}}}%%\nA\n").pie,
            None
        );
        let t = apply("%%{init: {'themeVariables': {'pie2': '#123456'}}}%%\nA\n");
        let pie = t.pie.unwrap();
        assert_eq!(hex(pie[1]), "#123456");
        assert_eq!(hex(pie[0]), "#ececff");
        assert_eq!(t.node_fill, Theme::LIGHT.node_fill);
    }

    #[test]
    fn variables_without_a_theme() {
        let src =
            "%%{init: {'themeVariables': {'nodeBorder': '#ff0000', 'fontSize': '21px'}}}%%\nA\n";
        let t = ThemeConfig::read(src).apply(&Theme::LIGHT);
        assert_eq!(hex(t.node_stroke), "#ff0000");
        assert_eq!(t.node_fill, Theme::LIGHT.node_fill);
        assert_eq!(t.font_size, 21.0);
        assert_eq!(t.line_height, 30.0);
        assert_eq!(ThemeConfig::read("A\n").apply(&Theme::DARK), Theme::DARK);
    }

    #[test]
    fn unknown_theme_warns() {
        let src = "---\nconfig:\n  theme: sunset\n---\nflowchart\nA\n";
        let c = ThemeConfig::read(src);
        let d = c.diagnostics();
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("`sunset` is not a Mermaid theme"));
        assert_eq!(&src[d[0].range], "sunset");
        assert_eq!(c.apply(&Theme::DARK), Theme::DARK);
    }
}
