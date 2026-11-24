//! The functions the parser builds nodes for: how a command reads its arguments, and what node it
//! makes from them.
//!
//! `functions_data.rs` lists every command KaTeX has, with its argument count and kinds. A command
//! is built here only if `Build::of` names it; any other is reported as not supported yet, before
//! its arguments are read. The commands here: fractions and binomials, `\sqrt`, the style
//! commands, and `\text` with its font variants.

use std::ops::Range;

use super::{
    MathError, Mode, Name, Node, NodeKind, Parser, TokenKind, arguments, environments, unsupported,
    written,
};
use crate::math::{
    ast::{MathClass, Measurement, OpContent, StyleName},
    expander::token_text,
    functions::{self, Function},
    lexer::Token,
    symbols::AtomGroup,
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
    /// `\genfrac{left}{right}{bar}{style}{numer}{denom}`.
    Genfrac,
    /// `\over`, `\atop` and the like: they split the expression, and become the fraction
    /// command named (`\frac`) around the two halves.
    Infix(&'static str),
    /// `$` and `\(`: math inside text, to the closing `$` or `\)`.
    MathShift,
    /// `\)` and `\]` with nothing open.
    Mismatched,
    /// `\@char{65}`.
    Char,
    /// `\html@mathml{html}{mathml}`.
    HtmlMathml,
    /// `\href`, `\url`, `\includegraphics`, `\htmlClass` and the other HTML commands, which
    /// KaTeX leaves out unless the formula is trusted: sirene never trusts one. The command's
    /// arguments are read and checked, and its name stands in its place in the error color.
    Untrusted(&'static str),
    /// `\fbox`, `\colorbox`, `\fcolorbox`, `\cancel` and the rest that draw around a body.
    Enclose,
    /// `\xrightarrow` and the other stretchy arrows with a label.
    XArrow,
    /// `\overbrace`, `\underbrace`, `\overbracket`, `\underbracket`.
    HorizBrace,
    /// `\overset`, `\underset` and `\stackrel`: one body on another.
    Stack,
    Pmb,
    HBox,
    VCenter,
    MathChoice,
    /// `\reflectbox` and `\mathreflectbox`.
    ReflectBox,
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
    /// `\kern`, `\mkern`, `\hskip`, `\mskip`.
    Kern,
    Rule,
    /// `\textcolor{c}{x}`.
    TextColor,
    /// `\color{c}`: the rest of the group.
    Color,
    Phantom,
    VPhantom,
    Smash,
    /// `\mathllap` and the others, with the alignment.
    Lap(&'static str),
    RaiseBox,
    /// A size command, with its number (1 to 11).
    Sizing(u8),
    /// `\relax`.
    Relax,
    /// `\begin{name}`: the whole environment.
    Begin,
    /// `\end{name}`, read by the `\begin` it closes.
    End,
    /// `\\` outside a table.
    Cr,
    /// `\hline` and `\hdashline`, which only a table may use.
    Hline,
    /// `\sum`, `\int`, `\lim`, `\sin`: a named operator.
    Op {
        limits: bool,
        symbol: bool,
    },
    /// `\mathop{...}`.
    MathOp,
    /// `\operatorname@` and `\operatornamewithlimits`.
    OperatorName {
        with_limits: bool,
    },
    /// `\hat`, `\widehat`, `\overrightarrow`, and the other accents over a base.
    Accent,
    /// `\'`, `\"` and the other text accents.
    TextAccent,
    /// `\underrightarrow` and the other accents under a base.
    AccentUnder,
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
            "\\\\atopfrac" => frac(false, false, None, None, None),
            "\\\\bracefrac" => frac(false, false, Some("\\{"), Some("\\}"), None),
            "\\\\brackfrac" => frac(false, false, Some("["), Some("]"), None),
            "\\genfrac" => Build::Genfrac,
            "\\over" => Build::Infix("\\frac"),
            "\\choose" => Build::Infix("\\binom"),
            "\\atop" => Build::Infix("\\\\atopfrac"),
            "\\brace" => Build::Infix("\\\\bracefrac"),
            "\\brack" => Build::Infix("\\\\brackfrac"),
            "\\above" => Build::Infix("\\\\abovefrac"),
            "$" | "\\(" => Build::MathShift,
            "\\)" | "\\]" => Build::Mismatched,
            "\\@char" => Build::Char,
            "\\html@mathml" => Build::HtmlMathml,
            "\\href" => Build::Untrusted("\\href"),
            "\\url" => Build::Untrusted("\\url"),
            "\\includegraphics" => Build::Untrusted("\\includegraphics"),
            "\\htmlClass" => Build::Untrusted("\\htmlClass"),
            "\\htmlId" => Build::Untrusted("\\htmlId"),
            "\\htmlStyle" => Build::Untrusted("\\htmlStyle"),
            "\\htmlData" => Build::Untrusted("\\htmlData"),
            "\\fbox" | "\\colorbox" | "\\fcolorbox" | "\\cancel" | "\\bcancel" | "\\xcancel"
            | "\\sout" | "\\angl" | "\\phase" => Build::Enclose,
            "\\xleftarrow"
            | "\\xrightarrow"
            | "\\xLeftarrow"
            | "\\xRightarrow"
            | "\\xleftrightarrow"
            | "\\xLeftrightarrow"
            | "\\xhookleftarrow"
            | "\\xhookrightarrow"
            | "\\xmapsto"
            | "\\xrightharpoondown"
            | "\\xrightharpoonup"
            | "\\xleftharpoondown"
            | "\\xleftharpoonup"
            | "\\xrightleftharpoons"
            | "\\xleftrightharpoons"
            | "\\xlongequal"
            | "\\xtwoheadrightarrow"
            | "\\xtwoheadleftarrow"
            | "\\xtofrom"
            | "\\xrightleftarrows"
            | "\\xrightequilibrium"
            | "\\xleftequilibrium" => Build::XArrow,
            "\\overbrace" | "\\underbrace" | "\\overbracket" | "\\underbracket" => {
                Build::HorizBrace
            }
            "\\overset" | "\\underset" | "\\stackrel" => Build::Stack,
            "\\pmb" => Build::Pmb,
            "\\hbox" => Build::HBox,
            "\\vcenter" => Build::VCenter,
            "\\mathchoice" => Build::MathChoice,
            "\\reflectbox" | "\\mathreflectbox" => Build::ReflectBox,
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
            "\\acute"
            | "\\grave"
            | "\\ddot"
            | "\\tilde"
            | "\\bar"
            | "\\breve"
            | "\\check"
            | "\\hat"
            | "\\vec"
            | "\\dot"
            | "\\mathring"
            | "\\widecheck"
            | "\\widehat"
            | "\\widetilde"
            | "\\overrightarrow"
            | "\\overleftarrow"
            | "\\Overrightarrow"
            | "\\overleftrightarrow"
            | "\\overgroup"
            | "\\overlinesegment"
            | "\\overleftharpoon"
            | "\\overrightharpoon" => Build::Accent,
            "\\'" | "\\`" | "\\^" | "\\~" | "\\=" | "\\u" | "\\." | "\\\"" | "\\c" | "\\r"
            | "\\H" | "\\v" | "\\textcircled" => Build::TextAccent,
            "\\underleftarrow"
            | "\\underrightarrow"
            | "\\underleftrightarrow"
            | "\\undergroup"
            | "\\underlinesegment"
            | "\\utilde" => Build::AccentUnder,
            "\\coprod" | "\\bigvee" | "\\bigwedge" | "\\biguplus" | "\\bigcap" | "\\bigcup"
            | "\\intop" | "\\prod" | "\\sum" | "\\bigotimes" | "\\bigoplus" | "\\bigodot"
            | "\\bigsqcup" | "\\smallint" | "\u{220f}" | "\u{2210}" | "\u{2211}" | "\u{22c0}"
            | "\u{22c1}" | "\u{22c2}" | "\u{22c3}" | "\u{2a00}" | "\u{2a01}" | "\u{2a02}"
            | "\u{2a04}" | "\u{2a06}" => Build::Op {
                limits: true,
                symbol: true,
            },
            "\\arcsin" | "\\arccos" | "\\arctan" | "\\arctg" | "\\arcctg" | "\\arg" | "\\ch"
            | "\\cos" | "\\cosec" | "\\cosh" | "\\cot" | "\\cotg" | "\\coth" | "\\csc"
            | "\\ctg" | "\\cth" | "\\deg" | "\\dim" | "\\exp" | "\\hom" | "\\ker" | "\\lg"
            | "\\ln" | "\\log" | "\\sec" | "\\sin" | "\\sinh" | "\\sh" | "\\tan" | "\\tanh"
            | "\\tg" | "\\th" => Build::Op {
                limits: false,
                symbol: false,
            },
            "\\det" | "\\gcd" | "\\inf" | "\\lim" | "\\max" | "\\min" | "\\Pr" | "\\sup" => {
                Build::Op {
                    limits: true,
                    symbol: false,
                }
            }
            "\\int" | "\\iint" | "\\iiint" | "\\oint" | "\\oiint" | "\\oiiint" | "\u{222b}"
            | "\u{222c}" | "\u{222d}" | "\u{222e}" | "\u{222f}" | "\u{2230}" => Build::Op {
                limits: false,
                symbol: true,
            },
            "\\kern" | "\\mkern" | "\\hskip" | "\\mskip" => Build::Kern,
            "\\rule" => Build::Rule,
            "\\textcolor" => Build::TextColor,
            "\\color" => Build::Color,
            "\\phantom" => Build::Phantom,
            "\\vphantom" => Build::VPhantom,
            "\\smash" => Build::Smash,
            "\\mathllap" => Build::Lap("llap"),
            "\\mathrlap" => Build::Lap("rlap"),
            "\\mathclap" => Build::Lap("clap"),
            "\\raisebox" => Build::RaiseBox,
            "\\tiny" => Build::Sizing(1),
            "\\sixptsize" => Build::Sizing(2),
            "\\scriptsize" => Build::Sizing(3),
            "\\footnotesize" => Build::Sizing(4),
            "\\small" => Build::Sizing(5),
            "\\normalsize" => Build::Sizing(6),
            "\\large" => Build::Sizing(7),
            "\\Large" => Build::Sizing(8),
            "\\LARGE" => Build::Sizing(9),
            "\\huge" => Build::Sizing(10),
            "\\Huge" => Build::Sizing(11),
            "\\relax" => Build::Relax,
            "\\begin" => Build::Begin,
            "\\end" => Build::End,
            "\\\\" => Build::Cr,
            "\\hline" | "\\hdashline" => Build::Hline,
            "\\mathop" => Build::MathOp,
            "\\operatorname@" => Build::OperatorName { with_limits: false },
            "\\operatornamewithlimits" => Build::OperatorName { with_limits: true },
            "\\left" => Build::Left,
            "\\middle" => Build::Middle,
            _ => return None,
        })
    }
}

/// The arguments of a function: the required ones in order, and the optional ones, each of which
/// may be absent.
pub(super) struct Arguments<'a> {
    pub(super) required: Vec<Node<'a>>,
    pub(super) optional: Vec<Option<Node<'a>>>,
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
        self.enter(&token.span)?;
        let built = self
            .parse_arguments(&name, &function)
            .and_then(|args| self.dispatch(build, &name, token, args));
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
            b's' => self.parse_size_group(optional),
            b'c' => self.parse_color_group(optional),
            // The string of the tokens, with `%` a character in it. A raw argument skips the
            // spaces and comments before it first.
            b'u' | b'r' => {
                if kind == b'r' {
                    self.expander.consume_spaces()?;
                }
                let before = self.expander.set_plain_percent(true);
                let found = self.parse_string_group(optional);
                self.expander.set_plain_percent(before);
                let Some((text, span)) = found? else {
                    return Ok(None);
                };
                let (kind, mode) = if kind == b'u' {
                    (NodeKind::Url { url: text }, self.mode)
                } else {
                    (NodeKind::Raw { string: text }, Mode::Text)
                };
                Ok(Some(Node { kind, mode, span }))
            }
            b'h' => {
                // A box argument is text, set in the text style with the font reset.
                let Some(group) = self.parse_argument_group(optional, Some(Mode::Text))? else {
                    return Ok(None);
                };
                let (mode, span) = (group.mode, group.span.clone());
                Ok(Some(Node {
                    kind: NodeKind::Styling {
                        style: StyleName::Text,
                        body: vec![group],
                        reset_font: true,
                    },
                    mode,
                    span,
                }))
            }
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
        let outer_color = self.current_color.clone();
        let body = self.parse_expression(false)?;
        // The argument ends at the `Eof` that `scan_argument` put after it.
        self.expect("EOF")?;
        self.expander.end_group(&arg.end)?;
        self.current_color = outer_color;
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

    fn dispatch(
        &mut self,
        build: Build,
        name: &str,
        token: &Token<'a>,
        args: Arguments<'a>,
    ) -> Result<Node<'a>, MathError> {
        match build {
            Build::Styling(_)
            | Build::OldFont(_)
            | Build::Color
            | Build::Sizing(_)
            | Build::Left
            | Build::MathShift
            | Build::Begin => self.build_open(build, name, token, args),
            _ => self.build(build, name, token, args),
        }
    }

    /// The functions that read the rest of the group, and so recurse: kept in a function of
    /// their own so the frames of that recursion stay small.
    fn build_open(
        &mut self,
        build: Build,
        name: &str,
        token: &Token<'a>,
        mut args: Arguments<'a>,
    ) -> Result<Node<'a>, MathError> {
        let start = token.span.start;
        let mode = self.mode;
        match build {
            Build::Styling(style) => {
                // The body is the rest of the group; an infix command ends it.
                let body = self.parse_expression_until(true, self.break_on)?;
                let end = body.last().map_or(token.span.end, |n| n.span.end);
                Ok(Node {
                    kind: NodeKind::Styling {
                        style,
                        body,
                        reset_font: false,
                    },
                    mode,
                    span: start..end,
                })
            }
            Build::OldFont(font) => {
                // The rest of the group is set in the font.
                let body = self.parse_expression_until(true, self.break_on)?;
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
            Build::Color => {
                let Some(color) = args.required.pop() else {
                    return Err(missing(name));
                };
                let color = color_token(color, name)?;
                // A `\right` after this reads the color; it lasts to the end of the group.
                self.current_color = Some(color.clone());
                // The rest of the group is in the color; an infix command ends it.
                let body = self.parse_expression_until(true, self.break_on)?;
                let end = body.last().map_or(token.span.end, |n| n.span.end);
                Ok(Node {
                    kind: NodeKind::Color { color, body },
                    mode,
                    span: start..end,
                })
            }
            Build::Sizing(size) => {
                let body = self.parse_expression_until(false, self.break_on)?;
                let end = body.last().map_or(token.span.end, |n| n.span.end);
                Ok(Node {
                    kind: NodeKind::Sizing { size, body },
                    mode,
                    span: start..end,
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
                    kind: NodeKind::LeftRight {
                        body,
                        left,
                        right,
                        right_color: self.current_color.clone(),
                    },
                    mode,
                    span: start..right_arg.span.end,
                })
            }
            Build::MathShift => {
                let close = if name == "\\(" { "\\)" } else { "$" };
                self.switch_mode(Mode::Math);
                let body = self.parse_expression_until(false, Some(close))?;
                let end = self.expect(close)?;
                self.switch_mode(mode);
                Ok(Node {
                    kind: NodeKind::Styling {
                        style: StyleName::Text,
                        body,
                        reset_font: true,
                    },
                    mode,
                    span: start..end.span.end,
                })
            }
            Build::Begin => {
                let Some(name_group) = args.required.pop() else {
                    return Err(missing(name));
                };
                let message = "Environment name should contain only text characters and spaces";
                let env_name = environments::character_group(&name_group, message, true)?;
                let name_span = name_group.span.clone();
                let Some(count) = environments::environment_args(&env_name) else {
                    return Err(MathError::new(
                        format!("No such environment: {env_name}"),
                        name_span,
                    ));
                };
                let function = environments::environment_function(count);
                let env_args =
                    self.parse_arguments(&format!("\\begin{{{env_name}}}"), &function)?;
                let mut result = self.build_environment(&env_name, &name_span, env_args)?;
                result.span.start = start;
                // The table stops at its `\end`, which is read here, as `\left` reads `\right`.
                let close = self.fetch()?;
                if token_text(&close.kind) != "\\end" {
                    return Err(MathError::new(
                        format!("Expected '\\end', got '{}'", token_text(&close.kind)),
                        close.span,
                    ));
                }
                let ended = self.parse_function(&close, "atom")?;
                let Some(Node {
                    kind: NodeKind::Environment { name: end_name },
                    span: end_span,
                    ..
                }) = ended
                else {
                    return Err(missing("\\end"));
                };
                if end_name != env_name {
                    return Err(MathError::new(
                        format!("Mismatch: \\begin{{{env_name}}} matched by \\end{{{end_name}}}"),
                        close.span,
                    ));
                }
                set_end(&mut result, end_span.end);
                Ok(result)
            }
            _ => Err(missing(name)),
        }
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
            Build::Frac { .. } => {
                let (Some(denom), Some(numer)) = (args.required.pop(), args.required.pop()) else {
                    return Err(missing(name));
                };
                frac_node(build, numer, denom, start, mode, name)
            }
            Build::Genfrac => {
                let (Some(denom), Some(numer), Some(style), Some(bar), Some(right), Some(left)) = (
                    args.required.pop(),
                    args.required.pop(),
                    args.required.pop(),
                    args.required.pop(),
                    args.required.pop(),
                    args.required.pop(),
                ) else {
                    return Err(missing(name));
                };
                // The delimiters count only if they are open and close atoms.
                let delimiter = |node: Node<'a>, group: AtomGroup| match node.normalized().kind {
                    NodeKind::Symbol {
                        group: found,
                        name: symbol,
                    } if found == group => Some(match symbol {
                        Name::Char(text) => text.to_string(),
                        Name::Control(control) => format!("\\{control}"),
                    }),
                    _ => None,
                };
                let left = delimiter(left, AtomGroup::Open);
                let right = delimiter(right, AtomGroup::Close);
                let NodeKind::Size { value, is_blank } = bar.kind else {
                    return Err(expected_type("size", &bar));
                };
                // An empty size asks for the usual bar; a size of zero for none.
                let (has_bar, bar_size) = if is_blank {
                    (true, None)
                } else {
                    (value.number > 0.0, Some(value))
                };
                // The style is a digit, 0 (display) to 3 (script script); anything else leaves
                // the fraction in the style it stands in.
                let style = match style.kind {
                    NodeKind::Group { body, .. } => match body.first() {
                        None => None,
                        Some(Node {
                            kind:
                                NodeKind::Symbol {
                                    group: AtomGroup::TextOrd,
                                    name: Name::Char(digit),
                                },
                            ..
                        }) => match *digit {
                            "0" => Some(StyleName::Display),
                            "1" => Some(StyleName::Text),
                            "2" => Some(StyleName::Script),
                            "3" => Some(StyleName::ScriptScript),
                            _ => None,
                        },
                        Some(other) => return Err(expected_type("textord", other)),
                    },
                    _ => None,
                };
                let span = start..denom.span.end;
                Ok(wrap_style(
                    Node {
                        kind: NodeKind::Frac {
                            numer: Box::new(numer),
                            denom: Box::new(denom),
                            continued: false,
                            bar: has_bar,
                            bar_size,
                            left,
                            right,
                        },
                        mode,
                        span,
                    },
                    style,
                ))
            }
            Build::Enclose => {
                let Some(body) = args.required.pop() else {
                    return Err(missing(name));
                };
                // The colors come first, so they are popped last to first:
                // `\fcolorbox{border}{background}{body}`.
                let (border_color, background_color) = match name {
                    "\\colorbox" => (None, Some(pop_color(&mut args, name)?)),
                    "\\fcolorbox" => {
                        let background = pop_color(&mut args, name)?;
                        let border = pop_color(&mut args, name)?;
                        (Some(border), Some(background))
                    }
                    _ => (None, None),
                };
                Ok(Node {
                    span: start..body.span.end,
                    kind: NodeKind::Enclose {
                        label: name.to_string(),
                        background_color,
                        border_color,
                        body: Box::new(body),
                    },
                    mode,
                })
            }
            Build::Mismatched => Err(MathError::new(
                format!("Mismatched {name}"),
                token.span.clone(),
            )),
            Build::Char => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let message = "\\@char has non-numeric argument";
                let number = environments::character_group(&arg, message, false)?;
                let Some(code) = js_parse_int(&number) else {
                    return Err(MathError::new(format!("{message} {number}"), arg.span));
                };
                let Some(c) = u32::try_from(code).ok().and_then(char::from_u32) else {
                    return Err(MathError::new(
                        format!("\\@char with invalid code point {number}"),
                        arg.span,
                    ));
                };
                Ok(Node {
                    kind: NodeKind::CharCode(c),
                    mode,
                    span: start..arg.span.end,
                })
            }
            Build::HtmlMathml => {
                let (Some(mathml), Some(html)) = (args.required.pop(), args.required.pop()) else {
                    return Err(missing(name));
                };
                let span = start..mathml.span.end;
                Ok(Node {
                    kind: NodeKind::HtmlMathml {
                        html: html.into_body(),
                        mathml: mathml.into_body(),
                    },
                    mode,
                    span,
                })
            }
            Build::Untrusted(command) => {
                // What KaTeX checks before it asks whether the command is trusted.
                match (command, args.required.first(), args.optional.first()) {
                    ("\\htmlData", Some(raw), _) => check_html_data(raw)?,
                    ("\\includegraphics", _, Some(Some(raw))) => check_graphics(raw)?,
                    _ => {}
                }
                Ok(left_out(command, mode, token.span.clone()))
            }
            Build::XArrow => {
                let Some(body) = args.required.pop() else {
                    return Err(missing(name));
                };
                let below = args.optional.pop().flatten();
                Ok(Node {
                    span: start..body.span.end,
                    kind: NodeKind::XArrow {
                        label: name.to_string(),
                        body: Box::new(body),
                        below: below.map(Box::new),
                    },
                    mode,
                })
            }
            Build::HorizBrace => {
                let Some(base) = args.required.pop() else {
                    return Err(missing(name));
                };
                Ok(Node {
                    span: start..base.span.end,
                    kind: NodeKind::HorizBrace {
                        label: name.to_string(),
                        is_over: name.contains("\\over"),
                        base: Box::new(base),
                    },
                    mode,
                })
            }
            Build::Stack => {
                let (Some(base), Some(shifted)) = (args.required.pop(), args.required.pop()) else {
                    return Err(missing(name));
                };
                // `\stackrel` is a relation; the others space as their base would.
                let class = if name == "\\stackrel" {
                    MathClass::Rel
                } else {
                    base.binrel_class()
                };
                let (base_mode, shifted_mode) = (base.mode, shifted.mode);
                let span = start..base.span.end;
                let op = Node {
                    kind: NodeKind::Op {
                        limits: true,
                        always_handle_supsub: true,
                        symbol: false,
                        suppress_base_shift: Some(name != "\\stackrel"),
                        content: OpContent::Body(base.into_body()),
                    },
                    mode: base_mode,
                    span: span.clone(),
                };
                let (sup, sub) = if name == "\\underset" {
                    (None, Some(Box::new(shifted)))
                } else {
                    (Some(Box::new(shifted)), None)
                };
                let supsub = Node {
                    kind: NodeKind::SupSub {
                        base: Some(Box::new(op)),
                        sup,
                        sub,
                    },
                    mode: shifted_mode,
                    span: span.clone(),
                };
                let character_box = supsub.is_character_box();
                Ok(Node {
                    kind: NodeKind::MClass {
                        class,
                        body: vec![supsub],
                        character_box,
                    },
                    mode,
                    span,
                })
            }
            Build::Pmb => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let (class, span) = (arg.binrel_class(), start..arg.span.end);
                Ok(Node {
                    kind: NodeKind::Pmb {
                        class,
                        body: arg.into_body(),
                    },
                    mode,
                    span,
                })
            }
            Build::HBox => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let span = start..arg.span.end;
                Ok(Node {
                    kind: NodeKind::HBox {
                        body: arg.into_body(),
                    },
                    mode,
                    span,
                })
            }
            Build::VCenter => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let span = start..arg.span.end;
                Ok(Node {
                    kind: NodeKind::VCenter {
                        body: Box::new(arg),
                    },
                    mode,
                    span,
                })
            }
            Build::MathChoice => {
                let (Some(script_script), Some(script), Some(text), Some(display)) = (
                    args.required.pop(),
                    args.required.pop(),
                    args.required.pop(),
                    args.required.pop(),
                ) else {
                    return Err(missing(name));
                };
                let span = start..script_script.span.end;
                Ok(Node {
                    kind: NodeKind::MathChoice {
                        display: display.into_body(),
                        text: text.into_body(),
                        script: script.into_body(),
                        script_script: script_script.into_body(),
                    },
                    mode,
                    span,
                })
            }
            Build::ReflectBox => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let span = start..arg.span.end;
                Ok(Node {
                    kind: NodeKind::ReflectBox {
                        body: Box::new(arg),
                    },
                    mode,
                    span,
                })
            }
            Build::Infix(replace_with) => {
                let size = match args.required.pop() {
                    Some(arg) => Some(measurement(arg, name)?),
                    None => None,
                };
                Ok(Node {
                    kind: NodeKind::Infix { replace_with, size },
                    mode,
                    span: token.span.clone(),
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
                    kind: NodeKind::Text {
                        body,
                        font: Some(font),
                    },
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
            Build::Accent => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let span = start..arg.span.end;
                // These stretch to the base, except the ones that are one fixed glyph.
                let fixed = matches!(
                    name,
                    "\\acute"
                        | "\\grave"
                        | "\\ddot"
                        | "\\tilde"
                        | "\\bar"
                        | "\\breve"
                        | "\\check"
                        | "\\hat"
                        | "\\vec"
                        | "\\dot"
                        | "\\mathring"
                );
                let shifty =
                    !(!fixed) || matches!(name, "\\widehat" | "\\widetilde" | "\\widecheck");
                Ok(Node {
                    kind: NodeKind::Accent {
                        label: name.to_string(),
                        stretchy: !fixed,
                        shifty,
                        base: Box::new(arg.normalized()),
                    },
                    mode,
                    span,
                })
            }
            Build::TextAccent => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let span = start..arg.span.end;
                // A text accent is text even where it stands in math.
                Ok(Node {
                    kind: NodeKind::Accent {
                        label: name.to_string(),
                        stretchy: false,
                        shifty: true,
                        base: Box::new(arg),
                    },
                    mode: Mode::Text,
                    span,
                })
            }
            Build::AccentUnder => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let span = start..arg.span.end;
                Ok(Node {
                    kind: NodeKind::AccentUnder {
                        label: name.to_string(),
                        base: Box::new(arg),
                    },
                    mode,
                    span,
                })
            }
            Build::Kern => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let span = start..arg.span.end;
                Ok(Node {
                    kind: NodeKind::Kern {
                        dimension: measurement(arg, name)?,
                    },
                    mode,
                    span,
                })
            }
            Build::Rule => {
                let (Some(height), Some(width)) = (args.required.pop(), args.required.pop()) else {
                    return Err(missing(name));
                };
                let span = start..height.span.end;
                let shift = match args.optional.pop().flatten() {
                    Some(shift) => Some(measurement(shift, name)?),
                    None => None,
                };
                Ok(Node {
                    kind: NodeKind::Rule {
                        shift,
                        width: measurement(width, name)?,
                        height: measurement(height, name)?,
                    },
                    mode,
                    span,
                })
            }
            Build::TextColor => {
                let (Some(body), Some(color)) = (args.required.pop(), args.required.pop()) else {
                    return Err(missing(name));
                };
                let span = start..body.span.end;
                Ok(Node {
                    kind: NodeKind::Color {
                        color: color_token(color, name)?,
                        body: body.into_body(),
                    },
                    mode,
                    span,
                })
            }
            Build::Phantom => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let span = start..arg.span.end;
                Ok(Node {
                    kind: NodeKind::Phantom {
                        body: arg.into_body(),
                    },
                    mode,
                    span,
                })
            }
            Build::VPhantom => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let span = start..arg.span.end;
                Ok(Node {
                    kind: NodeKind::VPhantom {
                        body: Box::new(arg),
                    },
                    mode,
                    span,
                })
            }
            Build::Smash => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let span = start..arg.span.end;
                // `[tb]` picks the side: `t` the height, `b` the depth, and any other letter
                // none. Without it both are smashed.
                let (mut height, mut depth) = (true, true);
                if let Some(Some(letters)) = args.optional.pop() {
                    (height, depth) = (false, false);
                    for letter in letters.into_body() {
                        let NodeKind::Symbol { name: symbol, .. } = &letter.kind else {
                            return Err(MathError::new(
                                format!(
                                    "Expected node of symbol group type, but got node of type {}",
                                    letter.kind.type_name()
                                ),
                                letter.span,
                            ));
                        };
                        match symbol {
                            Name::Char("t") => height = true,
                            Name::Char("b") => depth = true,
                            _ => {
                                (height, depth) = (false, false);
                                break;
                            }
                        }
                    }
                }
                Ok(Node {
                    kind: NodeKind::Smash {
                        body: Box::new(arg),
                        height,
                        depth,
                    },
                    mode,
                    span,
                })
            }
            Build::Lap(alignment) => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let span = start..arg.span.end;
                Ok(Node {
                    kind: NodeKind::Lap {
                        alignment,
                        body: Box::new(arg),
                    },
                    mode,
                    span,
                })
            }
            Build::RaiseBox => {
                let (Some(body), Some(amount)) = (args.required.pop(), args.required.pop()) else {
                    return Err(missing(name));
                };
                let span = start..body.span.end;
                Ok(Node {
                    kind: NodeKind::RaiseBox {
                        dy: measurement(amount, name)?,
                        body: Box::new(body),
                    },
                    mode,
                    span,
                })
            }
            Build::Relax => Ok(Node {
                kind: NodeKind::Internal,
                mode,
                span: token.span.clone(),
            }),
            Build::Op { limits, symbol } => Ok(Node {
                kind: NodeKind::Op {
                    limits,
                    always_handle_supsub: false,
                    symbol,
                    suppress_base_shift: None,
                    content: OpContent::Name(canonical_operator(name)),
                },
                mode,
                span: token.span.clone(),
            }),
            Build::MathOp => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let span = start..arg.span.end;
                Ok(Node {
                    kind: NodeKind::Op {
                        limits: false,
                        always_handle_supsub: false,
                        symbol: false,
                        suppress_base_shift: None,
                        content: OpContent::Body(arg.into_body()),
                    },
                    mode,
                    span,
                })
            }
            Build::OperatorName { with_limits } => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let span = start..arg.span.end;
                Ok(Node {
                    kind: NodeKind::OperatorName {
                        body: arg.into_body(),
                        always_handle_supsub: with_limits,
                        limits: false,
                    },
                    mode,
                    span,
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
            // Read by `build_open`.
            Build::Styling(_)
            | Build::OldFont(_)
            | Build::Color
            | Build::Sizing(_)
            | Build::Left
            | Build::MathShift
            | Build::Begin => Err(missing(name)),
            Build::End => {
                let Some(arg) = args.required.pop() else {
                    return Err(missing(name));
                };
                let message = "Environment name should contain only text characters and spaces";
                let env = environments::character_group(&arg, message, true)?;
                Ok(Node {
                    kind: NodeKind::Environment { name: env },
                    mode,
                    span: start..arg.span.end,
                })
            }
            Build::Cr => {
                let size = if token_text(&self.expander.future()?.kind) == "[" {
                    self.parse_size_group(true)?
                } else {
                    None
                };
                let end = size.as_ref().map_or(token.span.end, |n| n.span.end);
                Ok(Node {
                    kind: NodeKind::Cr {
                        // Display mode makes no difference in KaTeX's default strict setting.
                        new_line: true,
                        size: environments::size_value(size),
                    },
                    mode,
                    span: start..end,
                })
            }
            Build::Hline => Err(MathError::new(
                format!("{name} valid only within array environment"),
                token.span.clone(),
            )),
        }
    }
}

/// Sets where a node, and the table inside it, end.
fn set_end(node: &mut Node<'_>, end: usize) {
    node.span.end = end;
    if let NodeKind::LeftRight { body, .. } = &mut node.kind
        && let Some(inner) = body.first_mut()
    {
        inner.span.end = end;
    }
}

/// The measurement a size argument holds.
fn measurement(node: Node<'_>, name: &str) -> Result<Measurement, MathError> {
    match node.kind {
        NodeKind::Size { value, .. } => Ok(value),
        _ => Err(missing(name)),
    }
}

/// The color a color argument holds.
fn color_token(node: Node<'_>, name: &str) -> Result<String, MathError> {
    match node.kind {
        NodeKind::ColorToken(color) => Ok(color),
        _ => Err(missing(name)),
    }
}

/// The color KaTeX gives a command it leaves out (its `errorColor` setting).
const ERROR_COLOR: &str = "#cc0000";

/// What stands for a command that is not run: its name as text, in the error color.
fn left_out<'a>(command: &'static str, mode: Mode, span: Range<usize>) -> Node<'a> {
    let letter = |text| Node {
        kind: NodeKind::Symbol {
            group: AtomGroup::TextOrd,
            name: Name::Char(text),
        },
        mode: Mode::Text,
        span: span.clone(),
    };
    let body = command
        .char_indices()
        .filter_map(|(i, c)| command.get(i..i + c.len_utf8()))
        .map(letter)
        .collect();
    let text = Node {
        kind: NodeKind::Text { body, font: None },
        mode,
        span: span.clone(),
    };
    Node {
        kind: NodeKind::Color {
            color: ERROR_COLOR.to_string(),
            body: vec![text],
        },
        mode,
        span,
    }
}

/// The checks `\htmlData` makes of its attribute list: `key=value` items split at commas, where
/// `{,}` is a comma inside an item.
fn check_html_data(raw: &Node<'_>) -> Result<(), MathError> {
    let NodeKind::Raw { string } = &raw.kind else {
        return Err(expected_type("raw", raw));
    };
    let mut items = Vec::new();
    let mut current = String::new();
    let mut rest = string.as_str();
    loop {
        if let Some(after) = rest.strip_prefix("{,}") {
            current.push(',');
            rest = after;
        } else if let Some(after) = rest.strip_prefix(',') {
            items.push(std::mem::take(&mut current));
            rest = after;
        } else {
            let mut chars = rest.chars();
            let Some(c) = chars.next() else { break };
            current.push(c);
            rest = chars.as_str();
        }
    }
    items.push(current);
    for item in items {
        if !item.contains('=') {
            return Err(MathError::new(
                format!("\\htmlData key/value '{item}' missing equals sign"),
                raw.span.clone(),
            ));
        }
    }
    Ok(())
}

/// The checks `\includegraphics` makes of its `key=value` options before it looks at the URL.
fn check_graphics(raw: &Node<'_>) -> Result<(), MathError> {
    let NodeKind::Raw { string } = &raw.kind else {
        return Err(expected_type("raw", raw));
    };
    for attribute in string.split(',') {
        let parts: Vec<&str> = attribute.split('=').collect();
        if let [key, value] = parts.as_slice() {
            match key.trim() {
                "alt" => {}
                "width" | "height" | "totalheight" => {
                    arguments::check_graphics_size(value.trim(), &raw.span)?;
                }
                _ => {
                    return Err(MathError::new(
                        format!("Invalid key: '{key}' in \\includegraphics."),
                        raw.span.clone(),
                    ));
                }
            }
        }
    }
    Ok(())
}

/// What JavaScript's `parseInt` makes of `text`: a sign, then decimal digits (or hexadecimal ones
/// after `0x`), up to the first other character. `None` if there are no digits. A number too big
/// for the type stays at its largest, which is out of every range the caller allows.
fn js_parse_int(text: &str) -> Option<i64> {
    let text = text.trim_start();
    let (negative, rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let (radix, digits) = match rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        Some(digits) => (16, digits),
        None => (10, rest),
    };
    let mut value: i64 = 0;
    let mut any = false;
    for c in digits.chars() {
        let Some(digit) = c.to_digit(radix) else {
            break;
        };
        any = true;
        value = value
            .saturating_mul(i64::from(radix))
            .saturating_add(i64::from(digit));
    }
    any.then_some(if negative { -value } else { value })
}

/// The last required argument, a color.
fn pop_color(args: &mut Arguments<'_>, name: &str) -> Result<String, MathError> {
    match args.required.pop() {
        Some(arg) => color_token(arg, name),
        None => Err(missing(name)),
    }
}

/// The command an operator is known by: the Unicode operator characters stand for commands.
fn canonical_operator(name: &str) -> String {
    let command = match name {
        "\u{220f}" => "\\prod",
        "\u{2210}" => "\\coprod",
        "\u{2211}" => "\\sum",
        "\u{22c0}" => "\\bigwedge",
        "\u{22c1}" => "\\bigvee",
        "\u{22c2}" => "\\bigcap",
        "\u{22c3}" => "\\bigcup",
        "\u{2a00}" => "\\bigodot",
        "\u{2a01}" => "\\bigoplus",
        "\u{2a02}" => "\\bigotimes",
        "\u{2a04}" => "\\biguplus",
        "\u{2a06}" => "\\bigsqcup",
        "\u{222b}" => "\\int",
        "\u{222c}" => "\\iint",
        "\u{222d}" => "\\iiint",
        "\u{222e}" => "\\oint",
        "\u{222f}" => "\\oiint",
        "\u{2230}" => "\\oiiint",
        other => other,
    };
    command.to_string()
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

/// A fraction from a `Build::Frac` and its two halves, in the style the command forces.
fn frac_node<'a>(
    build: Build,
    numer: Node<'a>,
    denom: Node<'a>,
    start: usize,
    mode: Mode,
    name: &str,
) -> Result<Node<'a>, MathError> {
    let Build::Frac {
        bar,
        continued,
        left,
        right,
        style,
    } = build
    else {
        return Err(missing(name));
    };
    let span = start..denom.span.end;
    Ok(wrap_style(
        Node {
            kind: NodeKind::Frac {
                numer: Box::new(numer),
                denom: Box::new(denom),
                continued,
                bar,
                bar_size: None,
                left: left.map(String::from),
                right: right.map(String::from),
            },
            mode,
            span,
        },
        style,
    ))
}

/// `node` set in `style`, if there is one.
fn wrap_style(node: Node<'_>, style: Option<StyleName>) -> Node<'_> {
    match style {
        Some(style) => {
            let (mode, span) = (node.mode, node.span.clone());
            Node {
                kind: NodeKind::Styling {
                    style,
                    body: vec![node],
                    reset_font: false,
                },
                mode,
                span,
            }
        }
        None => node,
    }
}

/// The error for a node of another type than a function's handler needs.
fn expected_type(wanted: &str, node: &Node<'_>) -> MathError {
    MathError::new(
        format!(
            "Expected node of type {wanted}, but got node of type {}",
            node.kind.type_name()
        ),
        node.span.clone(),
    )
}

impl<'a> Parser<'a> {
    /// Rewrites the infix command in an expression (`a \over b`) as the fraction it stands for,
    /// with everything before it as the numerator and everything after as the denominator.
    /// One infix command to a group: with two the expression is ambiguous.
    pub(super) fn split_infix(&mut self, body: Vec<Node<'a>>) -> Result<Vec<Node<'a>>, MathError> {
        let mut found = None;
        for (i, node) in body.iter().enumerate() {
            if matches!(node.kind, NodeKind::Infix { .. }) {
                if found.is_some() {
                    return Err(MathError::new(
                        "only one infix operator per group",
                        node.span.clone(),
                    ));
                }
                found = Some(i);
            }
        }
        let Some(at) = found else {
            return Ok(body);
        };
        let mode = self.mode;
        let mut numer = body;
        let mut rest = numer.split_off(at).into_iter();
        let Some(Node {
            kind: NodeKind::Infix { replace_with, size },
            span: infix,
            ..
        }) = rest.next()
        else {
            return Err(missing("an infix command"));
        };
        // Each half is a group, or the one group it already is.
        let half = |mut nodes: Vec<Node<'a>>, at: usize| {
            if nodes.len() == 1
                && nodes
                    .first()
                    .is_some_and(|n| matches!(n.kind, NodeKind::Group { .. }))
                && let Some(only) = nodes.pop()
            {
                return only;
            }
            let span = nodes
                .first()
                .zip(nodes.last())
                .map_or(at..at, |(first, last)| first.span.start..last.span.end);
            Node {
                kind: NodeKind::Group {
                    body: nodes,
                    semisimple: false,
                },
                mode,
                span,
            }
        };
        let numer = half(numer, infix.start);
        let denom = half(rest.collect(), infix.end);
        let start = numer.span.start.min(infix.start);
        let frac = if replace_with == "\\\\abovefrac" {
            let Some(size) = size else {
                return Err(missing("\\above"));
            };
            let span = start..denom.span.end;
            Node {
                kind: NodeKind::Frac {
                    numer: Box::new(numer),
                    denom: Box::new(denom),
                    continued: false,
                    bar: size.number > 0.0,
                    bar_size: Some(size),
                    left: None,
                    right: None,
                },
                mode,
                span,
            }
        } else {
            match Build::of(replace_with) {
                Some(build) => frac_node(build, numer, denom, start, mode, replace_with)?,
                None => return Err(missing(replace_with)),
            }
        };
        Ok(vec![frac])
    }
}

/// A function's handler found fewer arguments than the table says it takes.
fn missing(name: &str) -> MathError {
    MathError::new(format!("Missing arguments for {name}"), 0..0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{expander::Expander, functions_data::FUNCTIONS, parser::parse};

    fn parser(src: &str) -> Parser<'_> {
        Parser {
            src,
            expander: Expander::new(src, Mode::Math).unwrap(),
            next: None,
            mode: Mode::Math,
            depth: 0,
            leftright_depth: 0,
            break_on: None,
            display_mode: false,
            current_color: None,
        }
    }

    fn x() -> Node<'static> {
        Node {
            kind: NodeKind::Symbol {
                group: AtomGroup::MathOrd,
                name: Name::Char("x"),
            },
            mode: Mode::Math,
            span: 0..1,
        }
    }

    fn none() -> Arguments<'static> {
        Arguments {
            required: Vec::new(),
            optional: Vec::new(),
        }
    }

    fn message<T>(result: Result<T, MathError>) -> String {
        match result {
            Ok(_) => "ok".to_string(),
            Err(e) => e.message,
        }
    }

    fn opens(build: Build) -> bool {
        matches!(
            build,
            Build::Styling(_)
                | Build::OldFont(_)
                | Build::Color
                | Build::Sizing(_)
                | Build::Left
                | Build::MathShift
                | Build::Begin
        )
    }

    /// A handler given fewer arguments than the table promises reports it, never panics: each
    /// function `Build::of` knows, called with none, by both builders.
    #[test]
    fn a_handler_without_its_arguments_is_an_error() {
        let mut wrong = String::new();
        let mut checked = 0;
        for row in FUNCTIONS {
            let name = row.0;
            let Some(build) = Build::of(name) else {
                continue;
            };
            let Some(function) = functions::lookup(name) else {
                continue;
            };
            checked += 1;
            let token = Token {
                kind: TokenKind::ControlWord(name.trim_start_matches('\\')),
                span: 0..name.len(),
            };
            let missing = format!("Missing arguments for {name}");
            // An untrusted command stands in as its name, and an infix one's size is optional.
            let needs = function.num_args > 0
                && !matches!(build, Build::Untrusted(_) | Build::Infix(_) | Build::Cr);
            let built = message(parser("").build(build, name, &token, none()));
            if (opens(build) || needs) && built != missing {
                wrong.push_str(&format!("build {name}: {built}\n"));
            }
            let opened = message(parser("").build_open(build, name, &token, none()));
            if (!opens(build) || function.num_args > 0) && opened != missing {
                wrong.push_str(&format!("build_open {name}: {opened}\n"));
            }
        }
        assert!(checked > 200, "{checked}");
        assert!(wrong.is_empty(), "\n{wrong}");
    }

    #[test]
    fn helpers_reject_nodes_of_the_wrong_kind() {
        assert_eq!(
            message(measurement(x(), "\\kern")),
            "Missing arguments for \\kern"
        );
        assert_eq!(
            message(color_token(x(), "\\color")),
            "Missing arguments for \\color"
        );
        assert_eq!(
            message(pop_color(&mut none(), "\\colorbox")),
            "Missing arguments for \\colorbox"
        );
        let wanted_raw = "Expected node of type raw, but got node of type mathord";
        assert_eq!(message(check_html_data(&x())), wanted_raw);
        assert_eq!(message(check_graphics(&x())), wanted_raw);
        assert_eq!(
            message(frac_node(Build::Relax, x(), x(), 0, Mode::Math, "\\frac")),
            "Missing arguments for \\frac"
        );
        let mut p = parser("x");
        assert_eq!(
            message(p.parse_argument("\\foo", b'p', true)),
            "A primitive argument cannot be optional"
        );
        assert_eq!(
            message(p.parse_argument("\\foo", b'z', false)),
            "\\foo is not supported yet"
        );
    }

    #[test]
    fn an_infix_command_with_no_fraction_is_an_error() {
        let infix = |replace_with| Node {
            kind: NodeKind::Infix {
                replace_with,
                size: None,
            },
            mode: Mode::Math,
            span: 1..2,
        };
        let mut p = parser("");
        assert_eq!(
            message(p.split_infix(vec![x(), infix("\\\\abovefrac"), x()])),
            "Missing arguments for \\above"
        );
        assert_eq!(
            message(p.split_infix(vec![infix("\\nosuch")])),
            "Missing arguments for \\nosuch"
        );
        assert_eq!(
            message(p.split_infix(vec![infix("\\frac"), infix("\\frac")])),
            "only one infix operator per group"
        );
        // Halves with nothing in them are empty groups at the command.
        let split = p.split_infix(vec![infix("\\frac")]).unwrap();
        let [
            Node {
                kind: NodeKind::Frac { numer, denom, .. },
                ..
            },
        ] = split.as_slice()
        else {
            panic!("{split:?}");
        };
        assert_eq!((numer.span.clone(), denom.span.clone()), (1..1, 2..2));
    }

    #[test]
    fn js_parse_int_reads_as_javascript_does() {
        for (text, want) in [
            ("65", Some(65)),
            ("  65", Some(65)),
            ("+7", Some(7)),
            ("-5", Some(-5)),
            ("0x1F", Some(31)),
            ("0X1f", Some(31)),
            ("12abc", Some(12)),
            ("0x", None),
            ("abc", None),
            ("", None),
            ("-", None),
            ("99999999999999999999999", Some(i64::MAX)),
        ] {
            assert_eq!(js_parse_int(text), want, "{text:?}");
        }
    }

    #[test]
    fn errors_of_the_source() {
        for (src, want) in [
            (
                r"\smash[\frac{a}{b}]{x}",
                "Expected node of symbol group type, but got node of type genfrac",
            ),
            (
                r"\includegraphics[width=1em,depth=2em]{x}",
                "Invalid key: 'depth' in \\includegraphics.",
            ),
        ] {
            assert_eq!(message(parse(src)), want, "{src}");
        }
        // `\@binrel{a}{b}`: `b` spaced as `a` would be.
        for (src, want) in [
            (r"\@binrel{=}{xy}", MathClass::Rel),
            (r"\@binrel{+}{x}", MathClass::Bin),
            (r"\@binrel{a}{x}", MathClass::Ord),
        ] {
            let nodes = parse(src).unwrap();
            let [
                Node {
                    kind:
                        NodeKind::MClass {
                            class,
                            body,
                            character_box,
                        },
                    span,
                    ..
                },
            ] = nodes.as_slice()
            else {
                panic!("{src}: {nodes:?}");
            };
            assert_eq!(*class, want, "{src}");
            assert_eq!(*character_box, body.len() == 1, "{src}");
            assert_eq!(span.clone(), 0..src.len(), "{src}");
        }
        // An option with no `=`, or more than one, is passed over.
        for src in [
            r"\includegraphics[alt]{x}",
            r"\includegraphics[a=b=c]{x}",
            r"\smash[tb]{x}",
            r"\smash[q]{x}",
            r"\htmlData{a{,}b=c}{x}",
        ] {
            assert_eq!(message(parse(src)), "ok", "{src}");
        }
    }
}
