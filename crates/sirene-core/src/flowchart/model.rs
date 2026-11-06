//! The semantic flowchart model (FLOWMO-*).

use rowan::TextRange;

pub use super::shape::Shape;

/// Layout direction of a diagram or subgraph (FLOWMO-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    /// Top to bottom (`TB`, `TD`, `v`).
    #[default]
    TopBottom,
    /// Bottom to top (`BT`, `^`).
    BottomTop,
    /// Left to right (`LR`, `>`).
    LeftRight,
    /// Right to left (`RL`, `<`).
    RightLeft,
}

impl Direction {
    /// Parses a direction keyword or symbol.
    pub fn from_text(text: &str) -> Option<Direction> {
        match text {
            "TB" | "TD" | "v" => Some(Direction::TopBottom),
            "BT" | "^" => Some(Direction::BottomTop),
            "LR" | ">" => Some(Direction::LeftRight),
            "RL" | "<" => Some(Direction::RightLeft),
            _ => None,
        }
    }
}

/// Label text with its markup flavor (FLOWMO-6).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Label {
    /// Decoded text; lines are separated by `\n`.
    pub text: String,
    /// Whether `text` is mermaid markdown (`**bold**`, `*italic*`).
    pub markdown: bool,
}

impl Label {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            markdown: false,
        }
    }

    /// Display lines: markdown emphasis markers dropped and control characters replaced with
    /// spaces.
    pub fn lines(&self) -> Vec<String> {
        let text = if self.markdown {
            self.text.replace("**", "").replace('*', "")
        } else {
            self.text.clone()
        };
        text.split('\n').map(clean).collect()
    }
}

/// Replaces control characters, which have no width, with spaces.
pub fn clean(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// One CSS-like style declaration, e.g. `fill: #f96` (FLOWMO-12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Style {
    pub property: String,
    pub value: String,
}

/// Ordered `@{ ... }` entries as raw string pairs (FLOWMO-7).
pub type Metadata = Vec<(String, String)>;

/// A `click` action (FLOWMO-14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Click {
    Callback { name: String, args: Vec<String> },
    Link { url: String, target: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub id: String,
    pub label: Label,
    pub shape: Shape,
    pub classes: Vec<String>,
    pub styles: Vec<Style>,
    pub click: Option<Click>,
    pub tooltip: Option<String>,
    pub metadata: Metadata,
    /// Range of every mention of the node, in source order.
    pub mentions: Vec<TextRange>,
}

/// Line style of an edge (FLOWMO-8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stroke {
    Normal,
    Thick,
    Dotted,
    Invisible,
}

/// Arrowhead at one end of an edge (FLOWMO-8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrow {
    Point,
    Cross,
    Circle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    /// User-defined id from `id@-->`.
    pub id: Option<String>,
    /// Source node or subgraph id.
    pub from: String,
    /// Target node or subgraph id.
    pub to: String,
    pub stroke: Stroke,
    pub start: Option<Arrow>,
    pub end: Option<Arrow>,
    /// Minimum rank span, 1 to 10.
    pub length: u8,
    pub label: Option<Label>,
    pub classes: Vec<String>,
    pub styles: Vec<Style>,
    /// Curve name from `linkStyle ... interpolate` or `@{ curve }`.
    pub curve: Option<String>,
    pub animate: bool,
    pub animation: Option<String>,
    pub metadata: Metadata,
    /// Range of the edge's stroke and label.
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subgraph {
    pub id: String,
    pub title: Label,
    pub direction: Option<Direction>,
    /// Ids of direct member nodes and subgraphs, in source order (FLOWMO-10).
    pub members: Vec<String>,
    pub classes: Vec<String>,
    pub styles: Vec<Style>,
    pub metadata: Metadata,
    /// Range of the whole `subgraph` ... `end` block.
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassDef {
    pub name: String,
    pub styles: Vec<Style>,
}

/// A lowered flowchart.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Flowchart {
    pub direction: Direction,
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    /// In first-mention order.
    pub nodes: Vec<Node>,
    /// In source order; `linkStyle` indexes into this list.
    pub edges: Vec<Edge>,
    /// In declaration order.
    pub subgraphs: Vec<Subgraph>,
    pub classes: Vec<ClassDef>,
    /// Styles from `linkStyle default`.
    pub default_edge_styles: Vec<Style>,
    /// Curve from `linkStyle default interpolate`.
    pub default_curve: Option<String>,
}

impl Flowchart {
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn subgraph(&self, id: &str) -> Option<&Subgraph> {
        self.subgraphs.iter().find(|s| s.id == id)
    }

    /// The edge with user-defined id `id`.
    pub fn edge(&self, id: &str) -> Option<&Edge> {
        self.edges.iter().find(|e| e.id.as_deref() == Some(id))
    }

    pub fn class(&self, name: &str) -> Option<&ClassDef> {
        self.classes.iter().find(|c| c.name == name)
    }

    /// The subgraph that directly contains the node or subgraph `id`.
    pub fn parent(&self, id: &str) -> Option<&Subgraph> {
        self.subgraphs
            .iter()
            .find(|s| s.members.iter().any(|m| m == id))
    }
}
