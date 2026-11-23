//! The functions the parser builds nodes for: how a command reads its arguments, and what node it
//! makes from them.
//!
//! `functions_data.rs` lists every command KaTeX has, with its argument count and kinds. A command
//! is built here only if `Build::of` names it; any other is reported as not supported yet, before
//! its arguments are read. The commands here: fractions and binomials, `\sqrt`, the style
//! commands, and `\text` with its font variants.

use super::{MathError, Mode, Node, NodeKind, Parser, TokenKind, unsupported, written};
use crate::math::{
    ast::StyleName,
    functions::{self, Function},
    lexer::Token,
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
        let args = self.parse_arguments(&name, &function)?;
        self.build(build, &name, token, args).map(Some)
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
        }
    }
}

/// A function's handler found fewer arguments than the table says it takes.
fn missing(name: &str) -> MathError {
    MathError::new(format!("Missing arguments for {name}"), 0..0)
}
