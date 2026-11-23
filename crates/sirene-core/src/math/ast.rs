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
    Symbol { group: AtomGroup, name: Name<'a> },
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
    Text { body: Vec<Node<'a>>, font: &'a str },
}

/// The four math styles of TeX.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StyleName {
    Display,
    Text,
    Script,
    ScriptScript,
}
