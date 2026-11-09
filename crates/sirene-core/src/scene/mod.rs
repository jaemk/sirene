//! Backend-independent drawing of diagrams as paths and text (DISPLA-*).

mod git;
mod hit;
mod paint;
#[cfg(feature = "png")]
pub mod png;
mod shapes;
mod style;
pub mod svg;
mod timeline;

use unicode_width::UnicodeWidthStr;

pub use git::git_graph;
pub use paint::{CONFIG, flowchart};
pub use style::parse_color;
pub use timeline::timeline;

/// A point in scene coordinates (pixels, y down).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
}

impl Pt {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// One step of a path (DISPLA-2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Segment {
    Move(Pt),
    Line(Pt),
    /// An elliptical arc to `to`, with SVG's flag semantics: `sweep` is clockwise on screen.
    Arc {
        radii: Pt,
        large: bool,
        sweep: bool,
        to: Pt,
    },
    /// A cubic Bezier curve to `to` with control points `c1` and `c2`.
    Cubic {
        c1: Pt,
        c2: Pt,
        to: Pt,
    },
    Close,
}

/// An sRGB color with alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// An opaque color from `0xRRGGBB`.
    pub const fn rgb(hex: u32) -> Self {
        Self {
            r: (hex >> 16) as u8,
            g: (hex >> 8) as u8,
            b: hex as u8,
            a: 255,
        }
    }

    pub const fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    /// This color drawn over the opaque color `below`.
    pub fn over(self, below: Color) -> Color {
        let a = u16::from(self.a);
        let mix = |top: u8, bottom: u8| {
            ((u16::from(top) * a + u16::from(bottom) * (255 - a) + 127) / 255) as u8
        };
        Color {
            r: mix(self.r, below.r),
            g: mix(self.g, below.g),
            b: mix(self.b, below.b),
            a: 255,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stroke {
    pub width: f32,
    pub color: Color,
    /// Dash and gap lengths; solid when `None`.
    pub dash: Option<(f32, f32)>,
}

/// What a scene element draws (DISPLA-2).
#[derive(Debug, Clone, PartialEq)]
pub enum Primitive {
    Path {
        segments: Vec<Segment>,
        fill: Option<Color>,
        stroke: Option<Stroke>,
    },
    /// A run of text on one line, centered on `center` both ways.
    Text {
        text: String,
        center: Pt,
        size: f32,
        color: Color,
        style: TextStyle,
    },
}

/// Font weight and slant of a text run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TextStyle {
    pub bold: bool,
    pub italic: bool,
}

/// The diagram element a primitive belongs to (DISPLA-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Owner {
    /// The diagram itself, e.g. its title.
    Diagram,
    Node(usize),
    Edge(usize),
    Cluster(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub owner: Owner,
    pub primitive: Primitive,
}

/// A drawing: primitives in paint order on a background (DISPLA-1).
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub width: f32,
    pub height: f32,
    pub background: Color,
    pub elements: Vec<Element>,
}

/// Text measurement supplied by the backend that will draw the scene (DISPLA-4).
pub trait TextMetrics {
    /// The advance width of one line of `text` at `size` pixels.
    fn width(&self, text: &str, size: f32) -> f32;

    /// The advance width of `text` in `style`; the regular width unless overridden.
    fn styled_width(&self, text: &str, size: f32, style: TextStyle) -> f32 {
        let _ = style;
        self.width(text, size)
    }
}

/// Monospace estimate: 0.6 em per terminal column, so wide characters count double.
#[derive(Debug, Clone, Copy, Default)]
pub struct Approx;

impl TextMetrics for Approx {
    fn width(&self, text: &str, size: f32) -> f32 {
        UnicodeWidthStr::width(text) as f32 * size * 0.6
    }
}

/// Colors and type sizes (DISPLA-5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub font_size: f32,
    pub line_height: f32,
    pub background: Color,
    pub text: Color,
    pub node_fill: Color,
    pub node_stroke: Color,
    pub edge: Color,
    pub edge_label_fill: Color,
    pub cluster_fill: Color,
    pub cluster_stroke: Color,
    /// Selection outlines and other editing marks.
    pub accent: Color,
}

impl Theme {
    pub const LIGHT: Theme = Theme {
        font_size: 14.0,
        line_height: 20.0,
        background: Color::rgb(0xffffff),
        text: Color::rgb(0x333333),
        node_fill: Color::rgb(0xececff),
        node_stroke: Color::rgb(0x9370db),
        edge: Color::rgb(0x333333),
        edge_label_fill: Color::rgb(0xe8e8e8),
        cluster_fill: Color::rgb(0xffffde),
        cluster_stroke: Color::rgb(0xaaaa33),
        accent: Color::rgb(0x2563eb),
    };

    pub const DARK: Theme = Theme {
        font_size: 14.0,
        line_height: 20.0,
        background: Color::rgb(0x1e1e1e),
        text: Color::rgb(0xe0e0e0),
        node_fill: Color::rgb(0x2b3245),
        node_stroke: Color::rgb(0x81b1db),
        edge: Color::rgb(0xc8c8c8),
        edge_label_fill: Color::rgb(0x3a3a3a),
        cluster_fill: Color::rgb(0x2a2a24),
        cluster_stroke: Color::rgb(0x8a8a5c),
        accent: Color::rgb(0x60a5fa),
    };
}

impl Default for Theme {
    fn default() -> Self {
        Theme::LIGHT
    }
}

#[cfg(test)]
mod tests;
