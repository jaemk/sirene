//! The parse tree (LMATH-2, step 3): what the formula says, before it is measured.
//!
//! One enum of node kinds, each node carrying the mode it was read in and the byte range of the
//! source it came from. The kinds grow with the parser, in the order of LMATH-9.

use std::ops::Range;

use super::symbols::{AtomGroup, Mode, Name};

/// A node of the parse tree.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Node<'a> {
    pub kind: NodeKind<'a>,
    pub mode: Mode,
    /// The source this node was read from.
    pub span: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum NodeKind<'a> {
    /// One symbol: a letter, digit, operator, relation, delimiter, or space command. The group
    /// says how TeX spaces it; the name is as the source writes it.
    Symbol {
        group: AtomGroup,
        name: Name<'a>,
    },
    /// Nodes read in order, from `{...}`. A `\begingroup ... \endgroup` group is the same but
    /// `semisimple`.
    Group {
        body: Vec<Node<'a>>,
        semisimple: bool,
    },
    /// A base with a superscript, a subscript, or both. The base is empty for `^2` alone.
    SupSub {
        base: Option<Box<Node<'a>>>,
        sup: Option<Box<Node<'a>>>,
        sub: Option<Box<Node<'a>>>,
    },
    /// A fraction or binomial: `\frac`, `\binom`, and their display and text forms. The
    /// delimiters are drawn around it; `\cfrac` is the continued form.
    Frac {
        numer: Box<Node<'a>>,
        denom: Box<Node<'a>>,
        continued: bool,
        bar: bool,
        left: Option<&'static str>,
        right: Option<&'static str>,
    },
    /// A root, with an optional index (`\sqrt[3]{x}`).
    Sqrt {
        body: Box<Node<'a>>,
        index: Option<Box<Node<'a>>>,
    },
    /// Nodes set in a given style (`\displaystyle` and the rest, which run to the end of the
    /// group, and `\dfrac`, which wraps one fraction).
    Styling {
        style: StyleName,
        body: Vec<Node<'a>>,
    },
    /// Text set in text mode: `\text{...}` and the font variants. `font` is the command's name
    /// without its backslash.
    Text {
        body: Vec<Node<'a>>,
        font: &'a str,
    },
    /// A math font for its body: `\mathbf{x}`, `\mathbb`, and the old `\bf` (which is
    /// `mathbf`). `font` is the name without the backslash, with aliases resolved (`\Bbb` is
    /// `mathbb`).
    Font {
        font: &'a str,
        body: Box<Node<'a>>,
    },
    /// The body spaced as an atom of the given class: `\mathbin{...}`, and `\boldsymbol`, which
    /// takes the class of its argument. `character_box` says the body is one character.
    MClass {
        class: MathClass,
        body: Vec<Node<'a>>,
        character_box: bool,
    },
    Overline {
        body: Box<Node<'a>>,
    },
    Underline {
        body: Box<Node<'a>>,
    },
    /// A delimiter of a fixed size: `\big(`, `\Bigr]`. The size is 1 to 4, the class says how it
    /// is spaced, and the delimiter is as the source writes it (`(`, `\langle`).
    DelimSizing {
        size: u8,
        class: MathClass,
        delim: String,
    },
    /// `\left ... \right` with the delimiters, which grow to fit the body.
    LeftRight {
        body: Vec<Node<'a>>,
        left: String,
        right: String,
    },
    /// `\middle|` inside `\left ... \right`.
    Middle {
        delim: String,
    },
}

impl NodeKind<'_> {
    /// The name KaTeX gives the node's type, which its error messages use.
    pub(crate) fn type_name(&self) -> &'static str {
        match self {
            NodeKind::Symbol { group, .. } => match group {
                AtomGroup::MathOrd => "mathord",
                AtomGroup::TextOrd => "textord",
                AtomGroup::Spacing => "spacing",
                AtomGroup::Op => "op-token",
                AtomGroup::Accent => "accent-token",
                _ => "atom",
            },
            NodeKind::Group { .. } => "ordgroup",
            NodeKind::SupSub { .. } => "supsub",
            NodeKind::Frac { .. } => "genfrac",
            NodeKind::Sqrt { .. } => "sqrt",
            NodeKind::Styling { .. } => "styling",
            NodeKind::Text { .. } => "text",
            NodeKind::Font { .. } => "font",
            NodeKind::MClass { .. } => "mclass",
            NodeKind::Overline { .. } => "overline",
            NodeKind::Underline { .. } => "underline",
            NodeKind::DelimSizing { .. } => "delimsizing",
            NodeKind::LeftRight { .. } => "leftright",
            NodeKind::Middle { .. } => "middle",
        }
    }
}

/// The atom classes `\mathbin` and the like set (TeX's `\mathord` through `\mathinner`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MathClass {
    Ord,
    Bin,
    Rel,
    Open,
    Close,
    Punct,
    Inner,
}

impl<'a> Node<'a> {
    /// A group of one node stands for that node.
    pub(crate) fn normalized(self) -> Node<'a> {
        match self.kind {
            NodeKind::Group {
                mut body,
                semisimple,
            } => {
                if body.len() == 1
                    && let Some(only) = body.pop()
                {
                    return only;
                }
                Node {
                    kind: NodeKind::Group { body, semisimple },
                    mode: self.mode,
                    span: self.span,
                }
            }
            _ => self,
        }
    }

    /// The nodes of a group, or the node alone.
    pub(crate) fn into_body(self) -> Vec<Node<'a>> {
        match self.kind {
            NodeKind::Group { body, .. } => body,
            _ => vec![self],
        }
    }

    /// The node inside any groups of one node and fonts around it.
    fn base(&self) -> &Node<'a> {
        match &self.kind {
            NodeKind::Group { body, .. } if body.len() == 1 => body[0].base(),
            NodeKind::Font { body, .. } => body.base(),
            _ => self,
        }
    }

    /// Whether the node is one character, as TeX's rules ask of a "character box".
    pub(crate) fn is_character_box(&self) -> bool {
        matches!(
            self.base().kind,
            NodeKind::Symbol {
                group: AtomGroup::MathOrd
                    | AtomGroup::TextOrd
                    | AtomGroup::Bin
                    | AtomGroup::Close
                    | AtomGroup::Inner
                    | AtomGroup::Open
                    | AtomGroup::Punct
                    | AtomGroup::Rel,
                ..
            }
        )
    }

    /// The class `\boldsymbol` and `\@binrel` take from an argument: a binary operator or
    /// relation keeps its class, anything else is ordinary.
    pub(crate) fn binrel_class(&self) -> MathClass {
        let atom = match &self.kind {
            NodeKind::Group { body, .. } if !body.is_empty() => &body[0],
            _ => self,
        };
        match atom.kind {
            NodeKind::Symbol {
                group: AtomGroup::Bin,
                ..
            } => MathClass::Bin,
            NodeKind::Symbol {
                group: AtomGroup::Rel,
                ..
            } => MathClass::Rel,
            _ => MathClass::Ord,
        }
    }
}

/// The four math styles of TeX.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StyleName {
    Display,
    Text,
    Script,
    ScriptScript,
}
