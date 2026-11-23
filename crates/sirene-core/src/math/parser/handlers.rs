//! The functions the parser builds nodes for: how a command reads its arguments, and what node it
//! makes from them.
//!
//! `functions_data.rs` lists every command KaTeX has, with its argument count and kinds. A command
//! is built here only if `Build::of` names it; any other is reported as not supported yet, before
//! its arguments are read. The commands here: fractions and binomials, `\sqrt`, the style
//! commands, and `\text` with its font variants.

use super::{MathError, Mode, Name, Node, NodeKind, Parser, TokenKind, unsupported, written};
use crate::math::{
    ast::{MathClass, StyleName},
    expander::token_text,
    functions::{self, Function},
    lexer::Token,
    limits,
};

/// What a function builds, and the details that tell the commands of one kind apart.
#[derive(Clone, Copy, Debug)]
enum Build {
    /// `\frac` and the other two-argument fractions.
    Frac {
        bar: bool,
        continued: bool,
        left: Option<&'static str>,
        right: Option<&'static str>,
        /// The style a fraction like `\dfrac` is forced into.
        style: Option<StyleName>,
    },
    Sqrt,
    /// `\displaystyle` and the other three: they style the rest of the group.
    Styling(StyleName),
    /// `\text` and its font variants.
    Text,
    /// `\mathbf` and the other math fonts, with the font's name.
    Font(&'static str),
    /// `\bf` and the other old font switches: they set the rest of the group.
    OldFont(&'static str),
    /// `\boldsymbol` and `\bm`.
    BoldSymbol,
    /// `\mathbin` and the other class commands.
    Class(MathClass),
    /// `\@binrel{x}{y}`: `y` spaced as `x` would be.
    Binrel,
    Overline,
    Underline,
    /// `\big` and the other fixed-size delimiters.
    DelimSizing(u8, MathClass),
    /// `\left ... \right`.
    Left,
    /// `\middle`.
    Middle,
}

impl Build {
    /// The build for the command written `name` (with its backslash), if there is one.
    fn of(name: &str) -> Option<Self> {
        let frac = |bar, continued, left, right, style| Build::Frac {
            bar,
            continued,
            left,
            right,
            style,
        };
        Some(match name {
            "\\frac" => frac(true, false, None, None, None),
            "\\dfrac" => frac(true, false, None, None, Some(StyleName::Display)),
            "\\tfrac" => frac(true, false, None, None, Some(StyleName::Text)),
            "\\cfrac" => frac(true, true, None, None, Some(StyleName::Display)),
            "\\binom" => frac(false, false, Some("("), Some(")"), None),
            "\\dbinom" => frac(false, false, Some("("), Some(")"), Some(StyleName::Display)),
            "\\tbinom" => frac(false, false, Some("("), Some(")"), Some(StyleName::Text)),
            "\\sqrt" => Build::Sqrt,
            "\\displaystyle" => Build::Styling(StyleName::Display),
            "\\textstyle" => Build::Styling(StyleName::Text),
            "\\scriptstyle" => Build::Styling(StyleName::Script),
            "\\scriptscriptstyle" => Build::Styling(StyleName::ScriptScript),
            "\\text" | "\\textrm" | "\\textsf" | "\\texttt" | "\\textnormal" | "\\textbf"
            | "\\textmd" | "\\textit" | "\\textup" | "\\emph" => Build::Text,
            "\\mathrm" => Build::Font("mathrm"),
            "\\mathit" => Build::Font("mathit"),
            "\\mathbf" | "\\bold" => Build::Font("mathbf"),
            "\\mathnormal" => Build::Font("mathnormal"),
            "\\mathsfit" => Build::Font("mathsfit"),
            "\\mathbb" | "\\Bbb" => Build::Font("mathbb"),
            "\\mathcal" => Build::Font("mathcal"),
            "\\mathfrak" | "\\frak" => Build::Font("mathfrak"),
            "\\mathscr" => Build::Font("mathscr"),
            "\\mathsf" => Build::Font("mathsf"),
            "\\mathtt" => Build::Font("mathtt"),
            "\\rm" => Build::OldFont("mathrm"),
            "\\sf" => Build::OldFont("mathsf"),
            "\\tt" => Build::OldFont("mathtt"),
            "\\bf" => Build::OldFont("mathbf"),
            "\\it" => Build::OldFont("mathit"),
            "\\cal" => Build::OldFont("mathcal"),
            "\\boldsymbol" | "\\bm" => Build::BoldSymbol,
            "\\mathord" => Build::Class(MathClass::Ord),
            "\\mathbin" => Build::Class(MathClass::Bin),
            "\\mathrel" => Build::Class(MathClass::Rel),
            "\\mathopen" => Build::Class(MathClass::Open),
            "\\mathclose" => Build::Class(MathClass::Close),
            "\\mathpunct" => Build::Class(MathClass::Punct),
            "\\mathinner" => Build::Class(MathClass::Inner),
            "\\@binrel" => Build::Binrel,
            "\\overline" => Build::Overline,
            "\\underline" => Build::Underline,
            "\\big" => Build::DelimSizing(1, MathClass::Ord),
            "\\Big" => Build::DelimSizing(2, MathClass::Ord),
            "\\bigg" => Build::DelimSizing(3, MathClass::Ord),
            "\\Bigg" => Build::DelimSizing(4, MathClass::Ord),
            "\\bigl" => Build::DelimSizing(1, MathClass::Open),
            "\\Bigl" => Build::DelimSizing(2, MathClass::Open),
            "\\biggl" => Build::DelimSizing(3, MathClass::Open),
            "\\Biggl" => Build::DelimSizing(4, MathClass::Open),
            "\\bigr" => Build::DelimSizing(1, MathClass::Close),
            "\\Bigr" => Build::DelimSizing(2, MathClass::Close),
            "\\biggr" => Build::DelimSizing(3, MathClass::Close),
            "\\Biggr" => Build::DelimSizing(4, MathClass::Close),
            "\\bigm" => Build::DelimSizing(1, MathClass::Rel),
            "\\Bigm" => Build::DelimSizing(2, MathClass::Rel),
            "\\biggm" => Build::DelimSizing(3, MathClass::Rel),
            "\\Biggm" => Build::DelimSizing(4, MathClass::Rel),
            "\\left" => Build::Left,
            "\\middle" => Build::Middle,
            _ => return None,
        })
    }
}

/// The arguments of a function: the required ones in order, and the optional ones, each of which
/// may be absent.
struct Arguments<'a> {
    required: Vec<Node<'a>>,
    optional: Vec<Option<Node<'a>>>,
}

impl<'a> Parser<'a> {
    /// The function `token` names, with its arguments, if it names one. `context` is what the
    /// function stands in: only a function that may be an argument can follow `^` or `_`.
    pub(super) fn parse_function(
        &mut self,
        token: &Token<'a>,
        context: &str,
    ) -> Result<Option<Node<'a>>, MathError> {
        let Some(name) = written(&token.kind) else {
            return Ok(None);
        };
        let Some(function) = functions::lookup(&name) else {
            return Ok(None);
        };
        let Some(build) = Build::of(&name) else {
            return Err(unsupported(&name, token));
        };
        self.consume();
        if context != "atom" && !function.allowed_in_argument {
            return Err(MathError::new(
                format!("Got function '{name}' with no arguments as {context}"),
                token.span.clone(),
            ));
        } else if self.mode == Mode::Text && !function.allowed_in_text {
            return Err(MathError::new(
                format!("Can't use function '{name}' in text mode"),
                token.span.clone(),
            ));
        } else if self.mode == Mode::Math && !function.allowed_in_math {
            return Err(MathError::new(
                format!("Can't use function '{name}' in math mode"),
                token.span.clone(),
            ));
        }
        if self.depth >= limits::MAX_DEPTH {
            return Err(MathError::new(
                format!("groups nested more than {} deep", limits::MAX_DEPTH),
                token.span.clone(),
            ));
        }
        self.depth += 1;
        let built = self
            .parse_arguments(&name, &function)
            .and_then(|args| self.build(build, &name, token, args));
        self.depth -= 1;
        built.map(Some)
    }

    fn parse_arguments(
        &mut self,
        name: &str,
        function: &Function,
    ) -> Result<Arguments<'a>, MathError> {
        let mut args = Arguments {
            required: Vec::new(),
            optional: Vec::new(),
        };
        let total = function.num_args + function.num_optional_args;
        for i in 0..total {
            let optional = i < function.num_optional_args;
            let mut kind = function
                .arg_types
                .as_bytes()
                .get(i)
                .copied()
                .unwrap_or(b'o');
            // `\sqrt` without an index reads its body as one group or symbol.
            if (function.primitive && kind == b'o')
                || (function.kind == "sqrt" && i == 1 && args.optional.first() == Some(&None))
            {
                kind = b'p';
            }
            let arg = self.parse_argument(name, kind, optional)?;
            if optional {
                args.optional.push(arg);
            } else if let Some(arg) = arg {
                args.required.push(arg);
            } else {
                return Err(MathError::new(
                    "Null argument, please report this as a bug",
                    0..0,
                ));
            }
        }
        Ok(args)
    }

    /// One argument, read as its kind says (`Function::arg_types`).
    fn parse_argument(
        &mut self,
        name: &str,
        kind: u8,
        optional: bool,
    ) -> Result<Option<Node<'a>>, MathError> {
        match kind {
            b'o' => self.parse_argument_group(optional, None),
            b'm' => self.parse_argument_group(optional, Some(Mode::Math)),
            b't' => self.parse_argument_group(optional, Some(Mode::Text)),
            b'p' => {
                if optional {
                    return Err(MathError::new(
                        "A primitive argument cannot be optional",
                        0..0,
                    ));
                }
                let context = format!("argument to '{name}'");
                let ahead = self.fetch()?;
                match self.parse_group(&context)? {
                    Some(group) => Ok(Some(group)),
                    None => Err(MathError::new(
                        format!("Expected group as {context}"),
                        ahead.span,
                    )),
                }
            }
            _ => Err(MathError::new(format!("{name} is not supported yet"), 0..0)),
        }
    }

    /// A braced argument, or the single token that stands for one, read as a formula of its own,
    /// in `mode` if that is given. An optional argument is `[...]`, and is `None` when absent.
    fn parse_argument_group(
        &mut self,
        optional: bool,
        mode: Option<Mode>,
    ) -> Result<Option<Node<'a>>, MathError> {
        let Some(arg) = self.expander.scan_argument(optional)? else {
            return Ok(None);
        };
        let outer = self.mode;
        if let Some(mode) = mode {
            self.switch_mode(mode);
        }
        self.expander.begin_group(&arg.span)?;
        let body = self.parse_expression(false)?;
        // The argument ends at the `Eof` that `scan_argument` put after it.
        self.expect("EOF")?;
        self.expander.end_group(&arg.end)?;
        let node = Node {
            kind: NodeKind::Group {
                body,
                semisimple: false,
            },
            mode: self.mode,
            span: arg.span,
        };
        if mode.is_some() {
            self.switch_mode(outer);
        }
        Ok(Some(node))
    }

    fn build(
        &mut self,
        build: Build,
        name: &str,
        token: &Token<'a>,
        mut args: Arguments<'a>,
    ) -> Result<Node<'a>, MathError> {
        let start = token.span.start;
        let mode = self.mode;
        match build {
            Build::Frac {
                bar,
                continued,
                left,
                right,
                style,
            } => {
                let (Some(denom), Some(numer)) = (args.required.pop(), args.required.pop()) else {
                    return Err(missing(name));
                };
                let span = start..denom.span.end;
                let frac = Node {
                    kind: NodeKind::Frac {
                        numer: Box::new(numer),
                        denom: Box::new(denom),
                        continued,
                        bar,
                        left,
                        right,
                    },
                    mode,
                    span: span.clone(),
                };
                Ok(match style {
                    Some(style) => Node {
                        kind: NodeKind::Styling {
                            style,
                            body: vec![frac],
                        },
                        mode,
                        span,
                    },
                    None => frac,
                })
            }
            Build::Sqrt => {
                let Some(body) = args.required.pop() else {
                    return Err(missing(name));
                };
                let index = args.optional.pop().flatten();
                Ok(Node {
                    span: start..body.span.end,
                    kind: NodeKind::Sqrt {
                        body: Box::new(body),
                        index: index.map(Box::new),
                    },
                    mode,
                })
            }
            Build::Styling(style) => {
                // The body is the rest of the group; an infix command ends it.
                let body = self.parse_expression(true)?;
                let end = body.last().map_or(token.span.end, |n| n.span.end);
                Ok(Node {
                    kind: NodeKind::Styling { style, body },
                    mode,
                    span: start..end,
                })
            }
            Build::Text => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let span = start..arg.span.end;
                // The argument is a group; its nodes are the text.
                let body = match arg.kind {
                    NodeKind::Group { body, .. } => body,
                    _ => vec![arg],
                };
                let font = match token.kind {
                    TokenKind::ControlWord(word) => word,
                    _ => return Err(missing(name)),
                };
                Ok(Node {
                    kind: NodeKind::Text { body, font },
                    mode,
                    span,
                })
            }
            Build::Font(font) => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                Ok(Node {
                    span: start..arg.span.end,
                    kind: NodeKind::Font {
                        font,
                        body: Box::new(arg.normalized()),
                    },
                    mode,
                })
            }
            Build::OldFont(font) => {
                // The rest of the group is set in the font.
                let body = self.parse_expression(true)?;
                let span = body
                    .first()
                    .zip(body.last())
                    .map_or(token.span.end..token.span.end, |(first, last)| {
                        first.span.start..last.span.end
                    });
                let group = Node {
                    kind: NodeKind::Group {
                        body,
                        semisimple: false,
                    },
                    mode: self.mode,
                    span: span.clone(),
                };
                Ok(Node {
                    kind: NodeKind::Font {
                        font,
                        body: Box::new(group),
                    },
                    mode,
                    span: start..span.end,
                })
            }
            Build::BoldSymbol => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let (class, character_box) = (arg.binrel_class(), arg.is_character_box());
                let span = start..arg.span.end;
                let font = Node {
                    kind: NodeKind::Font {
                        font: "boldsymbol",
                        body: Box::new(arg),
                    },
                    mode,
                    span: span.clone(),
                };
                Ok(Node {
                    kind: NodeKind::MClass {
                        class,
                        body: vec![font],
                        character_box,
                    },
                    mode,
                    span,
                })
            }
            Build::Class(class) => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let (span, character_box) = (start..arg.span.end, arg.is_character_box());
                Ok(Node {
                    kind: NodeKind::MClass {
                        class,
                        body: arg.into_body(),
                        character_box,
                    },
                    mode,
                    span,
                })
            }
            Build::Binrel => {
                let (Some(body), Some(kind)) = (args.required.pop(), args.required.pop()) else {
                    return Err(missing(name));
                };
                let (span, character_box) = (start..body.span.end, body.is_character_box());
                Ok(Node {
                    kind: NodeKind::MClass {
                        class: kind.binrel_class(),
                        body: body.into_body(),
                        character_box,
                    },
                    mode,
                    span,
                })
            }
            Build::DelimSizing(size, class) => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let span = start..arg.span.end;
                let delim = check_delimiter(&arg.normalized(), name)?;
                Ok(Node {
                    kind: NodeKind::DelimSizing { size, class, delim },
                    mode,
                    span,
                })
            }
            Build::Middle => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let span = start..arg.span.end;
                let delim = check_delimiter(&arg, name)?;
                if self.leftright_depth == 0 {
                    return Err(MathError::new(
                        "\\middle without preceding \\left",
                        token.span.clone(),
                    ));
                }
                Ok(Node {
                    kind: NodeKind::Middle { delim },
                    mode,
                    span,
                })
            }
            Build::Left => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let left = check_delimiter(&arg, name)?;
                // The body runs to the matching `\right`, which ends an expression.
                self.leftright_depth += 1;
                let body = self.parse_expression(false);
                self.leftright_depth -= 1;
                let body = body?;
                // The `\right` is read here, as the `\left` reads it, not by the expression.
                let close = self.fetch()?;
                if token_text(&close.kind) != "\\right" {
                    return Err(MathError::new(
                        format!("Expected '\\right', got '{}'", token_text(&close.kind)),
                        close.span,
                    ));
                }
                self.consume();
                let right_arg = self.parse_argument("\\right", b'p', false)?;
                let Some(right_arg) = right_arg else {
                    return Err(missing("\\right"));
                };
                let right = check_delimiter(&right_arg, "\\right")?;
                Ok(Node {
                    kind: NodeKind::LeftRight { body, left, right },
                    mode,
                    span: start..right_arg.span.end,
                })
            }
            Build::Overline | Build::Underline => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let span = start..arg.span.end;
                let body = Box::new(arg);
                Ok(Node {
                    kind: if matches!(build, Build::Overline) {
                        NodeKind::Overline { body }
                    } else {
                        NodeKind::Underline { body }
                    },
                    mode,
                    span,
                })
            }
        }
    }
}

/// The commands and characters a delimiter command takes.
const DELIMITERS: &[&str] = &[
    "(",
    "\\lparen",
    ")",
    "\\rparen",
    "[",
    "\\lbrack",
    "]",
    "\\rbrack",
    "\\{",
    "\\lbrace",
    "\\}",
    "\\rbrace",
    "\\lfloor",
    "\\rfloor",
    "\u{230a}",
    "\u{230b}",
    "\\lceil",
    "\\rceil",
    "\u{2308}",
    "\u{2309}",
    "<",
    ">",
    "\\langle",
    "\u{27e8}",
    "\\rangle",
    "\u{27e9}",
    "\\lt",
    "\\gt",
    "\\lvert",
    "\\rvert",
    "\\lVert",
    "\\rVert",
    "\\lgroup",
    "\\rgroup",
    "\u{27ee}",
    "\u{27ef}",
    "\\lmoustache",
    "\\rmoustache",
    "\u{23b0}",
    "\u{23b1}",
    "/",
    "\\backslash",
    "|",
    "\\vert",
    "\\|",
    "\\Vert",
    "\\uparrow",
    "\\Uparrow",
    "\\downarrow",
    "\\Downarrow",
    "\\updownarrow",
    "\\Updownarrow",
    ".",
];

/// The delimiter a node stands for, as the source writes it, if it is one `name` accepts.
fn check_delimiter(node: &Node<'_>, name: &str) -> Result<String, MathError> {
    let NodeKind::Symbol { name: symbol, .. } = &node.kind else {
        return Err(MathError::new(
            format!("Invalid delimiter type '{}'", node.kind.type_name()),
            node.span.clone(),
        ));
    };
    let text = match symbol {
        Name::Char(text) => (*text).to_string(),
        Name::Control(control) => format!("\\{control}"),
    };
    if DELIMITERS.contains(&text.as_str()) {
        Ok(text)
    } else {
        Err(MathError::new(
            format!("Invalid delimiter '{text}' after '{name}'"),
            node.span.clone(),
        ))
    }
}

/// A function's handler found fewer arguments than the table says it takes.
fn missing(name: &str) -> MathError {
    MathError::new(format!("Missing arguments for {name}"), 0..0)
}
