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
        self.spans()
            .iter()
            .map(|line| line.iter().map(|s| s.text.as_str()).collect())
            .collect()
    }

    /// Display lines as styled spans: markdown `**bold**`, `__bold__`, `*italic*`, and
    /// `_italic_` become styles (FLOWMO-6); markers without a partner stay as text. Plain labels
    /// are one unstyled span per line.
    pub fn spans(&self) -> Vec<Vec<Span>> {
        self.text
            .split('\n')
            .map(|line| {
                let line = clean(line);
                if self.markdown {
                    markdown_spans(&line)
                } else {
                    vec![Span::plain(line)]
                }
            })
            .collect()
    }
}

/// A run of label text in one style.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
}

impl Span {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }
}

/// An emphasis marker in a markdown line: `true` for bold.
enum Token<'a> {
    Text(&'a str),
    Marker { bold: bool, raw: &'a str },
}

fn markdown_spans(line: &str) -> Vec<Span> {
    // Split into text and markers. `_` marks only at a word edge, so `snake_case` stays text.
    let mut tokens = Vec::new();
    let mut start = 0;
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let mut k = 0;
    while k < chars.len() {
        let (i, c) = chars[k];
        let double = chars.get(k + 1).is_some_and(|&(_, d)| d == c);
        let len = if double { 2 } else { 1 };
        let marker = match c {
            '*' => true,
            '_' => {
                let before = k.checked_sub(1).map(|j| chars[j].1);
                let after = chars.get(k + len).map(|&(_, a)| a);
                !(before.is_some_and(char::is_alphanumeric)
                    && after.is_some_and(char::is_alphanumeric))
            }
            _ => false,
        };
        if marker {
            if start < i {
                tokens.push(Token::Text(&line[start..i]));
            }
            let end = i + c.len_utf8() * len;
            tokens.push(Token::Marker {
                bold: double,
                raw: &line[i..end],
            });
            start = end;
            k += len;
        } else {
            k += 1;
        }
    }
    if start < line.len() {
        tokens.push(Token::Text(&line[start..]));
    }

    // Pair each marker with the next unpaired one of the same kind and character.
    let mut paired = vec![false; tokens.len()];
    for i in 0..tokens.len() {
        let Token::Marker { bold, raw } = tokens[i] else {
            continue;
        };
        if paired[i] {
            continue;
        }
        let partner = (i + 1..tokens.len()).find(|&j| {
            !paired[j]
                && matches!(tokens[j], Token::Marker { bold: b, raw: r } if b == bold && r == raw)
        });
        if let Some(j) = partner {
            paired[i] = true;
            paired[j] = true;
        }
    }

    let (mut bold, mut italic) = (false, false);
    let mut spans: Vec<Span> = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        let text = match *t {
            Token::Marker { bold: b, .. } if paired[i] => {
                if b {
                    bold = !bold;
                } else {
                    italic = !italic;
                }
                continue;
            }
            Token::Marker { raw, .. } => raw,
            Token::Text(text) => text,
        };
        match spans.last_mut() {
            Some(s) if s.bold == bold && s.italic == italic => s.text.push_str(text),
            _ => spans.push(Span {
                text: text.to_owned(),
                bold,
                italic,
            }),
        }
    }
    if spans.is_empty() {
        spans.push(Span::default());
    }
    spans
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

/// One row of a compartment: cells aligned in columns across the section (LANGS-3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Row {
    pub cells: Vec<String>,
    pub italic: bool,
    pub underline: bool,
}

impl Row {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            cells: vec![text.into()],
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub id: String,
    pub label: Label,
    pub shape: Shape,
    /// Compartments under the label, each a list of rows, separated by rules (LANGS-3); empty
    /// for an ordinary node.
    pub sections: Vec<Vec<Row>>,
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

/// Arrowhead at one end of an edge (FLOWMO-8), and the markers of class and ER diagrams
/// (LANGS-3), which flowcharts never use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrow {
    Point,
    Cross,
    Circle,
    /// A hollow triangle: inheritance, realization.
    Triangle,
    /// A filled diamond: composition.
    Diamond,
    /// A hollow diamond: aggregation.
    HollowDiamond,
    /// Crow's foot: zero or one (`|o`).
    ZeroOne,
    /// Crow's foot: exactly one (`||`).
    One,
    /// Crow's foot: zero or more (`}o`).
    ZeroMany,
    /// Crow's foot: one or more (`}|`).
    OneMany,
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
    /// Labels beside the source and target ends, such as multiplicities (LANGS-3).
    pub start_label: Option<Label>,
    pub end_label: Option<Label>,
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
    /// A concurrent region of a state (LANGS-3): untitled, unfilled, with a dashed border.
    pub region: bool,
}

/// Which side of its target a note sits on (LANGS-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteSide {
    /// Left of the target in a vertical layout, above it in a horizontal one.
    Before,
    /// Right of the target in a vertical layout, below it in a horizontal one.
    After,
}

/// A note beside a node, joined to it by a dashed line (LANGS-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// The node the note is about.
    pub target: String,
    pub side: NoteSide,
    pub label: Label,
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
    /// Notes beside nodes, in source order.
    pub notes: Vec<Note>,
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
