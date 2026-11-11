//! A diagram's own theme (MTHEME-1): `theme` and `themeVariables` from its frontmatter `config:`
//! or an `%%{init: ...}%%` directive, resolved to drawing colors as Mermaid 12.3's themes resolve
//! them. The palettes and the color derivations are ported from Mermaid's `src/themes/`
//! (Copyright (c) 2014-2022 Knut Sveidqvist; MIT) and khroma's `adjust`, `lighten`, `darken`,
//! and `invert`; see THIRD-PARTY-NOTICES.

use std::ops::Range;

use rowan::{TextRange, TextSize};

use crate::diagnostic::Diagnostic;
use crate::scene::{Color, Theme, parse_color};

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
}

impl ThemeConfig {
    /// Reads the frontmatter `config:` and then every `%%{init}%%` directive, which win.
    pub fn read(src: &str) -> ThemeConfig {
        let mut out = ThemeConfig::default();
        let body = frontmatter(src, &mut out);
        directives(src, body, &mut out);
        out
    }

    /// Warnings for settings Mermaid ignores: an unknown theme name.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        match &self.name {
            Some((name, range)) if !NAMES.contains(&name.as_str()) => {
                let range = TextRange::new(
                    TextSize::new(range.start as u32),
                    TextSize::new(range.end as u32),
                );
                vec![Diagnostic::warning(
                    range,
                    format!(
                        "`{name}` is not a Mermaid theme; expected one of {}",
                        NAMES.join(", ")
                    ),
                )]
            }
            _ => Vec::new(),
        }
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
        let p = match name.filter(|n| NAMES.contains(n)) {
            None => classic(&color, &from_theme(base)),
            Some(n) => resolve(n, &color, var("darkMode") == Some("true")),
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

/// The frontmatter's `config:` `theme` and `themeVariables`; returns where the body starts.
fn frontmatter(src: &str, out: &mut ThemeConfig) -> usize {
    let mut lines = src.split_inclusive('\n');
    let Some(first) = lines.next() else {
        return 0;
    };
    if first.trim_end() != "---" {
        return 0;
    }
    let mut at = first.len();
    let indent = |line: &str| line.len() - line.trim_start().len();
    let mut in_config = false;
    let mut vars_at: Option<usize> = None;
    for line in lines {
        let start = at;
        at += line.len();
        let text = line.trim_end_matches(['\n', '\r']);
        if text.trim() == "---" {
            return at;
        }
        if text.trim().is_empty() || text.trim_start().starts_with('#') {
            continue;
        }
        let ind = indent(text);
        if ind == 0 {
            in_config = text.trim_end() == "config:";
            vars_at = None;
            continue;
        }
        if !in_config {
            continue;
        }
        let Some((key, value)) = text.trim().split_once(':') else {
            continue;
        };
        let key = unquote(key);
        if let Some(v) = vars_at {
            if ind > v {
                if !value.trim().is_empty() {
                    out.variables.push((key, unquote(value)));
                }
                continue;
            }
            vars_at = None;
        }
        match key.as_str() {
            "theme" => {
                let raw = value.trim();
                let offset = start + ind + text.trim().len() - value.trim_start().len();
                let name = unquote(raw);
                out.name = Some((name, offset..offset + raw.len()));
            }
            "themeVariables" if value.trim().is_empty() => vars_at = Some(ind),
            _ => {}
        }
    }
    0
}

/// Every `%%{init: ...}%%` (or `initialize`) directive after `from`.
fn directives(src: &str, from: usize, out: &mut ThemeConfig) {
    let mut at = from;
    while let Some(i) = src[at..].find("%%{") {
        let open = at + i + 3;
        let Some(j) = src[open..].find("}%%") else {
            return;
        };
        let close = open + j;
        let mut p = Parser {
            src,
            at: open,
            end: close,
        };
        let mut entries = Vec::new();
        if p.object_body("", &mut entries).is_some() {
            for (path, value, range) in entries {
                let key = path
                    .strip_prefix("init.")
                    .or_else(|| path.strip_prefix("initialize."));
                match key {
                    Some("theme") => out.name = Some((value, range)),
                    Some(k) => {
                        if let Some(var) = k.strip_prefix("themeVariables.") {
                            out.variables.push((var.to_owned(), value));
                        }
                    }
                    None => {}
                }
            }
        }
        at = close + 3;
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

/// A lenient reader of a directive's JSON, which may use single quotes and bare words.
struct Parser<'a> {
    src: &'a str,
    at: usize,
    end: usize,
}

type Entry = (String, String, Range<usize>);

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.src[self.at..self.end].chars().next()
    }

    fn skip_space(&mut self) {
        while let Some(c) = self.peek() {
            if !c.is_whitespace() {
                break;
            }
            self.at += c.len_utf8();
        }
    }

    fn eat(&mut self, c: char) -> bool {
        self.skip_space();
        if self.peek() == Some(c) {
            self.at += c.len_utf8();
            true
        } else {
            false
        }
    }

    /// `key: value` pairs up to a `}` or the end.
    fn object_body(&mut self, path: &str, out: &mut Vec<Entry>) -> Option<()> {
        loop {
            self.skip_space();
            match self.peek() {
                None | Some('}') => return Some(()),
                _ => {}
            }
            let (key, _) = self.scalar(true)?;
            if !self.eat(':') {
                return None;
            }
            let path = if path.is_empty() {
                key
            } else {
                format!("{path}.{key}")
            };
            self.value(&path, out)?;
            if !self.eat(',') {
                self.skip_space();
                return matches!(self.peek(), None | Some('}')).then_some(());
            }
        }
    }

    fn value(&mut self, path: &str, out: &mut Vec<Entry>) -> Option<()> {
        self.skip_space();
        match self.peek()? {
            '{' => {
                self.at += 1;
                self.object_body(path, out)?;
                self.eat('}').then_some(())
            }
            '[' => {
                let mut depth = 0;
                while let Some(c) = self.peek() {
                    self.at += c.len_utf8();
                    match c {
                        '[' => depth += 1,
                        ']' => {
                            depth -= 1;
                            if depth == 0 {
                                return Some(());
                            }
                        }
                        _ => {}
                    }
                }
                None
            }
            _ => {
                let (v, range) = self.scalar(false)?;
                out.push((path.to_owned(), v, range));
                Some(())
            }
        }
    }

    /// A quoted string, or a bare word up to a delimiter; a key also stops at `:`.
    fn scalar(&mut self, key: bool) -> Option<(String, Range<usize>)> {
        self.skip_space();
        let q = self.peek()?;
        if q == '"' || q == '\'' {
            let start = self.at + 1;
            let mut i = start;
            let bytes = self.src.as_bytes();
            while i < self.end {
                match bytes[i] {
                    b'\\' => i += 2,
                    b if b == q as u8 => {
                        self.at = i + 1;
                        return Some((self.src[start..i].to_owned(), start..i));
                    }
                    _ => i += 1,
                }
            }
            return None;
        }
        let start = self.at;
        while let Some(c) = self.peek() {
            if matches!(c, ',' | '}' | ']') || (key && c == ':') {
                break;
            }
            self.at += c.len_utf8();
        }
        let raw = &self.src[start..self.at];
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        let s = start + (raw.len() - raw.trim_start().len());
        Some((trimmed.to_owned(), s..s + trimmed.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
