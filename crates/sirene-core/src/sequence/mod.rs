//! Mermaid sequence diagrams (MSEQ-*): participants exchanging messages over time.

pub(crate) mod layout;
mod lower;
mod parser;

use rowan::GreenNode;

pub(crate) use parser::HEADERS;

use crate::diagnostic::Diagnostic;
use crate::syntax::SyntaxNode;

/// Text split into lines at `<br>`.
pub type Text = Vec<String>;

/// How a participant is drawn (MSEQ-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParticipantKind {
    #[default]
    Participant,
    Actor,
    Boundary,
    Control,
    Entity,
    Database,
    Collections,
    Queue,
    /// ZenUML's implicit starter: no header or lifeline, so its calls come in from the left.
    Hidden,
}

impl ParticipantKind {
    /// The kind a `type` of `@{ "type": ... }` names.
    pub fn from_type(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "participant" => Self::Participant,
            "actor" => Self::Actor,
            "boundary" => Self::Boundary,
            "control" => Self::Control,
            "entity" => Self::Entity,
            "database" => Self::Database,
            "collections" => Self::Collections,
            "queue" => Self::Queue,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Participant {
    pub id: String,
    /// The alias, or the id.
    pub label: Text,
    pub kind: ParticipantKind,
    /// The `box` it was declared in, an index into [`Sequence::boxes`].
    pub group: Option<usize>,
}

/// A `box` around participants (MSEQ-1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Group {
    /// The CSS color it was given, as written.
    pub color: Option<String>,
    pub title: Option<String>,
}

/// The line of a message arrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStyle {
    Solid,
    Dotted,
}

/// The end of a message arrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Head {
    /// `->` and `-->`.
    None,
    /// `->>` and `-->>`.
    Filled,
    /// `-x` and `--x`.
    Cross,
    /// `-)` and `--)`, an async message.
    Open,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arrow {
    pub line: LineStyle,
    pub head: Head,
    /// `<<->>` and `<<-->>`: a filled head at both ends.
    pub both: bool,
}

impl Arrow {
    /// The arrow `s` spells, one of the parser's arrows.
    pub fn parse(s: &str) -> Option<Arrow> {
        let both = s.starts_with("<<");
        let body = s.trim_start_matches('<');
        let line = if body.starts_with("--") {
            LineStyle::Dotted
        } else {
            LineStyle::Solid
        };
        let head = match body.trim_start_matches('-') {
            ">" => Head::None,
            ">>" => Head::Filled,
            "x" => Head::Cross,
            ")" => Head::Open,
            _ => return None,
        };
        Some(Arrow { line, head, both })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub from: usize,
    pub to: usize,
    pub arrow: Arrow,
    pub text: Text,
    /// `+` after the arrow: the target becomes active.
    pub activate: bool,
    /// `-` after the arrow: the sender stops being active.
    pub deactivate: bool,
    /// The number `autonumber` gives it.
    pub number: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteSide {
    Left,
    Right,
    Over,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub side: NoteSide,
    /// One participant, or two for a note over both.
    pub participants: Vec<usize>,
    pub text: Text,
}

/// The kind of a block (MSEQ-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Loop,
    Alt,
    Opt,
    Par,
    Critical,
    Break,
    /// A colored background; its label is the color.
    Rect,
    /// ZenUML's `try`, with `catch` and `finally` dividers.
    Try,
}

impl BlockKind {
    pub fn keyword(self) -> &'static str {
        match self {
            BlockKind::Loop => "loop",
            BlockKind::Alt => "alt",
            BlockKind::Opt => "opt",
            BlockKind::Par => "par",
            BlockKind::Critical => "critical",
            BlockKind::Break => "break",
            BlockKind::Rect => "rect",
            BlockKind::Try => "try",
        }
    }
}

/// One step of a sequence diagram, in order (MSEQ-2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Message(Message),
    Note(Note),
    Activate(usize),
    Deactivate(usize),
    /// The opening of a block, with its label.
    Start(BlockKind, String),
    /// `else`, `and`, or `option`, with its label.
    Divider(String),
    End,
    /// `create`: the participant's header is drawn here instead of at the top.
    Create(usize),
    /// `destroy`: the participant's lifeline ends at its next message, or here.
    Destroy(usize),
}

/// A sequence diagram (MSEQ-2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Sequence {
    pub title: Option<String>,
    pub acc_title: Option<String>,
    pub acc_descr: Option<String>,
    pub participants: Vec<Participant>,
    pub groups: Vec<Group>,
    pub events: Vec<Event>,
}

impl Sequence {
    pub fn messages(&self) -> impl Iterator<Item = &Message> {
        self.events.iter().filter_map(|e| match e {
            Event::Message(m) => Some(m),
            _ => None,
        })
    }
}

/// The result of parsing sequence diagram source: a lossless syntax tree plus diagnostics.
#[derive(Debug, Clone)]
pub struct Parse {
    green: GreenNode,
    diagnostics: Vec<Diagnostic>,
}

impl Parse {
    /// Root of the syntax tree. Its text is always identical to the parsed source.
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Lowers the tree to a [`Sequence`]. The returned diagnostics include the parser's,
    /// ordered by position.
    pub fn lower(&self) -> (Sequence, Vec<Diagnostic>) {
        let (sequence, lowering) = lower::lower(&self.syntax());
        let mut diagnostics = self.diagnostics.clone();
        diagnostics.extend(lowering);
        diagnostics.sort_by_key(|d| d.range.start());
        log::debug!(
            "lowered a sequence diagram: {} participants, {} events; {} diagnostics",
            sequence.participants.len(),
            sequence.events.len(),
            diagnostics.len()
        );
        (sequence, diagnostics)
    }
}

/// Parses sequence diagram source. Never fails; problems are reported as diagnostics.
pub fn parse(src: &str) -> Parse {
    let (green, diagnostics) = parser::parse(src);
    Parse { green, diagnostics }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows() {
        for (s, line, head, both) in [
            ("->", LineStyle::Solid, Head::None, false),
            ("-->", LineStyle::Dotted, Head::None, false),
            ("->>", LineStyle::Solid, Head::Filled, false),
            ("-->>", LineStyle::Dotted, Head::Filled, false),
            ("-x", LineStyle::Solid, Head::Cross, false),
            ("--x", LineStyle::Dotted, Head::Cross, false),
            ("-)", LineStyle::Solid, Head::Open, false),
            ("--)", LineStyle::Dotted, Head::Open, false),
            ("<<->>", LineStyle::Solid, Head::Filled, true),
            ("<<-->>", LineStyle::Dotted, Head::Filled, true),
        ] {
            assert_eq!(Arrow::parse(s), Some(Arrow { line, head, both }), "{s}");
        }
        assert_eq!(Arrow::parse("=>"), None);
        assert_eq!(
            ParticipantKind::from_type("Database"),
            Some(ParticipantKind::Database)
        );
        assert_eq!(ParticipantKind::from_type("cloud"), None);
    }
}
