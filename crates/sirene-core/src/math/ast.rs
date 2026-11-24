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
        /// The thickness of the bar, if the source gave one (`\above`, `\genfrac`).
        bar_size: Option<Measurement>,
        left: Option<String>,
        right: Option<String>,
    },
    /// An infix fraction command (`\over`, `\above`) as read: the expression around it is
    /// split at it and it becomes a `Frac`, so it stays in the tree only where no expression
    /// holds it. `replace_with` is the command it stands for.
    Infix {
        replace_with: &'static str,
        size: Option<Measurement>,
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
        /// The font is reset too: the body of `\raisebox` and the box commands is text.
        reset_font: bool,
    },
    /// Text set in text mode: `\text{...}` and the font variants. `font` is the command's name
    /// without its backslash.
    Text {
        body: Vec<Node<'a>>,
        /// `None` for the name of a command left out (`\href` without trust), which has no font
        /// of its own.
        font: Option<&'a str>,
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
        /// The color the `\right` delimiter takes, if a `\color` before it set one.
        right_color: Option<String>,
    },
    /// A base with an accent over it: `\hat{x}`, `\widehat`, a text accent (`\'e`), or a
    /// combining mark in the source. `label` is the command, with its backslash.
    Accent {
        label: String,
        stretchy: bool,
        shifty: bool,
        base: Box<Node<'a>>,
    },
    /// A base with an accent under it: `\underrightarrow{x}`.
    AccentUnder {
        label: String,
        base: Box<Node<'a>>,
    },
    /// A large operator or a named one: `\sum`, `\int`, `\lim`, `\sin`, and `\mathop{...}`.
    /// `limits` puts scripts above and below in display style; `always_handle_supsub` says
    /// `\limits` or `\nolimits` followed it, which fixes that choice. `symbol` means a glyph
    /// that grows in display style. `suppress_base_shift` is set only by `\overset`,
    /// `\underset` and `\stackrel`, which build an operator of their base: it says whether the
    /// base keeps its place instead of centering on the axis (`\stackrel` does not keep it).
    Op {
        limits: bool,
        always_handle_supsub: bool,
        symbol: bool,
        suppress_base_shift: Option<bool>,
        content: OpContent<'a>,
    },
    /// `\operatorname{...}`, and the starred form that takes limits.
    OperatorName {
        body: Vec<Node<'a>>,
        always_handle_supsub: bool,
        limits: bool,
    },
    /// A kern: `\kern1em`, `\mskip3mu`, `\hskip`.
    Kern {
        dimension: Measurement,
    },
    /// A rule: `\rule[shift]{width}{height}`.
    Rule {
        shift: Option<Measurement>,
        width: Measurement,
        height: Measurement,
    },
    /// Nodes in a color: `\textcolor{red}{x}`, and `\color{red}` for the rest of the group.
    Color {
        color: String,
        body: Vec<Node<'a>>,
    },
    /// `\phantom{x}`: takes the space of the body and draws nothing.
    Phantom {
        body: Vec<Node<'a>>,
    },
    /// `\vphantom{x}`: takes the height and depth of the body only.
    VPhantom {
        body: Box<Node<'a>>,
    },
    /// `\smash[tb]{x}`: the body, with its height or depth (or both) set to nothing.
    Smash {
        body: Box<Node<'a>>,
        height: bool,
        depth: bool,
    },
    /// `\mathllap`, `\mathrlap`, `\mathclap`: the body takes no width. `alignment` is
    /// `llap`, `rlap`, or `clap`.
    Lap {
        alignment: &'a str,
        body: Box<Node<'a>>,
    },
    /// `\raisebox{dy}{x}`.
    RaiseBox {
        dy: Measurement,
        body: Box<Node<'a>>,
    },
    /// A size command (`\Huge`, `\small`) and the rest of the group. `size` is 1 (`\tiny`)
    /// to 11 (`\Huge`).
    Sizing {
        size: u8,
        body: Vec<Node<'a>>,
    },
    /// A size argument as read, before the function that wanted it takes it apart.
    Size {
        value: Measurement,
        is_blank: bool,
    },
    /// A color argument as read.
    ColorToken(String),
    /// Nothing: `\relax`. It is not kept in an expression, and a script after it has no base.
    Internal,
    /// A table: `array`, the matrices, `cases`, and the aligned environments. `body` is rows of
    /// cells, each cell a group (in a styling node for the environments that fix a style).
    Array(Box<Array<'a>>),
    /// `\\` outside a table, or `\\[1em]`: a line break.
    Cr {
        new_line: bool,
        size: Option<Measurement>,
    },
    /// `\end{name}`, read by the `\begin` that it closes.
    Environment {
        name: String,
    },
    /// A formula with an equation tag (`\tag{1}`), which only display mode allows.
    Tag {
        body: Vec<Node<'a>>,
        tag: Vec<Node<'a>>,
    },
    /// `\middle|` inside `\left ... \right`.
    Middle {
        delim: String,
    },
    /// A box, a line or a background around the body: `\fbox`, `\colorbox`, `\fcolorbox`,
    /// `\cancel` and its two variants, `\sout`, `\angl`, `\phase`. `label` is the command, with
    /// its backslash.
    Enclose {
        label: String,
        background_color: Option<String>,
        border_color: Option<String>,
        body: Box<Node<'a>>,
    },
    /// `\pmb{x}`: the body in poor man's bold, spaced as the class of its argument.
    Pmb {
        class: MathClass,
        body: Vec<Node<'a>>,
    },
    /// `\hbox{...}`: text, set in no font of its own.
    HBox {
        body: Vec<Node<'a>>,
    },
    /// `\vcenter{...}`: the body centered on the math axis.
    VCenter {
        body: Box<Node<'a>>,
    },
    /// `\mathchoice{d}{t}{s}{ss}`: one of the four, by the style it is set in.
    MathChoice {
        display: Vec<Node<'a>>,
        text: Vec<Node<'a>>,
        script: Vec<Node<'a>>,
        script_script: Vec<Node<'a>>,
    },
    /// `\reflectbox{x}` and `\mathreflectbox{x}`: the body mirrored.
    ReflectBox {
        body: Box<Node<'a>>,
    },
    /// `\verb|text|`, set in text mode as written.
    Verb {
        body: &'a str,
        star: bool,
    },
    /// `\html@mathml{html}{mathml}`: the first is drawn, the second is the MathML form, which
    /// sirene does not produce.
    HtmlMathml {
        html: Vec<Node<'a>>,
        mathml: Vec<Node<'a>>,
    },
    /// A character made from a number: `\@char{65}`, which `\char"41` expands to.
    CharCode(char),
    /// A raw string argument as read (`\htmlId{a}{...}`), before the function that wanted it
    /// takes it.
    Raw {
        string: String,
    },
    /// A URL argument as read (`\href{...}{...}`).
    Url {
        url: String,
    },
    /// An arrow that stretches to its label: `\xrightarrow[below]{above}`. `label` is the
    /// command, with its backslash.
    XArrow {
        label: String,
        body: Box<Node<'a>>,
        below: Option<Box<Node<'a>>>,
    },
    /// A brace or bracket over or under a base: `\overbrace{x}`, `\underbracket{x}`.
    HorizBrace {
        label: String,
        is_over: bool,
        base: Box<Node<'a>>,
    },
    /// The label beside a vertical arrow of a `{CD}` diagram.
    CdLabel {
        side: CdSide,
        label: Box<Node<'a>>,
    },
    /// A vertical arrow of a `{CD}` diagram with its two labels, which sit beside it.
    CdLabelParent {
        fragment: Box<Node<'a>>,
    },
}

/// Which side of a vertical arrow a `{CD}` label is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CdSide {
    Left,
    Right,
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
            NodeKind::Infix { .. } => "infix",
            NodeKind::Enclose { .. } => "enclose",
            NodeKind::Pmb { .. } => "pmb",
            NodeKind::HBox { .. } => "hbox",
            NodeKind::VCenter { .. } => "vcenter",
            NodeKind::MathChoice { .. } => "mathchoice",
            NodeKind::ReflectBox { .. } => "reflectbox",
            NodeKind::Verb { .. } => "verb",
            NodeKind::HtmlMathml { .. } => "htmlmathml",
            NodeKind::CharCode(_) => "textord",
            NodeKind::Raw { .. } => "raw",
            NodeKind::Url { .. } => "url",
            NodeKind::XArrow { .. } => "xArrow",
            NodeKind::HorizBrace { .. } => "horizBrace",
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
            NodeKind::Array(_) => "array",
            NodeKind::Cr { .. } => "cr",
            NodeKind::Environment { .. } => "environment",
            NodeKind::Tag { .. } => "tag",
            NodeKind::Kern { .. } => "kern",
            NodeKind::Rule { .. } => "rule",
            NodeKind::Color { .. } => "color",
            NodeKind::Phantom { .. } => "phantom",
            NodeKind::VPhantom { .. } => "vphantom",
            NodeKind::Smash { .. } => "smash",
            NodeKind::Lap { .. } => "lap",
            NodeKind::RaiseBox { .. } => "raisebox",
            NodeKind::Sizing { .. } => "sizing",
            NodeKind::Size { .. } => "size",
            NodeKind::ColorToken(_) => "color-token",
            NodeKind::Internal => "internal",
            NodeKind::Op { .. } => "op",
            NodeKind::OperatorName { .. } => "operatorname",
            NodeKind::Accent { .. } => "accent",
            NodeKind::AccentUnder { .. } => "accentUnder",
            NodeKind::CdLabel { .. } => "cdlabel",
            NodeKind::CdLabelParent { .. } => "cdlabelparent",
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

    /// The node inside any groups or colors of one node and fonts around it (KaTeX's
    /// `getBaseElem`).
    fn base(&self) -> &Node<'a> {
        match &self.kind {
            NodeKind::Group { body, .. } | NodeKind::Color { body, .. } if body.len() == 1 => {
                body[0].base()
            }
            NodeKind::Font { body, .. } => body.base(),
            _ => self,
        }
    }

    /// Whether the node is one character, as TeX's rules ask of a "character box".
    pub(crate) fn is_character_box(&self) -> bool {
        matches!(
            self.base().kind,
            NodeKind::CharCode(_)
                | NodeKind::Symbol {
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

/// What a column of a table looks like.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AlignSpec {
    /// A column aligned `l`, `c`, or `r`, with optional gaps (in em) before and after.
    Align {
        align: char,
        pregap: Option<f64>,
        postgap: Option<f64>,
    },
    /// A rule between columns: `|` or `:`.
    Separator(char),
}

/// How the columns of a table are spaced, for the aligned environments and `smallmatrix`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ColSeparation {
    Align,
    Alignat,
    Gather,
    Small,
    /// A `{CD}` diagram, whose rows are `3ex` apart.
    Cd,
}

/// A row's equation tag: whether it is numbered automatically, or the nodes of its `\tag`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RowTag<'a> {
    Numbered(bool),
    Custom(Vec<Node<'a>>),
}

/// A table of nodes.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Array<'a> {
    pub add_jot: bool,
    pub arraystretch: f64,
    pub body: Vec<Vec<Node<'a>>>,
    pub cols: Option<Vec<AlignSpec>>,
    pub row_gaps: Vec<Option<Measurement>>,
    pub hskip_before_and_after: Option<bool>,
    pub hlines_before_row: Vec<Vec<bool>>,
    pub col_separation: Option<ColSeparation>,
    pub tags: Option<Vec<RowTag<'a>>>,
    pub leqno: Option<bool>,
}

/// A length: a number and a unit (`1.5em`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Measurement {
    pub number: f64,
    /// Two lowercase letters, one of KaTeX's units.
    pub unit: String,
}

/// What an operator shows.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum OpContent<'a> {
    /// A named operator, with its backslash (`\sum`), also for a Unicode character (`\u{2211}`
    /// is `\sum`).
    Name(String),
    /// The body of `\mathop`.
    Body(Vec<Node<'a>>),
}

/// The four math styles of TeX.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StyleName {
    Display,
    Text,
    Script,
    ScriptScript,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(kind: NodeKind<'static>) -> Node<'static> {
        Node {
            kind,
            mode: Mode::Math,
            span: 0..1,
        }
    }

    fn x() -> Node<'static> {
        node(NodeKind::Symbol {
            group: AtomGroup::MathOrd,
            name: Name::Char("x"),
        })
    }

    fn bx() -> Box<Node<'static>> {
        Box::new(x())
    }

    fn em() -> Measurement {
        Measurement {
            number: 1.0,
            unit: "em".to_string(),
        }
    }

    fn symbol(group: AtomGroup) -> NodeKind<'static> {
        NodeKind::Symbol {
            group,
            name: Name::Char("x"),
        }
    }

    /// Every kind, with the type name KaTeX's error messages give it.
    #[test]
    fn type_names_are_katex_s() {
        let array = Array {
            add_jot: false,
            arraystretch: 1.0,
            body: Vec::new(),
            cols: None,
            row_gaps: Vec::new(),
            hskip_before_and_after: None,
            hlines_before_row: Vec::new(),
            col_separation: None,
            tags: None,
            leqno: None,
        };
        let cases: Vec<(NodeKind<'static>, &str)> = vec![
            (symbol(AtomGroup::MathOrd), "mathord"),
            (symbol(AtomGroup::TextOrd), "textord"),
            (symbol(AtomGroup::Spacing), "spacing"),
            (symbol(AtomGroup::Op), "op-token"),
            (symbol(AtomGroup::Accent), "accent-token"),
            (symbol(AtomGroup::Bin), "atom"),
            (symbol(AtomGroup::Rel), "atom"),
            (symbol(AtomGroup::Open), "atom"),
            (symbol(AtomGroup::Close), "atom"),
            (symbol(AtomGroup::Punct), "atom"),
            (symbol(AtomGroup::Inner), "atom"),
            (
                NodeKind::Group {
                    body: vec![],
                    semisimple: false,
                },
                "ordgroup",
            ),
            (
                NodeKind::SupSub {
                    base: None,
                    sup: Some(bx()),
                    sub: None,
                },
                "supsub",
            ),
            (
                NodeKind::Frac {
                    numer: bx(),
                    denom: bx(),
                    continued: false,
                    bar: true,
                    bar_size: None,
                    left: None,
                    right: None,
                },
                "genfrac",
            ),
            (
                NodeKind::Infix {
                    replace_with: "\\frac",
                    size: None,
                },
                "infix",
            ),
            (
                NodeKind::Sqrt {
                    body: bx(),
                    index: None,
                },
                "sqrt",
            ),
            (
                NodeKind::Styling {
                    style: StyleName::Script,
                    body: vec![],
                    reset_font: false,
                },
                "styling",
            ),
            (
                NodeKind::Text {
                    body: vec![],
                    font: None,
                },
                "text",
            ),
            (
                NodeKind::Font {
                    font: "mathbf",
                    body: bx(),
                },
                "font",
            ),
            (
                NodeKind::MClass {
                    class: MathClass::Bin,
                    body: vec![],
                    character_box: false,
                },
                "mclass",
            ),
            (NodeKind::Overline { body: bx() }, "overline"),
            (NodeKind::Underline { body: bx() }, "underline"),
            (
                NodeKind::DelimSizing {
                    size: 1,
                    class: MathClass::Open,
                    delim: "(".to_string(),
                },
                "delimsizing",
            ),
            (
                NodeKind::LeftRight {
                    body: vec![],
                    left: "(".to_string(),
                    right: ")".to_string(),
                    right_color: None,
                },
                "leftright",
            ),
            (
                NodeKind::Accent {
                    label: "\\hat".to_string(),
                    stretchy: false,
                    shifty: true,
                    base: bx(),
                },
                "accent",
            ),
            (
                NodeKind::AccentUnder {
                    label: "\\utilde".to_string(),
                    base: bx(),
                },
                "accentUnder",
            ),
            (
                NodeKind::Op {
                    limits: true,
                    always_handle_supsub: false,
                    symbol: true,
                    suppress_base_shift: None,
                    content: OpContent::Name("\\sum".to_string()),
                },
                "op",
            ),
            (
                NodeKind::OperatorName {
                    body: vec![],
                    always_handle_supsub: false,
                    limits: false,
                },
                "operatorname",
            ),
            (NodeKind::Kern { dimension: em() }, "kern"),
            (
                NodeKind::Rule {
                    shift: None,
                    width: em(),
                    height: em(),
                },
                "rule",
            ),
            (
                NodeKind::Color {
                    color: "red".to_string(),
                    body: vec![],
                },
                "color",
            ),
            (NodeKind::Phantom { body: vec![] }, "phantom"),
            (NodeKind::VPhantom { body: bx() }, "vphantom"),
            (
                NodeKind::Smash {
                    body: bx(),
                    height: true,
                    depth: true,
                },
                "smash",
            ),
            (
                NodeKind::Lap {
                    alignment: "llap",
                    body: bx(),
                },
                "lap",
            ),
            (
                NodeKind::RaiseBox {
                    dy: em(),
                    body: bx(),
                },
                "raisebox",
            ),
            (
                NodeKind::Sizing {
                    size: 6,
                    body: vec![],
                },
                "sizing",
            ),
            (
                NodeKind::Size {
                    value: em(),
                    is_blank: false,
                },
                "size",
            ),
            (NodeKind::ColorToken("red".to_string()), "color-token"),
            (NodeKind::Internal, "internal"),
            (NodeKind::Array(Box::new(array)), "array"),
            (
                NodeKind::Cr {
                    new_line: true,
                    size: None,
                },
                "cr",
            ),
            (
                NodeKind::Environment {
                    name: "matrix".to_string(),
                },
                "environment",
            ),
            (
                NodeKind::Tag {
                    body: vec![],
                    tag: vec![],
                },
                "tag",
            ),
            (
                NodeKind::Middle {
                    delim: "|".to_string(),
                },
                "middle",
            ),
            (
                NodeKind::Enclose {
                    label: "\\fbox".to_string(),
                    background_color: None,
                    border_color: None,
                    body: bx(),
                },
                "enclose",
            ),
            (
                NodeKind::Pmb {
                    class: MathClass::Ord,
                    body: vec![],
                },
                "pmb",
            ),
            (NodeKind::HBox { body: vec![] }, "hbox"),
            (NodeKind::VCenter { body: bx() }, "vcenter"),
            (
                NodeKind::MathChoice {
                    display: vec![],
                    text: vec![],
                    script: vec![],
                    script_script: vec![],
                },
                "mathchoice",
            ),
            (NodeKind::ReflectBox { body: bx() }, "reflectbox"),
            (
                NodeKind::Verb {
                    body: "x",
                    star: false,
                },
                "verb",
            ),
            (
                NodeKind::HtmlMathml {
                    html: vec![],
                    mathml: vec![],
                },
                "htmlmathml",
            ),
            (NodeKind::CharCode('A'), "textord"),
            (
                NodeKind::Raw {
                    string: "a".to_string(),
                },
                "raw",
            ),
            (
                NodeKind::Url {
                    url: "a".to_string(),
                },
                "url",
            ),
            (
                NodeKind::XArrow {
                    label: "\\xrightarrow".to_string(),
                    body: bx(),
                    below: None,
                },
                "xArrow",
            ),
            (
                NodeKind::HorizBrace {
                    label: "\\overbrace".to_string(),
                    is_over: true,
                    base: bx(),
                },
                "horizBrace",
            ),
            (
                NodeKind::CdLabel {
                    side: CdSide::Left,
                    label: bx(),
                },
                "cdlabel",
            ),
            (NodeKind::CdLabelParent { fragment: bx() }, "cdlabelparent"),
        ];
        let mut wrong = String::new();
        for (kind, want) in &cases {
            let got = kind.type_name();
            if got != *want {
                wrong.push_str(&format!("{kind:?}: {got} want {want}\n"));
            }
        }
        assert!(wrong.is_empty(), "\n{wrong}");
    }

    #[test]
    fn a_group_of_one_stands_for_its_node() {
        let group = |body| {
            node(NodeKind::Group {
                body,
                semisimple: false,
            })
        };
        assert_eq!(group(vec![x()]).normalized(), x());
        let two = group(vec![x(), x()]);
        assert_eq!(two.clone().normalized(), two);
        assert_eq!(x().normalized(), x());
        assert_eq!(group(vec![x(), x()]).into_body().len(), 2);
        assert_eq!(x().into_body(), vec![x()]);
    }

    #[test]
    fn character_boxes_and_binrel_classes() {
        let colored = node(NodeKind::Color {
            color: "red".to_string(),
            body: vec![x()],
        });
        assert!(colored.is_character_box());
        let font = node(NodeKind::Font {
            font: "mathbf",
            body: bx(),
        });
        assert!(font.is_character_box());
        assert!(node(NodeKind::CharCode('A')).is_character_box());
        assert!(!node(symbol(AtomGroup::Op)).is_character_box());
        let two = node(NodeKind::Color {
            color: "red".to_string(),
            body: vec![x(), x()],
        });
        assert!(!two.is_character_box());

        assert_eq!(node(symbol(AtomGroup::Bin)).binrel_class(), MathClass::Bin);
        assert_eq!(node(symbol(AtomGroup::Rel)).binrel_class(), MathClass::Rel);
        assert_eq!(x().binrel_class(), MathClass::Ord);
        let rel_first = node(NodeKind::Group {
            body: vec![node(symbol(AtomGroup::Rel)), x()],
            semisimple: false,
        });
        assert_eq!(rel_first.binrel_class(), MathClass::Rel);
        let empty = node(NodeKind::Group {
            body: vec![],
            semisimple: false,
        });
        assert_eq!(empty.binrel_class(), MathClass::Ord);
    }
}
