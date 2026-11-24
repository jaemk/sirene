//! The macro expander (LMATH-2, step 2): TeX's "gullet".
//!
//! It sits between the lexer and the parser. The parser asks it for the next token that is not a
//! macro; it reads the lexer, expands macros until a plain token is left, and hands that over. It
//! also reads macro arguments for the parser's functions (`\frac{a}{b}` takes two groups).
//!
//! Macros come from three places, looked up in this order: definitions made in the formula
//! (`\newcommand`, `\renewcommand`, `\providecommand`, `\def`, `\gdef`), which follow TeX's group
//! scoping; the commands that read their own arguments (`\@ifnextchar`, `\TextOrMath`, ...),
//! which are code here; and KaTeX's predefined macros, a table of TeX source generated into
//! `macros_data.rs`. Expansion is bounded by `limits` (LMATH-6).

use std::{collections::HashMap, ops::Range, rc::Rc};

use super::{
    MathError, functions,
    lexer::{Lexer, Token, TokenKind},
    limits, macros_data,
    symbols::{self, AtomGroup, Mode, Name},
};

/// What a macro is called: a control word (`\foo`), a control symbol (`\,`), or a character that
/// is active, such as `~`. Borrowed from the source, so a lookup allocates nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Key<'a> {
    Word(&'a str),
    Symbol(char),
    Char(&'a str),
}

impl<'a> Key<'a> {
    /// The key a token would be looked up by, if it can name a macro.
    pub(crate) fn of(kind: &TokenKind<'a>) -> Option<Self> {
        match *kind {
            TokenKind::ControlWord(word) => Some(Key::Word(word)),
            TokenKind::ControlSymbol(c) => Some(Key::Symbol(c)),
            TokenKind::Char(text) => Some(Key::Char(text)),
            TokenKind::Space | TokenKind::Verb { .. } | TokenKind::Frozen(_) | TokenKind::Eof => {
                None
            }
        }
    }

    /// The name as KaTeX writes it, with the backslash of a control sequence, for messages.
    fn display(&self) -> String {
        match self {
            Key::Word(word) => format!("\\{word}"),
            Key::Symbol(c) => format!("\\{c}"),
            Key::Char(text) => (*text).to_string(),
        }
    }

    /// Calls `with` with the key's name as the symbol table and the macro tables spell it.
    fn with_name<T>(&self, with: impl FnOnce(Name<'_>) -> T) -> T {
        match self {
            Key::Word(word) => with(Name::Control(word)),
            Key::Symbol(c) => with(Name::Control(c.encode_utf8(&mut [0; 4]))),
            Key::Char(text) => with(Name::Char(text)),
        }
    }
}

/// A macro: replacement tokens in order, how many arguments it takes, and for a `\def` the
/// tokens that must follow each argument (`delimiters[0]` comes before the first argument).
#[derive(Clone, Debug)]
pub(crate) struct Macro<'a> {
    body: Vec<Token<'a>>,
    num_args: usize,
    delimiters: Option<Vec<Vec<Token<'a>>>>,
    /// Set for the `|` and `\|` that `\bra@ket` and `\bra@set` define while they expand their
    /// argument: the macro then stands for the separator and ignores its body.
    mid: Option<Mid<'a>>,
}

/// The `|` or `\|` inside a `\bra@ket` or `\bra@set` argument.
#[derive(Clone, Debug)]
struct Mid<'a> {
    /// Whether this is `\|` (so the double separator is used without looking ahead).
    double: bool,
    /// `\bra@set`: only the first `|` or `\|` is replaced.
    one: bool,
    middle: Vec<Token<'a>>,
    middle_double: Vec<Token<'a>>,
    /// What `|` and `\|` meant before the command, restored by `one`.
    old_middle: Option<Rc<Macro<'a>>>,
    old_double: Option<Rc<Macro<'a>>>,
}

/// The commands that read their own arguments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Builtin {
    /// `\newcommand`: define, and fail when the name is taken.
    NewCommand,
    /// `\renewcommand`: define, and fail when the name is free.
    RenewCommand,
    /// `\providecommand`: define unless the name is taken.
    ProvideCommand,
    Def,
    GlobalDef,
    FirstOfTwo,
    SecondOfTwo,
    IfNextChar,
    TextOrMath,
    /// `\\tag@literal{x}`: records the equation tag as `\\text{x}` in `\\df@tag`.
    TagLiteral,
    /// `\char"41` and its forms: the number, as `\@char{65}`.
    Char,
    /// `\dots`: the dots that suit what follows.
    Dots,
    /// `\dotso`, `\dotsc`, `\cdots`: dots, with a thin space after them before a closing
    /// symbol or a punctuation mark. The `bool` is whether a comma counts as one.
    SpacedDots(Dots),
    /// `\expandafter\a\b`: expands `\b` once, then reads `\a`.
    ExpandAfter,
    /// `\noexpand\a`: `\a` unexpanded.
    NoExpand,
    /// `\message`, `\errmessage`: read their argument and write nothing.
    Message,
    /// `\show`: reads a token and writes nothing.
    Show,
    /// `\bra@ket` (`false`) and `\bra@set` (`true`): four arguments (left, middle, middle for
    /// `||`, right) and the text between, in which `|` stands for the middle.
    BraKet(bool),
}

const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

/// What follows the dots that makes them want a thin space after them: a closing symbol, or
/// punctuation (`\rightdelim@`, `\extra@` and `\extrap@` of amsmath).
const SPACE_AFTER_DOTS: &[&str] = &[
    ")",
    "]",
    "\\rbrack",
    "\\}",
    "\\rbrace",
    "\\rangle",
    "\\rceil",
    "\\rfloor",
    "\\rgroup",
    "\\rmoustache",
    "\\right",
    "\\bigr",
    "\\biggr",
    "\\Bigr",
    "\\Biggr",
    "$",
    ";",
    ".",
    ",",
];

/// The dots command `\dots` stands for when `next` follows it (amsmath's `\mdots@@`): between
/// binary operators and relations, `\dotsb`; after a comma, `\dotsc`; before an integral,
/// `\dotsi`; otherwise `\dotso`.
fn dots_for(next: &TokenKind<'_>) -> &'static str {
    let text = token_text(next);
    match text.as_str() {
        "," => "\\dotsc",
        "\\not"
        | "+"
        | "="
        | "<"
        | ">"
        | "-"
        | "*"
        | ":"
        | "\\DOTSB"
        | "\\coprod"
        | "\\bigvee"
        | "\\bigwedge"
        | "\\biguplus"
        | "\\bigcap"
        | "\\bigcup"
        | "\\prod"
        | "\\sum"
        | "\\bigotimes"
        | "\\bigoplus"
        | "\\bigodot"
        | "\\bigsqcup"
        | "\\And"
        | "\\longrightarrow"
        | "\\Longrightarrow"
        | "\\longleftarrow"
        | "\\Longleftarrow"
        | "\\longleftrightarrow"
        | "\\Longleftrightarrow"
        | "\\mapsto"
        | "\\longmapsto"
        | "\\hookrightarrow"
        | "\\doteq"
        | "\\mathbin"
        | "\\mathrel"
        | "\\relbar"
        | "\\Relbar"
        | "\\xrightarrow"
        | "\\xleftarrow" => "\\dotsb",
        "\\DOTSI" | "\\int" | "\\oint" | "\\iint" | "\\iiint" | "\\iiiint" | "\\idotsint" => {
            "\\dotsi"
        }
        "\\DOTSX" => "\\dotsx",
        _ if text.starts_with("\\not") => "\\dotsb",
        _ => {
            let group = Key::of(next).and_then(|key| {
                key.with_name(|name| symbols::lookup(Mode::Math, name).map(|s| s.group))
            });
            if matches!(group, Some(AtomGroup::Bin | AtomGroup::Rel)) {
                "\\dotsb"
            } else {
                "\\dotso"
            }
        }
    }
}

/// The three dots commands that add a thin space before a closing symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dots {
    /// `\dotso`.
    Other,
    /// `\dotsc`: not before a comma.
    Comma,
    /// `\cdots`.
    Centered,
}

impl Builtin {
    fn named(word: &str) -> Option<Self> {
        Some(match word {
            "newcommand" => Builtin::NewCommand,
            "renewcommand" => Builtin::RenewCommand,
            "providecommand" => Builtin::ProvideCommand,
            "def" => Builtin::Def,
            "gdef" => Builtin::GlobalDef,
            "@firstoftwo" => Builtin::FirstOfTwo,
            "@secondoftwo" => Builtin::SecondOfTwo,
            "@ifnextchar" => Builtin::IfNextChar,
            "TextOrMath" => Builtin::TextOrMath,
            "tag@literal" => Builtin::TagLiteral,
            "char" => Builtin::Char,
            "dots" => Builtin::Dots,
            "dotso" => Builtin::SpacedDots(Dots::Other),
            "dotsc" => Builtin::SpacedDots(Dots::Comma),
            "cdots" => Builtin::SpacedDots(Dots::Centered),
            "expandafter" => Builtin::ExpandAfter,
            "noexpand" => Builtin::NoExpand,
            "message" | "errmessage" => Builtin::Message,
            "show" => Builtin::Show,
            "bra@ket" => Builtin::BraKet(false),
            "bra@set" => Builtin::BraKet(true),
            _ => return None,
        })
    }
}

/// Commands the parser handles that are not macros, so a definition may not take their name.
const IMPLICIT: &[&str] = &["limits", "nolimits"];

/// A macro argument: its tokens in order, and the source range from its first token to its last
/// (a braced argument includes the braces; the tokens do not).
#[derive(Clone, Debug)]
pub(crate) struct Arg<'a> {
    pub tokens: Vec<Token<'a>>,
    pub span: Range<usize>,
    /// The range of the token that ended it (the closing brace, or a delimiter).
    pub end: Range<usize>,
}

pub(crate) struct Expander<'a> {
    lexer: Lexer<'a>,
    /// Tokens put back or produced by expansion, the next one last.
    stack: Vec<Token<'a>>,
    macros: HashMap<Key<'a>, Rc<Macro<'a>>>,
    /// One list per open group of what to restore when it closes, newest last.
    undo: Vec<Vec<(Key<'a>, Option<Rc<Macro<'a>>>)>>,
    mode: Mode,
    expansions: usize,
    produced: usize,
}

fn expected(token: &Token<'_>) -> String {
    token_text(&token.kind)
}

/// A token as it reads in TeX, for messages.
pub(crate) fn token_text(kind: &TokenKind<'_>) -> String {
    match kind {
        TokenKind::Space => " ".into(),
        TokenKind::ControlWord(word) => format!("\\{word}"),
        TokenKind::ControlSymbol(c) => format!("\\{c}"),
        TokenKind::Char(text) => (*text).into(),
        TokenKind::Verb {
            star,
            delimiter,
            body,
        } => format!(
            "\\verb{}{delimiter}{body}{delimiter}",
            if *star { "*" } else { "" }
        ),
        TokenKind::Frozen(inner) => token_text(inner),
        TokenKind::Eof => "EOF".into(),
    }
}

/// How much of `limits::MAX_EXPANDED` a token uses.
fn token_len(kind: &TokenKind<'_>) -> usize {
    match kind {
        TokenKind::Space => 1,
        TokenKind::ControlWord(word) => 1 + word.len(),
        TokenKind::ControlSymbol(c) => 1 + c.len_utf8(),
        TokenKind::Char(text) => text.len(),
        TokenKind::Verb {
            body, delimiter, ..
        } => 6 + body.len() + 2 * delimiter.len_utf8(),
        TokenKind::Frozen(inner) => token_len(inner),
        TokenKind::Eof => 0,
    }
}

fn is_char(token: &Token<'_>, text: &str) -> bool {
    matches!(token.kind, TokenKind::Char(c) if c == text)
}

/// Whether the argument read so far ends with the delimiters. Each must sit outside every brace
/// group, except a delimiter that is itself `{`, which is read one level in (it opens a group).
fn ends_with(tokens: &[Token<'_>], depths: &[i32], delims: &[Token<'_>]) -> bool {
    let Some(from) = tokens.len().checked_sub(delims.len()) else {
        return false;
    };
    tokens[from..]
        .iter()
        .zip(&depths[from..])
        .zip(delims)
        .all(|((token, &depth), want)| {
            token.kind == want.kind && (depth == 0 || (depth == 1 && is_char(want, "{")))
        })
}

/// The delimiter token the argument is waiting for: the one after the longest run of delimiters
/// the argument so far ends with.
fn next_delimiter<'d, 'a>(
    tokens: &[Token<'a>],
    depths: &[i32],
    delims: &'d [Token<'a>],
) -> &'d Token<'a> {
    let matched = (1..delims.len())
        .rev()
        .find(|&k| ends_with(tokens, depths, &delims[..k]))
        .unwrap_or(0);
    &delims[matched]
}

/// How many arguments a string macro takes: the highest `#n` in a row from `#1`, ignoring `##`.
fn count_args(body: &str) -> usize {
    if !body.contains('#') {
        return 0;
    }
    let stripped = body.replace("##", "");
    let mut count = 0;
    while stripped.contains(&format!("#{}", count + 1)) {
        count += 1;
    }
    count
}

impl<'a> Expander<'a> {
    /// An expander over `src`, reading in `mode`.
    pub(crate) fn new(src: &'a str, mode: Mode) -> Result<Self, MathError> {
        Ok(Self {
            lexer: Lexer::new(src)?,
            stack: Vec::new(),
            macros: HashMap::new(),
            undo: Vec::new(),
            mode,
            expansions: 0,
            produced: 0,
        })
    }

    /// Switches between text and math mode, which `\TextOrMath` reads.
    pub(crate) fn switch_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    /// Makes `%` a plain character in the source read from now on (or a comment again), as it is
    /// in the argument of `\url`. Returns the setting it replaces.
    pub(crate) fn set_plain_percent(&mut self, plain: bool) -> bool {
        self.lexer.set_plain_percent(plain)
    }

    // Groups: a definition inside `{...}` ends with it.

    pub(crate) fn begin_group(&mut self, at: &Range<usize>) -> Result<(), MathError> {
        if self.undo.len() >= limits::MAX_DEPTH {
            return Err(MathError::new(
                format!("groups nested more than {} deep", limits::MAX_DEPTH),
                at.clone(),
            ));
        }
        self.undo.push(Vec::new());
        Ok(())
    }

    pub(crate) fn end_group(&mut self, at: &Range<usize>) -> Result<(), MathError> {
        let Some(undo) = self.undo.pop() else {
            return Err(MathError::new(
                "Unbalanced namespace destruction: attempt to pop global namespace",
                at.clone(),
            ));
        };
        for (key, old) in undo.into_iter().rev() {
            match old {
                Some(old) => self.macros.insert(key, old),
                None => self.macros.remove(&key),
            };
        }
        Ok(())
    }

    /// Closes every open group, as after an error in the middle of one.
    #[cfg(test)]
    pub(crate) fn end_groups(&mut self) {
        while let Some(undo) = self.undo.pop() {
            for (key, old) in undo.into_iter().rev() {
                match old {
                    Some(old) => self.macros.insert(key, old),
                    None => self.macros.remove(&key),
                };
            }
        }
    }

    fn set(&mut self, key: Key<'a>, definition: Macro<'a>, global: bool) {
        if global {
            // A global definition outlives every group, so none may restore the old one.
            for undo in &mut self.undo {
                undo.retain(|(k, _)| *k != key);
            }
        } else if let Some(undo) = self.undo.last_mut() {
            undo.push((key, self.macros.get(&key).cloned()));
        }
        self.macros.insert(key, Rc::new(definition));
    }

    // The token stack.

    /// The next token without expanding or removing it.
    pub(crate) fn future(&mut self) -> Result<&Token<'a>, MathError> {
        if self.stack.is_empty() {
            let token = self.lexer.next_token()?;
            self.stack.push(token);
        }
        match self.stack.last() {
            Some(token) => Ok(token),
            None => Err(MathError::new("Empty token stack", 0..0)),
        }
    }

    /// Removes and returns the next token, unexpanded.
    pub(crate) fn pop_token(&mut self) -> Result<Token<'a>, MathError> {
        self.future()?;
        match self.stack.pop() {
            Some(token) => Ok(token),
            None => Err(MathError::new("Empty token stack", 0..0)),
        }
    }

    pub(crate) fn push_token(&mut self, token: Token<'a>) {
        self.stack.push(token);
    }

    /// Puts `tokens`, in reading order, back in front of the rest.
    pub(crate) fn push_tokens(&mut self, tokens: Vec<Token<'a>>) {
        self.stack.extend(tokens.into_iter().rev());
    }

    pub(crate) fn consume_spaces(&mut self) -> Result<(), MathError> {
        while self.future()?.kind == TokenKind::Space {
            self.stack.pop();
        }
        Ok(())
    }

    // Arguments.

    /// Reads one macro argument. With no delimiters it is the next non-blank token, or the whole
    /// `{...}` group if that is a brace, with the outer braces removed. With delimiters it is the
    /// shortest run of tokens, braces balanced, ending in exactly those tokens.
    pub(crate) fn consume_arg(
        &mut self,
        delims: Option<&[Token<'a>]>,
    ) -> Result<Arg<'a>, MathError> {
        let delims = delims.filter(|d| !d.is_empty());
        if delims.is_none() {
            // TeX does not take a space as an undelimited argument.
            self.consume_spaces()?;
        }
        let start = self.future()?.clone();
        let braced = is_char(&start, "{");
        let mut tokens: Vec<Token<'a>> = Vec::new();
        // The brace depth after each token, which a delimiter's position is judged by.
        let mut depths: Vec<i32> = Vec::new();
        let mut depth = 0_i32;
        // Where the group the argument opens with closes, if it does.
        let mut group_end: Option<usize> = None;
        let end = loop {
            let token = self.pop_token()?;
            if is_char(&token, "{") {
                depth += 1;
            } else if is_char(&token, "}") {
                depth -= 1;
                if depth == -1 {
                    return Err(MathError::new("Extra }", token.span));
                }
            } else if token.kind == TokenKind::Eof {
                let want = delims.map_or_else(
                    || "}".to_string(),
                    |d| expected(next_delimiter(&tokens, &depths, d)),
                );
                return Err(MathError::new(
                    format!("Unexpected end of input in a macro argument, expected '{want}'"),
                    token.span,
                ));
            }
            tokens.push(token.clone());
            depths.push(depth);
            if braced && depth == 0 && group_end.is_none() {
                group_end = Some(tokens.len() - 1);
            }
            match delims {
                Some(delims) => {
                    if ends_with(&tokens, &depths, delims) {
                        // The delimiters are not part of the argument.
                        tokens.truncate(tokens.len() - delims.len());
                        break token;
                    }
                }
                None if depth == 0 => break token,
                None => {}
            }
        };
        // An argument that is one braced group loses its outer braces; `{a}{b}` keeps them.
        if group_end.is_some() && group_end == tokens.len().checked_sub(1) {
            tokens.pop();
            tokens.remove(0);
        }
        Ok(Arg {
            tokens,
            span: start.span.start..end.span.end,
            end: end.span,
        })
    }

    /// Reads `count` arguments, each after the delimiters a `\def` gave it.
    fn consume_args(
        &mut self,
        count: usize,
        delimiters: Option<&[Vec<Token<'a>>]>,
    ) -> Result<Vec<Vec<Token<'a>>>, MathError> {
        if let Some(delimiters) = delimiters {
            if delimiters.len() != count + 1 {
                return Err(MathError::new(
                    "The length of delimiters doesn't match the number of args!",
                    0..0,
                ));
            }
            for want in &delimiters[0] {
                let token = self.pop_token()?;
                if token.kind != want.kind {
                    return Err(MathError::new(
                        "Use of the macro doesn't match its definition",
                        token.span,
                    ));
                }
            }
        }
        let mut args = Vec::with_capacity(count.min(9));
        for i in 0..count {
            let delims = delimiters.map(|d| d[i + 1].as_slice());
            args.push(self.consume_arg(delims)?.tokens);
        }
        Ok(args)
    }

    /// Reads a function's argument for the parser, then marks its end with an `Eof` token so the
    /// parser can read it as a formula of its own. `optional` reads `[...]` if it is next, and
    /// returns `None` if not.
    pub(crate) fn scan_argument(&mut self, optional: bool) -> Result<Option<Arg<'a>>, MathError> {
        let (arg, start) = if optional {
            self.consume_spaces()?;
            if !is_char(self.future()?, "[") {
                return Ok(None);
            }
            let open = self.pop_token()?;
            let close = Token {
                kind: TokenKind::Char("]"),
                span: 0..0,
            };
            (self.consume_arg(Some(&[close]))?, open.span.start)
        } else {
            let arg = self.consume_arg(None)?;
            let start = arg.span.start;
            (arg, start)
        };
        self.push_token(Token {
            kind: TokenKind::Eof,
            span: arg.end.clone(),
        });
        self.push_tokens(arg.tokens.clone());
        Ok(Some(Arg {
            tokens: arg.tokens,
            span: start..arg.span.end,
            end: arg.end,
        }))
    }

    // Expansion.

    fn count_expansion(&mut self, at: &Range<usize>) -> Result<(), MathError> {
        self.count_expansions(1, at)
    }

    fn count_expansions(&mut self, n: usize, at: &Range<usize>) -> Result<(), MathError> {
        self.expansions = self.expansions.saturating_add(n);
        if self.expansions > limits::MAX_EXPANSIONS {
            return Err(MathError::new(
                format!(
                    "Too many expansions: infinite loop or more than {} macro calls",
                    limits::MAX_EXPANSIONS
                ),
                at.clone(),
            ));
        }
        Ok(())
    }

    fn produce(&mut self, tokens: &[Token<'a>], at: &Range<usize>) -> Result<(), MathError> {
        self.produced += tokens.iter().map(|t| token_len(&t.kind)).sum::<usize>();
        if self.produced > limits::MAX_EXPANDED {
            return Err(MathError::new(
                format!(
                    "Macro expansion produced more than {} bytes",
                    limits::MAX_EXPANDED
                ),
                at.clone(),
            ));
        }
        Ok(())
    }

    /// Expands the next token once if it is a macro, putting the result back on the stack, and
    /// says whether it did. A token that is not a macro stays where it was.
    pub(crate) fn expand_once(&mut self) -> Result<bool, MathError> {
        self.expand_once_in(false)
    }

    /// `expand_once`; with `expandable_only` a control sequence that means nothing at all is an
    /// error, as it is after `\expandafter`.
    fn expand_once_in(&mut self, expandable_only: bool) -> Result<bool, MathError> {
        let token = self.pop_token()?;
        let Some(key) = Key::of(&token.kind) else {
            self.push_token(token);
            return Ok(false);
        };
        let tokens = if let Some(definition) = self.macros.get(&key).cloned() {
            self.count_expansion(&token.span)?;
            self.instantiate(&definition)?
        } else if let Some(builtin) = Self::builtin(&key)
            // KaTeX's `\def` is a function of the parser, not a macro: `\expandafter` leaves it.
            && !(expandable_only && matches!(builtin, Builtin::Def | Builtin::GlobalDef))
        {
            self.count_expansion(&token.span)?;
            self.run(builtin, &token)?
        } else if let Some(definition) = Self::predefined(&key, &token.span)? {
            self.count_expansion(&token.span)?;
            self.instantiate(&definition)?
        } else {
            let is_control = matches!(
                token.kind,
                TokenKind::ControlWord(_) | TokenKind::ControlSymbol(_)
            );
            if expandable_only && is_control && !self.is_defined(&key) {
                return Err(MathError::new(
                    format!("Undefined control sequence: {}", token_text(&token.kind)),
                    token.span,
                ));
            }
            self.push_token(token);
            return Ok(false);
        };
        self.produce(&tokens, &token.span)?;
        self.push_tokens(tokens);
        Ok(true)
    }

    /// The next token that is not a macro, with every macro before it expanded. A token that
    /// `\noexpand` froze comes out as `\relax`.
    pub(crate) fn expand_next_token(&mut self) -> Result<Token<'a>, MathError> {
        while self.expand_once()? {}
        let token = self.pop_token()?;
        Ok(match token.kind {
            TokenKind::Frozen(_) => Token {
                kind: TokenKind::ControlWord("relax"),
                span: token.span,
            },
            _ => token,
        })
    }

    /// Whether `\noexpand` would stop `key` from expanding: it is a macro, or a function that
    /// reads its own arguments.
    fn is_expandable(&self, key: &Key<'a>) -> bool {
        self.macros.contains_key(key)
            || matches!(key, Key::Word(word)
                if Builtin::named(word).is_some_and(|b| !matches!(b, Builtin::Def | Builtin::GlobalDef))
            )
            || key.with_name(|name| {
                let (table, text) = match name {
                    Name::Control(k) => (macros_data::CONTROLS, k),
                    Name::Char(k) => (macros_data::CHARS, k),
                };
                table.binary_search_by(|(t, _)| (*t).cmp(text)).is_ok()
            })
            || functions::lookup(&key.display()).is_some_and(|f| !f.primitive)
    }

    fn builtin(key: &Key<'a>) -> Option<Builtin> {
        match key {
            Key::Word(word) => Builtin::named(word),
            _ => None,
        }
    }

    /// KaTeX's predefined macro called `key`, lexed from its source with every token placed at
    /// the call.
    fn predefined(key: &Key<'a>, call: &Range<usize>) -> Result<Option<Rc<Macro<'a>>>, MathError> {
        let body = key.with_name(|name| {
            let (table, key) = match name {
                Name::Control(key) => (macros_data::CONTROLS, key),
                Name::Char(key) => (macros_data::CHARS, key),
            };
            table
                .binary_search_by(|(k, _)| (*k).cmp(key))
                .ok()
                .map(|at| table[at].1)
        });
        let Some(body) = body else {
            return Ok(None);
        };
        let mut lexer = Lexer::new(body).map_err(|e| MathError::new(e.message, call.clone()))?;
        let mut tokens = Vec::new();
        loop {
            let mut token = lexer
                .next_token()
                .map_err(|e| MathError::new(e.message, call.clone()))?;
            if token.kind == TokenKind::Eof {
                break;
            }
            token.span = call.clone();
            tokens.push(token);
        }
        Ok(Some(Rc::new(Macro {
            body: tokens,
            num_args: count_args(body),
            delimiters: None,
            mid: None,
        })))
    }

    /// The tokens a macro call expands to: its arguments are read and put in place of `#1` to
    /// `#9`, and `##` is a single `#`.
    fn instantiate(&mut self, definition: &Macro<'a>) -> Result<Vec<Token<'a>>, MathError> {
        if let Some(mid) = &definition.mid {
            return self.separator(mid);
        }
        let args = self.consume_args(definition.num_args, definition.delimiters.as_deref())?;
        if definition.num_args == 0 {
            return Ok(definition.body.clone());
        }
        let mut out = Vec::with_capacity(definition.body.len());
        // A body that repeats a long argument can be far larger than either, so the size is
        // checked as the tokens are added, not after.
        let mut size = self.produced;
        let mut body = definition.body.iter();
        while let Some(token) = body.next() {
            if !is_char(token, "#") {
                out.push(token.clone());
                continue;
            }
            let Some(next) = body.next() else {
                return Err(MathError::new(
                    "Incomplete placeholder at end of macro body",
                    token.span.clone(),
                ));
            };
            if is_char(next, "#") {
                out.push(next.clone());
                continue;
            }
            let index = match next.kind {
                TokenKind::Char(digit) if digit.len() == 1 => {
                    digit.as_bytes()[0].checked_sub(b'1').map(usize::from)
                }
                _ => None,
            };
            match index.and_then(|i| args.get(i)) {
                Some(arg) => {
                    size += arg.iter().map(|t| token_len(&t.kind)).sum::<usize>();
                    if size > limits::MAX_EXPANDED {
                        return Err(MathError::new(
                            format!(
                                "Macro expansion produced more than {} bytes",
                                limits::MAX_EXPANDED
                            ),
                            token.span.clone(),
                        ));
                    }
                    out.extend(arg.iter().cloned());
                }
                None => {
                    return Err(MathError::new(
                        "Not a valid argument number",
                        next.span.clone(),
                    ));
                }
            }
        }
        Ok(out)
    }

    // The commands that read their own arguments.

    fn run(&mut self, builtin: Builtin, call: &Token<'a>) -> Result<Vec<Token<'a>>, MathError> {
        match builtin {
            Builtin::NewCommand => self.new_command(call, false, true, false),
            Builtin::RenewCommand => self.new_command(call, true, false, false),
            Builtin::ProvideCommand => self.new_command(call, true, true, true),
            Builtin::Def => self.define(false),
            Builtin::GlobalDef => self.define(true),
            Builtin::FirstOfTwo => Ok(self.consume_args(2, None)?.swap_remove(0)),
            Builtin::SecondOfTwo => Ok(self.consume_args(2, None)?.swap_remove(1)),
            Builtin::IfNextChar => {
                let mut args = self.consume_args(3, None)?;
                self.consume_spaces()?;
                let next = self.future()?.kind.clone();
                let same = args[0].len() == 1 && args[0][0].kind == next;
                Ok(args.swap_remove(if same { 1 } else { 2 }))
            }
            Builtin::TagLiteral => {
                // The tag is kept in a global macro, which the parser reads at the end of the row
                // (or of the formula); a second `\\tag` in the same place is an error.
                if self.macros.contains_key(&Key::Word("df@tag")) {
                    return Err(MathError::new("Multiple \\tag", call.span.clone()));
                }
                let arg = self.consume_args(1, None)?.swap_remove(0);
                let at = &call.span;
                let word = |w: &'static str| Token {
                    kind: TokenKind::ControlWord(w),
                    span: at.clone(),
                };
                let brace = |c: &'static str| Token {
                    kind: TokenKind::Char(c),
                    span: at.clone(),
                };
                let mut tokens = vec![
                    word("gdef"),
                    word("df@tag"),
                    brace("{"),
                    word("text"),
                    brace("{"),
                ];
                tokens.extend(arg);
                tokens.push(brace("}"));
                tokens.push(brace("}"));
                Ok(tokens)
            }
            Builtin::TextOrMath => {
                let mut args = self.consume_args(2, None)?;
                Ok(args.swap_remove(if self.mode == Mode::Text { 0 } else { 1 }))
            }
            Builtin::Char => {
                let number = self.char_number()?;
                let at = &call.span;
                let token = |kind| Token {
                    kind,
                    span: at.clone(),
                };
                let mut tokens = vec![
                    token(TokenKind::ControlWord("@char")),
                    token(TokenKind::Char("{")),
                ];
                for digit in number.to_string().bytes() {
                    let text = DIGITS.get(usize::from(digit.wrapping_sub(b'0'))).copied();
                    tokens.extend(text.map(|text| token(TokenKind::Char(text))));
                }
                tokens.push(token(TokenKind::Char("}")));
                Ok(tokens)
            }
            Builtin::Dots => {
                // The next token, after one expansion, says what the dots go with.
                self.expand_once_in(false)?;
                let next = self.future()?.clone();
                let name = dots_for(&next.kind);
                self.static_tokens(name, &call.span)
            }
            Builtin::SpacedDots(which) => {
                let next = token_text(&self.future()?.kind);
                let spaced = SPACE_AFTER_DOTS.contains(&next.as_str())
                    && !(which == Dots::Comma && next == ",");
                let text = match (which, spaced) {
                    (Dots::Centered, true) => "\\@cdots\\,",
                    (Dots::Centered, false) => "\\@cdots",
                    (_, true) => "\\ldots\\,",
                    (_, false) => "\\ldots",
                };
                self.static_tokens(text, &call.span)
            }
            Builtin::ExpandAfter => {
                // The token after this one is read as it stands; the one after that is
                // expanded once, and the first goes back in front of the result.
                let first = self.pop_token()?;
                self.expand_once_in(true)?;
                Ok(vec![first])
            }
            Builtin::NoExpand => {
                let token = self.pop_token()?;
                let expandable = Key::of(&token.kind).is_some_and(|key| self.is_expandable(&key));
                Ok(vec![if expandable {
                    Token {
                        kind: TokenKind::Frozen(Box::new(token.kind)),
                        span: token.span,
                    }
                } else {
                    token
                }])
            }
            Builtin::Message => {
                // Nothing is written anywhere: the argument is read and dropped.
                self.consume_args(1, None)?;
                Ok(Vec::new())
            }
            Builtin::Show => {
                self.pop_token()?;
                Ok(Vec::new())
            }
            Builtin::BraKet(one) => self.bra_ket(one, &call.span),
        }
    }

    /// Makes `key` mean `old` (nothing if `None`) in the current group.
    fn restore(&mut self, key: Key<'a>, old: Option<Rc<Macro<'a>>>) {
        if let Some(undo) = self.undo.last_mut() {
            undo.push((key, self.macros.get(&key).cloned()));
        }
        match old {
            Some(old) => self.macros.insert(key, old),
            None => self.macros.remove(&key),
        };
    }

    /// Expands `tokens` (in reading order) as far as macros go, and returns what is left, also in
    /// reading order. Tokens that are not macros are kept as they are.
    fn expand_tokens(
        &mut self,
        tokens: Vec<Token<'a>>,
        at: &Range<usize>,
    ) -> Result<Vec<Token<'a>>, MathError> {
        let mut out = Vec::new();
        let base = self.stack.len();
        self.push_tokens(tokens);
        while self.stack.len() > base {
            if !self.expand_once_in(true)? {
                let mut token = self.pop_token()?;
                // The expansion of `\noexpand\x` is `\x` itself, expandable again later.
                if let TokenKind::Frozen(inner) = token.kind {
                    token.kind = *inner;
                }
                out.push(token);
            }
        }
        // Each token out counts as an expansion, as in KaTeX, so repeated expansion of long
        // text is bounded by the expansion limit.
        self.count_expansions(out.len(), at)?;
        Ok(out)
    }

    /// `\bra@ket{left}{middle}{middle for ||}{right}<text>` and `\bra@set`: `left`, the text with
    /// each `|` replaced by `middle` (and `||` or `\|` by the third argument, if it is not empty),
    /// then `right`. For `\bra@set` only the first `|` is replaced.
    fn bra_ket(&mut self, one: bool, at: &Range<usize>) -> Result<Vec<Token<'a>>, MathError> {
        let left = self.consume_arg(None)?.tokens;
        let middle = self.consume_arg(None)?.tokens;
        let middle_double = self.consume_arg(None)?.tokens;
        let right = self.consume_arg(None)?.tokens;
        let old_middle = self.macros.get(&Key::Char("|")).cloned();
        let old_double = self.macros.get(&Key::Symbol('|')).cloned();
        self.begin_group(at)?;
        let result = (|| {
            let separator = |double: bool| Macro {
                body: Vec::new(),
                num_args: 0,
                delimiters: None,
                mid: Some(Mid {
                    double,
                    one,
                    middle: middle.clone(),
                    middle_double: middle_double.clone(),
                    old_middle: old_middle.clone(),
                    old_double: old_double.clone(),
                }),
            };
            self.set(Key::Char("|"), separator(false), false);
            if !middle_double.is_empty() {
                self.set(Key::Symbol('|'), separator(true), false);
            }
            let arg = self.consume_arg(None)?.tokens;
            let mut input = left.clone();
            input.extend(arg);
            input.extend(right.iter().cloned());
            self.expand_tokens(input, at)
        })();
        self.end_group(at)?;
        result
    }

    /// What `|` or `\|` stands for inside a `\bra@ket` or `\bra@set` argument.
    fn separator(&mut self, mid: &Mid<'a>) -> Result<Vec<Token<'a>>, MathError> {
        if mid.one {
            // Only the first one is replaced: the rest mean what they did before.
            self.restore(Key::Char("|"), mid.old_middle.clone());
            if !mid.middle_double.is_empty() {
                self.restore(Key::Symbol('|'), mid.old_double.clone());
            }
        }
        let mut doubled = mid.double;
        if !doubled && !mid.middle_double.is_empty() && is_char(self.future()?, "|") {
            // `||`, as `\@ifnextchar` would see it.
            self.pop_token()?;
            doubled = true;
        }
        Ok(if doubled {
            mid.middle_double.clone()
        } else {
            mid.middle.clone()
        })
    }

    /// The tokens of `text`, a constant, all placed at `at`.
    fn static_tokens(
        &self,
        text: &'static str,
        at: &Range<usize>,
    ) -> Result<Vec<Token<'a>>, MathError> {
        let mut lexer = Lexer::new(text)?;
        let mut tokens = Vec::new();
        loop {
            let mut token = lexer.next_token()?;
            if token.kind == TokenKind::Eof {
                return Ok(tokens);
            }
            token.span = at.clone();
            tokens.push(token);
        }
    }

    /// The number after `\char`: decimal, `'` octal, `"` hexadecimal, or `` ` `` and a character.
    fn char_number(&mut self) -> Result<u64, MathError> {
        let mut token = self.pop_token()?;
        let base: u32 = match token_text(&token.kind).as_str() {
            "'" => {
                token = self.pop_token()?;
                8
            }
            "\"" => {
                token = self.pop_token()?;
                16
            }
            "`" => {
                let quoted = self.pop_token()?;
                if quoted.kind == TokenKind::Eof {
                    return Err(MathError::new("\\char` missing argument", quoted.span));
                }
                // The code of the character, or of the one after the backslash.
                let text = token_text(&quoted.kind);
                let named = text.strip_prefix('\\').unwrap_or(&text);
                let unit = named.encode_utf16().next().unwrap_or(0);
                return Ok(u64::from(unit));
            }
            _ => 10,
        };
        let digit = |token: &Token<'_>| match token.kind {
            TokenKind::Char(text) => {
                let mut chars = text.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => c.to_digit(16),
                    _ => None,
                }
            }
            _ => None,
        };
        let Some(mut number) = digit(&token).filter(|d| *d < base).map(u64::from) else {
            return Err(MathError::new(
                format!("Invalid base-{base} digit {}", token_text(&token.kind)),
                token.span,
            ));
        };
        while let Some(next) = digit(self.future()?).filter(|d| *d < base) {
            self.pop_token()?;
            number = number
                .saturating_mul(u64::from(base))
                .saturating_add(u64::from(next));
        }
        Ok(number)
    }

    /// Defines a macro with no arguments whose body is `text`, for the parser's own use
    /// (`\\cr` in an array, the equation-number switch).
    pub(crate) fn set_text_macro(
        &mut self,
        name: &'static str,
        text: &'static str,
        global: bool,
    ) -> Result<(), MathError> {
        let mut lexer = Lexer::new(text)?;
        let mut body = Vec::new();
        loop {
            let mut token = lexer.next_token()?;
            if token.kind == TokenKind::Eof {
                break;
            }
            token.span = 0..0;
            body.push(token);
        }
        let definition = Macro {
            body,
            num_args: 0,
            delimiters: None,
            mid: None,
        };
        self.set(Key::Word(name), definition, global);
        Ok(())
    }

    /// The text of a macro with no arguments, without expanding it, if one is defined.
    pub(crate) fn macro_text(&self, name: &'static str) -> Option<String> {
        let definition = self.macros.get(&Key::Word(name))?;
        Some(
            definition
                .body
                .iter()
                .map(|t| token_text(&t.kind))
                .collect(),
        )
    }

    /// Whether a macro of this name was defined in the formula.
    pub(crate) fn has_macro(&self, name: &'static str) -> bool {
        self.macros.contains_key(&Key::Word(name))
    }

    /// Removes a macro from every group, as a global assignment of nothing does.
    pub(crate) fn clear_macro(&mut self, name: &'static str) {
        let key = Key::Word(name);
        self.macros.remove(&key);
        for undo in &mut self.undo {
            undo.retain(|(k, _)| *k != key);
        }
    }

    /// Whether `key` already means something: a macro, a symbol, or a command the parser reads.
    /// (The parser's own functions join this when they exist.)
    pub(crate) fn is_defined(&self, key: &Key<'a>) -> bool {
        if self.macros.contains_key(key) || functions::is_function(key) {
            return true;
        }
        let word = match key {
            Key::Word(word) => Some(*word),
            _ => None,
        };
        word.is_some_and(|w| Builtin::named(w).is_some() || IMPLICIT.contains(&w))
            || matches!(key, Key::Char("^" | "_"))
            || key.with_name(|name| {
                let in_table = |table: &[(&str, &str)], k: &str| {
                    table.binary_search_by(|(t, _)| (*t).cmp(k)).is_ok()
                };
                let (table, text) = match name {
                    Name::Control(k) => (macros_data::CONTROLS, k),
                    Name::Char(k) => (macros_data::CHARS, k),
                };
                in_table(table, text)
                    || symbols::lookup(Mode::Math, name).is_some()
                    || symbols::lookup(Mode::Text, name).is_some()
            })
    }

    /// `\newcommand{\name}[count]{body}` and its two relatives: the first fails if `\name` is
    /// taken, the second if it is free, the third leaves a taken name alone.
    fn new_command(
        &mut self,
        call: &Token<'a>,
        exists_ok: bool,
        nonexists_ok: bool,
        skip_if_exists: bool,
    ) -> Result<Vec<Token<'a>>, MathError> {
        let name_error = || {
            MathError::new(
                "\\newcommand's first argument must be a macro name",
                call.span.clone(),
            )
        };
        let arg = self.consume_arg(None)?;
        let [name] = arg.tokens.as_slice() else {
            return Err(name_error());
        };
        let key = Key::of(&name.kind).ok_or_else(name_error)?;
        let shown = key.display();
        let exists = self.is_defined(&key);
        if exists && !exists_ok {
            return Err(MathError::new(
                format!(
                    "\\newcommand{{{shown}}} attempting to redefine {shown}; use \\renewcommand"
                ),
                name.span.clone(),
            ));
        }
        if !exists && !nonexists_ok {
            return Err(MathError::new(
                format!(
                    "\\renewcommand{{{shown}}} when command {shown} does not yet exist; use \\newcommand"
                ),
                name.span.clone(),
            ));
        }

        let mut num_args = 0;
        let mut body = self.consume_arg(None)?;
        if let [open] = body.tokens.as_slice()
            && is_char(open, "[")
        {
            let mut digits = String::new();
            loop {
                let token = self.expand_next_token()?;
                if is_char(&token, "]") || token.kind == TokenKind::Eof {
                    break;
                }
                digits.push_str(&token_text(&token.kind));
            }
            // TeX allows at most nine parameters, `#1` to `#9`.
            let count = digits.trim();
            num_args = count
                .bytes()
                .all(|b| b.is_ascii_digit())
                .then(|| count.parse::<usize>().ok())
                .flatten()
                .filter(|n| *n <= 9)
                .ok_or_else(|| {
                    MathError::new(
                        format!("Invalid number of arguments: {digits}"),
                        call.span.clone(),
                    )
                })?;
            body = self.consume_arg(None)?;
        }
        if !(exists && skip_if_exists) {
            self.set(
                key,
                Macro {
                    body: body.tokens,
                    num_args,
                    delimiters: None,
                    mid: None,
                },
                false,
            );
        }
        Ok(Vec::new())
    }

    /// `\def\name<parameter text>{body}` and `\gdef`, which defines outside the current group.
    /// The parameter text is `#1`, `#2`, ... in order, with any tokens between them that must
    /// follow the argument at the call.
    fn define(&mut self, global: bool) -> Result<Vec<Token<'a>>, MathError> {
        let name = self.pop_token()?;
        let key = match Key::of(&name.kind) {
            Some(Key::Char("{" | "}" | "$" | "&" | "#" | "^" | "_")) | None => {
                return Err(MathError::new("Expected a control sequence", name.span));
            }
            Some(key) => key,
        };

        let mut num_args = 0;
        let mut insert: Option<Token<'a>> = None;
        let mut delimiters: Vec<Vec<Token<'a>>> = vec![Vec::new()];
        while !is_char(self.future()?, "{") {
            let token = self.pop_token()?;
            if is_char(&token, "#") {
                // A `#` just before the `{` acts as if the brace were also the last delimiter
                // and the first token of the body.
                if is_char(self.future()?, "{") {
                    let brace = self.future()?.clone();
                    delimiters[num_args].push(brace.clone());
                    insert = Some(brace);
                    break;
                }
                let digit = self.pop_token()?;
                let number = match digit.kind {
                    TokenKind::Char(d)
                        if d.len() == 1 && (b'1'..=b'9').contains(&d.as_bytes()[0]) =>
                    {
                        usize::from(d.as_bytes()[0] - b'0')
                    }
                    _ => {
                        return Err(MathError::new(
                            format!("Invalid argument number \"{}\"", expected(&digit)),
                            digit.span,
                        ));
                    }
                };
                if number != num_args + 1 {
                    return Err(MathError::new(
                        format!("Argument number \"{number}\" out of order"),
                        digit.span,
                    ));
                }
                num_args += 1;
                delimiters.push(Vec::new());
            } else if token.kind == TokenKind::Eof {
                return Err(MathError::new("Expected a macro definition", token.span));
            } else if delimiters[num_args].len() >= limits::MAX_DELIMITERS {
                return Err(MathError::new(
                    format!(
                        "Macro parameter text longer than {} tokens",
                        limits::MAX_DELIMITERS
                    ),
                    token.span,
                ));
            } else {
                delimiters[num_args].push(token);
            }
        }
        let mut body = self.consume_arg(None)?.tokens;
        if let Some(brace) = insert {
            // TeX puts the brace at the right end of the replacement text.
            body.push(brace);
        }
        self.set(
            key,
            Macro {
                body,
                num_args,
                delimiters: Some(delimiters),
                mid: None,
            },
            global,
        );
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TokenKind::*;

    /// Everything the expander reads from `src`, in text, until the end.
    fn expand(src: &str) -> Result<String, MathError> {
        let mut expander = Expander::new(src, Mode::Math)?;
        let mut out = String::new();
        loop {
            let token = expander.expand_next_token()?;
            if token.kind == Eof {
                return Ok(out);
            }
            out.push_str(&token_text(&token.kind));
        }
    }

    fn error(src: &str) -> String {
        expand(src).unwrap_err().message
    }

    #[test]
    fn text_without_macros_passes_through() {
        assert_eq!(expand("a+b\\frac{1}{2}").unwrap(), "a+b\\frac{1}{2}");
        assert_eq!(expand("").unwrap(), "");
        assert_eq!(expand("\\foo").unwrap(), "\\foo");
    }

    #[test]
    fn predefined_macros_expand() {
        // Tokens print without the space a control word swallows, so `\r A` reads `\rA`.
        assert_eq!(expand("\\Alpha").unwrap(), "\\mathrm{A}");
        assert_eq!(expand("\\AA").unwrap(), "\\rA");
        // A macro may take arguments.
        assert_eq!(expand("\\Bra{x}").unwrap(), "\\left\\langlex\\right|");
        // `~` is an active character.
        assert_eq!(expand("a~b").unwrap(), "a\\nobreakspaceb");
    }

    #[test]
    fn newcommand_with_and_without_arguments() {
        assert_eq!(expand("\\newcommand{\\foo}{xy}\\foo\\foo").unwrap(), "xyxy");
        // The space between the two calls is a token of its own.
        assert_eq!(
            expand("\\newcommand\\swap[2]{#2-#1}\\swap ab \\swap{c}{d}").unwrap(),
            "b-a d-c"
        );
        assert_eq!(
            expand("\\newcommand{\\twice}[1]{#1#1}\\twice{\\alpha}").unwrap(),
            "\\alpha\\alpha"
        );
        // `##` is a single `#`.
        assert_eq!(expand("\\newcommand{\\h}[1]{##1#1}\\h x").unwrap(), "#1x");
    }

    #[test]
    fn newcommand_checks_the_name() {
        assert_eq!(
            error("\\newcommand{\\alpha}{x}"),
            "\\newcommand{\\alpha} attempting to redefine \\alpha; use \\renewcommand"
        );
        assert_eq!(
            error("\\renewcommand{\\nope}{x}"),
            "\\renewcommand{\\nope} when command \\nope does not yet exist; use \\newcommand"
        );
        assert_eq!(
            error("\\newcommand{\\a\\b}{x}"),
            "\\newcommand's first argument must be a macro name"
        );
        assert_eq!(expand("\\renewcommand{\\alpha}{x}\\alpha").unwrap(), "x");
        // `\providecommand` defines a free name and leaves a taken one.
        assert_eq!(expand("\\providecommand{\\new}{n}\\new").unwrap(), "n");
        assert_eq!(
            expand("\\providecommand{\\Alpha}{n}\\Alpha").unwrap(),
            "\\mathrm{A}"
        );
        assert_eq!(
            error("\\newcommand\\f[x]{}"),
            "Invalid number of arguments: x"
        );
        assert_eq!(
            error("\\newcommand\\f[10]{}"),
            "Invalid number of arguments: 10"
        );
    }

    #[test]
    fn def_with_delimited_parameters() {
        assert_eq!(expand("\\def\\a{x}\\a\\a").unwrap(), "xx");
        assert_eq!(
            expand("\\def\\pair#1,#2.{(#1;#2)}\\pair a,b.c").unwrap(),
            "(a;b)c"
        );
        // A delimiter before the first argument must match.
        assert_eq!(expand("\\def\\f x#1{[#1]}\\f xy").unwrap(), "[y]");
        assert_eq!(
            error("\\def\\f x#1{[#1]}\\f zy"),
            "Use of the macro doesn't match its definition"
        );
        // `#{` makes the brace both the last delimiter and the first token of the body.
        assert_eq!(expand("\\def\\g#1#{[#1]}\\g ab{c}").unwrap(), "[ab]{c}");
        assert_eq!(error("\\def\\f#2{}"), "Argument number \"2\" out of order");
        assert_eq!(error("\\def\\f#a{}"), "Invalid argument number \"a\"");
        assert_eq!(error("\\def{x}"), "Expected a control sequence");
        assert_eq!(error("\\def\\f"), "Expected a macro definition");
    }

    #[test]
    fn a_delimited_argument_is_the_shortest_run_before_the_delimiters() {
        // After a partial match, the token that broke it may start the delimiters again.
        assert_eq!(expand("\\def\\f#1ab{[#1]}\\f aab").unwrap(), "[a]");
        assert_eq!(expand("\\def\\f#1aab{[#1]}\\f aaaab").unwrap(), "[aa]");
        // A delimiter inside braces does not end the argument.
        assert_eq!(expand("\\def\\f#1.{[#1]}\\f{a.}b.").unwrap(), "[{a.}b]");
        assert_eq!(expand("\\def\\f#1.{[#1]}\\f a{b.}c.").unwrap(), "[a{b.}c]");
        // An empty argument is allowed.
        assert_eq!(expand("\\def\\f#1.{[#1]}\\f .").unwrap(), "[]");
        // The message names the delimiter the argument is waiting for.
        assert_eq!(
            error("\\def\\f#1ab{}\\f xa"),
            "Unexpected end of input in a macro argument, expected 'b'"
        );
        assert_eq!(
            error("\\def\\f#1ab{}\\f x"),
            "Unexpected end of input in a macro argument, expected 'a'"
        );
    }

    #[test]
    fn only_a_whole_braced_group_loses_its_braces() {
        assert_eq!(expand("\\def\\f#1.{[#1]}\\f{a}.").unwrap(), "[a]");
        assert_eq!(expand("\\def\\f#1.{[#1]}\\f{a}{b}.").unwrap(), "[{a}{b}]");
        assert_eq!(expand("\\def\\f#1.{[#1]}\\f{{a}}.").unwrap(), "[{a}]");
        assert_eq!(expand("\\def\\f#1{[#1]}\\f{{a}}").unwrap(), "[{a}]");
        assert_eq!(expand("\\def\\f#1{[#1]}\\f{}").unwrap(), "[]");
    }

    #[test]
    fn a_placeholder_must_name_an_argument() {
        assert_eq!(
            error("\\newcommand\\f[1]{#}\\f x"),
            "Incomplete placeholder at end of macro body"
        );
        assert_eq!(
            error("\\newcommand\\f[1]{#a}\\f x"),
            "Not a valid argument number"
        );
        assert_eq!(
            error("\\newcommand\\f[1]{#2}\\f x"),
            "Not a valid argument number"
        );
    }

    /// Reads tokens until the character `stop`, as a parser reads up to a brace.
    fn read_to(e: &mut Expander<'_>, stop: &str) {
        while !is_char(&e.expand_next_token().unwrap(), stop) {}
    }

    #[test]
    fn groups_scope_definitions() {
        let at = 0..0;
        let a = Key::Word("a");
        let mut e = Expander::new("|\\def\\a{x}|", Mode::Math).unwrap();
        read_to(&mut e, "|");
        e.begin_group(&at).unwrap();
        read_to(&mut e, "|");
        assert!(e.is_defined(&a));
        e.end_group(&at).unwrap();
        assert!(!e.is_defined(&a));
        assert!(e.end_group(&at).is_err());
    }

    #[test]
    fn a_global_definition_outlives_its_group_and_a_local_one_restores_the_outer() {
        let at = 0..0;
        fn body<'a>(e: &Expander<'a>, name: &'a str) -> Option<TokenKind<'a>> {
            e.macros
                .get(&Key::Word(name))
                .map(|m| m.body[0].kind.clone())
        }
        let src = "\\def\\a{1}\\def\\b{9}|\\gdef\\g{2}\\def\\a{3}\\def\\b{4}|";
        let mut e = Expander::new(src, Mode::Math).unwrap();
        read_to(&mut e, "|");
        e.begin_group(&at).unwrap();
        read_to(&mut e, "|");
        assert_eq!(body(&e, "a"), Some(Char("3")));
        e.end_group(&at).unwrap();
        assert_eq!(body(&e, "g"), Some(Char("2")));
        assert_eq!(body(&e, "a"), Some(Char("1")));
        assert_eq!(body(&e, "b"), Some(Char("9")));
    }

    #[test]
    fn end_groups_closes_every_group() {
        let at = 0..0;
        let mut e = Expander::new("|\\def\\a{1}|\\def\\a{2}|", Mode::Math).unwrap();
        read_to(&mut e, "|");
        e.begin_group(&at).unwrap();
        read_to(&mut e, "|");
        e.begin_group(&at).unwrap();
        read_to(&mut e, "|");
        e.end_groups();
        assert!(!e.is_defined(&Key::Word("a")));
        assert!(e.end_group(&at).is_err());
    }

    #[test]
    fn look_ahead_and_mode_commands() {
        assert_eq!(expand("\\@firstoftwo{a}{b}").unwrap(), "a");
        assert_eq!(expand("\\@secondoftwo{a}{b}").unwrap(), "b");
        assert_eq!(expand("\\@ifnextchar x{yes}{no}x").unwrap(), "yesx");
        assert_eq!(expand("\\@ifnextchar x{yes}{no}  z").unwrap(), "noz");
        assert_eq!(expand("\\TextOrMath{t}{m}").unwrap(), "m");
        let mut e = Expander::new("\\TextOrMath{t}{m}", Mode::Text).unwrap();
        assert_eq!(e.expand_next_token().unwrap().kind, Char("t"));
        // `\@ifstar{A}{B}` is a predefined macro built on them: A if a star follows, and the star
        // goes, or else B.
        assert_eq!(expand("\\@ifstar{S}{T}*x").unwrap(), "Sx");
        assert_eq!(expand("\\@ifstar{S}{T}x").unwrap(), "Tx");
    }

    /// What `\,`, `\:` and `\;` expand to.
    const THIN: &str = "\\mskip+3mu\\relax";
    const MEDIUM: &str = "\\mskip+4mu\\relax";
    const THICK: &str = "\\mskip+5mu\\relax";

    #[test]
    fn bra_and_ket_without_separators_are_plain_macros() {
        assert_eq!(expand("\\bra{x}").unwrap(), "\\mathinner{\\langle{x}|}");
        assert_eq!(expand("\\ket{x}").unwrap(), "\\mathinner{|{x}\\rangle}");
        assert_eq!(
            expand("\\braket{x}").unwrap(),
            "\\mathinner{\\langle{x}\\rangle}"
        );
        // `\braket` leaves a `|` as it is; only `\Braket` reads it.
        assert_eq!(
            expand("\\braket{\\phi|\\psi}").unwrap(),
            "\\mathinner{\\langle{\\phi|\\psi}\\rangle}"
        );
        assert_eq!(expand("\\Bra{x}").unwrap(), "\\left\\langlex\\right|");
        assert_eq!(expand("\\Ket{x}").unwrap(), "\\left|x\\right\\rangle");
    }

    #[test]
    fn upper_case_braket_replaces_every_bar() {
        let bar = format!("{THIN}\\middle\\vert{THIN}");
        assert_eq!(
            expand("\\Braket{\\phi|\\psi}").unwrap(),
            format!("\\left\\langle\\phi{bar}\\psi\\right\\rangle")
        );
        assert_eq!(
            expand("\\Braket{a|b|c}").unwrap(),
            format!("\\left\\langlea{bar}b{bar}c\\right\\rangle")
        );
        // `||` and `\|` take the double separator, which for `\Braket` is the same.
        let want = format!("\\left\\langlea{bar}b\\right\\rangle");
        assert_eq!(expand("\\Braket{a||b}").unwrap(), want);
        assert_eq!(expand("\\Braket{a\\|b}").unwrap(), want);
        assert_eq!(
            expand("\\Braket{}").unwrap(),
            "\\left\\langle\\right\\rangle"
        );
        // Spaces after a control word are not tokens.
        assert_eq!(
            expand("\\Braket{ \\phi | \\frac{\\partial^2}{\\partial t^2} | \\psi }").unwrap(),
            format!(
                "\\left\\langle \\phi{bar} \\frac{{\\partial^2}}{{\\partialt^2}} {bar} \\psi\\right\\rangle"
            )
        );
    }

    #[test]
    fn set_replaces_only_the_first_bar() {
        let t = THIN;
        assert_eq!(
            expand("\\set{x|x<5}").unwrap(),
            format!("\\{{{t}x\\midx<5{t}\\}}")
        );
        assert_eq!(
            expand("\\set{x|y|z}").unwrap(),
            format!("\\{{{t}x\\midy|z{t}\\}}")
        );
        // `\set` has no double separator, so `||` is two bars and `\|` is left alone.
        assert_eq!(
            expand("\\set{x||y}").unwrap(),
            format!("\\{{{t}x\\mid|y{t}\\}}")
        );
        assert_eq!(
            expand("\\set{x\\|y}").unwrap(),
            format!("\\{{{t}x\\|y{t}\\}}")
        );
        assert_eq!(expand("\\set{}").unwrap(), format!("\\{{{t}{t}\\}}"));
        let bar = format!("{THICK}\\middle\\vert{THICK}");
        assert_eq!(
            expand("\\Set{ x | x<\\frac 12 }").unwrap(),
            format!("\\left\\{{{MEDIUM} x {bar} x<\\frac12 {MEDIUM}\\right\\}}")
        );
        assert_eq!(
            expand("\\Set{x|y|z}").unwrap(),
            format!("\\left\\{{{MEDIUM}x{bar}y|z{MEDIUM}\\right\\}}")
        );
    }

    #[test]
    fn upper_case_set_has_a_double_separator() {
        let (m, k) = (MEDIUM, THICK);
        let double = format!("{k}\\middle\\Vert{k}");
        let want = format!("\\left\\{{{m}x{double}y{m}\\right\\}}");
        assert_eq!(expand("\\Set{x||y}").unwrap(), want);
        assert_eq!(expand("\\Set{x\\|y}").unwrap(), want);
        // The first separator ends the replacement, whichever kind it is.
        assert_eq!(
            expand("\\Set{x\\|y|z}").unwrap(),
            format!("\\left\\{{{m}x{double}y|z{m}\\right\\}}")
        );
        assert_eq!(
            expand("\\Set{x|y\\|z}").unwrap(),
            format!("\\left\\{{{m}x{k}\\middle\\vert{k}y\\|z{m}\\right\\}}")
        );
    }

    #[test]
    fn bra_ket_and_bra_set_take_their_own_parts() {
        assert_eq!(expand("\\bra@ket{L}{M}{D}{R}{a|b||c}").unwrap(), "LaMbDcR");
        assert_eq!(expand("\\bra@ket{L}{M}{}{R}{a||b}").unwrap(), "LaMMbR");
        assert_eq!(expand("\\bra@set{L}{M}{D}{R}{a|b|c}").unwrap(), "LaMb|cR");
        assert_eq!(expand("\\bra@set{L}{M}{D}{R}{a||b|c}").unwrap(), "LaDb|cR");
        // The text is one argument: a single token needs no braces, and the rest follows.
        assert_eq!(expand("\\bra@ket{L}{M}{}{R}x|y").unwrap(), "LxR|y");
        // A `\noexpand` token in the text comes out as itself, as KaTeX's `expandTokens`
        // thaws it: a macro is expanded later, and a bar is not a separator.
        assert_eq!(
            expand("\\def\\foo{X}\\bra@ket{L}{M}{}{R}{a\\noexpand\\foo}").unwrap(),
            "LaXR"
        );
        assert_eq!(
            expand("\\bra@ket{L}{M}{}{R}{a\\noexpand|b}").unwrap(),
            "La|bR"
        );
        // A bar that ends the text, with no right part, looks past the text for a second bar.
        assert_eq!(expand("\\bra@ket{L}{M}{D}{}{a|}|b").unwrap(), "LaDb");
    }

    #[test]
    fn bars_mean_what_they_did_before_afterwards() {
        assert_eq!(
            expand("\\Braket{a}|").unwrap(),
            "\\left\\langlea\\right\\rangle|"
        );
        assert_eq!(expand("\\bra@set{L}{M}{D}{R}{a|b|c}|").unwrap(), "LaMb|cR|");
        assert!(
            expand("\\def|{Z}\\Braket{a}|")
                .unwrap()
                .ends_with("\\right\\rangleZ")
        );
        // `\set` puts back the bar it found, not the plain one.
        assert_eq!(
            expand("\\def|{Z}\\set{a|b|c}|").unwrap(),
            format!("\\{{{THIN}a\\midbZc{THIN}\\}}Z")
        );
    }

    #[test]
    fn nested_forms_each_read_their_own_bars() {
        let bar = format!("{THIN}\\middle\\vert{THIN}");
        assert_eq!(
            expand("\\Braket{a|\\Braket{b|c}|d}").unwrap(),
            format!(
                "\\left\\langlea{bar}\\left\\langleb{bar}c\\right\\rangle{bar}d\\right\\rangle"
            )
        );
        let t = THIN;
        assert_eq!(
            expand("\\set{\\set{a|b}|c}").unwrap(),
            format!("\\{{{t}\\{{{t}a\\midb{t}\\}}\\midc{t}\\}}")
        );
        assert_eq!(
            expand("\\set{a|\\set{b|c}}").unwrap(),
            // The inner one finds no earlier meaning of `|` to restore, and replaces its own.
            format!("\\{{{t}a\\mid\\{{{t}b\\midc{t}\\}}{t}\\}}")
        );
    }

    #[test]
    fn the_parser_reads_the_bracket_macros() {
        use super::super::parser::parse;
        for src in [
            "\\braket{\\phi|\\psi}",
            "\\Braket{ \\phi | \\frac{\\partial^2}{\\partial t^2} | \\psi }",
            "\\set{x|x<5}",
            "\\Set{ x | x<\\frac 12 }",
            "\\Set{x||y}",
            "\\Bra{x}\\Ket{y}",
        ] {
            assert!(parse(src).is_ok(), "{src}: {:?}", parse(src).err());
        }
    }

    #[test]
    fn bra_ket_errors_are_clean() {
        let eof = "Unexpected end of input in a macro argument, expected '}'";
        assert_eq!(error("\\Braket"), eof);
        assert_eq!(error("\\Braket{a|b"), eof);
        assert_eq!(error("\\bra@ket{a}{b}"), eof);
        assert_eq!(error("\\Braket}"), "Extra }");
        assert_eq!(error("\\bra@ket{a}{b}{c}}"), "Extra }");
        // An error inside leaves no group open.
        let src = "\\Braket{a|\\expandafter x\\nosuchmacro}";
        assert!(error(src).starts_with("Undefined control sequence"));
        let mut e = Expander::new(src, Mode::Math).unwrap();
        while e.expand_next_token().is_ok_and(|t| t.kind != Eof) {}
        assert!(e.undo.is_empty());
        assert!(!e.macros.contains_key(&Key::Char("|")));
    }

    #[test]
    fn bra_ket_nesting_and_size_are_bounded() {
        for src in [
            "\\def\\a{\\Braket{\\a}}\\a",
            "\\def\\a{\\set{\\a}}\\a",
            "\\def\\a{\\bra@set{x}{y}{z}{w}{\\a}}\\a",
        ] {
            assert_eq!(
                error(src),
                format!("groups nested more than {} deep", limits::MAX_DEPTH)
            );
        }
        let e = error("\\def\\a{\\Braket{\\a\\a}}\\a");
        assert!(
            e.starts_with("groups nested") || e.starts_with("Too many"),
            "{e}"
        );
        let big = "x".repeat(1000);
        let src = format!(
            "\\def\\a{{{big}{big}{big}{big}}}\\def\\b{{\\a\\a\\a\\a\\a\\a\\a\\a}}\\def\\c{{\\b\\b\\b\\b\\b\\b\\b\\b}}\\def\\d{{\\c\\c\\c\\c\\c\\c\\c\\c}}\\Braket{{\\d}}"
        );
        assert!(error(&src).starts_with("Macro expansion produced more than"));
    }

    #[test]
    fn commands_not_implemented_yet_say_so() {
        assert_eq!(
            error("\\newcommand{\\bra@ket}{}"),
            "\\newcommand{\\bra@ket} attempting to redefine \\bra@ket; use \\renewcommand"
        );
        assert_eq!(
            error("\\newcommand{\\noexpand}{}"),
            "\\newcommand{\\noexpand} attempting to redefine \\noexpand; use \\renewcommand"
        );
    }

    #[test]
    fn char_makes_a_character_command() {
        assert_eq!(expand("\\char65").unwrap(), "\\@char{65}");
        assert_eq!(expand("\\char'101").unwrap(), "\\@char{65}");
        assert_eq!(expand("\\char\"41").unwrap(), "\\@char{65}");
        assert_eq!(expand("\\char`A").unwrap(), "\\@char{65}");
        assert_eq!(expand("\\char`\\%").unwrap(), "\\@char{37}");
        // The digits are read as long as they fit the base.
        assert_eq!(expand("\\char'18").unwrap(), "\\@char{1}8");
        assert_eq!(expand("\\char\"1g").unwrap(), "\\@char{1}g");
        assert_eq!(error("\\char"), "Invalid base-10 digit EOF");
        assert_eq!(error("\\char'a"), "Invalid base-8 digit a");
        assert_eq!(error("\\char\"g"), "Invalid base-16 digit g");
        assert_eq!(error("\\char`"), "\\char` missing argument");
    }

    #[test]
    fn dots_suit_what_follows() {
        // After one expansion of what follows.
        assert_eq!(expand("\\dots,").unwrap(), "\\ldots,");
        assert_eq!(expand("\\dots+").unwrap(), "\\@cdots+");
        assert_eq!(expand("\\dots x").unwrap(), "\\ldotsx");
        assert_eq!(
            expand("\\dots\\int").unwrap(),
            "\\mskip-3mu\\relax\\@cdots\\int"
        );
        assert_eq!(expand("\\dots\\le").unwrap(), "\\@cdots\\le");
        // `\notin` is a macro, so it is `\html@mathml` that follows after its one expansion.
        assert!(
            expand("\\dots\\notin")
                .unwrap()
                .starts_with("\\ldots\\html@mathml")
        );
        assert_eq!(expand("\\dots\\sum").unwrap(), "\\@cdots\\sum");
        // A thin space goes before a closing symbol or punctuation.
        assert_eq!(expand("\\dotso)").unwrap(), "\\ldots\\mskip+3mu\\relax)");
        assert_eq!(expand("\\dotsc,").unwrap(), "\\ldots,");
        assert_eq!(expand("\\dotsc;").unwrap(), "\\ldots\\mskip+3mu\\relax;");
        assert_eq!(expand("\\cdots.").unwrap(), "\\@cdots\\mskip+3mu\\relax.");
        assert_eq!(expand("\\cdots a").unwrap(), "\\@cdotsa");
    }

    #[test]
    fn expandafter_and_noexpand() {
        let defs = "\\def\\a#1{#1#1}\\def\\b{xy}";
        // `\b` is expanded first, so `\a` takes `x`.
        assert_eq!(
            expand(&format!("{defs}\\expandafter\\a\\b")).unwrap(),
            "xxy"
        );
        assert_eq!(expand(&format!("{defs}\\a\\b")).unwrap(), "xyxy");
        assert_eq!(
            error("\\expandafter\\a\\nothing"),
            "Undefined control sequence: \\nothing"
        );
        // A macro that is not expanded reads as `\relax`; anything else is unchanged.
        assert_eq!(expand(&format!("{defs}\\noexpand\\b")).unwrap(), "\\relax");
        assert_eq!(expand("\\noexpand\\nothing").unwrap(), "\\nothing");
        assert_eq!(expand("\\noexpand a").unwrap(), "a");
        // `\def` is not a macro for `\expandafter`, as in KaTeX.
        assert_eq!(expand("\\expandafter\\a\\def\\a{b}").unwrap(), "\\a");
    }

    #[test]
    fn messages_write_nothing() {
        assert_eq!(expand("\\message{hi}a").unwrap(), "a");
        assert_eq!(expand("\\errmessage{hi}a").unwrap(), "a");
        assert_eq!(expand("\\show\\foo a").unwrap(), "a");
    }

    #[test]
    fn arguments() {
        let mut e = Expander::new("  {a{b}c} d \\x}", Mode::Math).unwrap();
        // Spaces before an undelimited argument are skipped, and the braces go.
        let arg = e.consume_arg(None).unwrap();
        let text: String = arg.tokens.iter().map(|t| token_text(&t.kind)).collect();
        assert_eq!(text, "a{b}c");
        assert_eq!(arg.span, 2..9);
        // The next non-blank token is an argument by itself.
        let arg = e.consume_arg(None).unwrap();
        assert_eq!(arg.tokens.len(), 1);
        e.consume_arg(None).unwrap();
        // A closing brace with no opener is an error.
        assert_eq!(e.consume_arg(None).unwrap_err().message, "Extra }");

        let mut e = Expander::new("{a", Mode::Math).unwrap();
        assert_eq!(
            e.consume_arg(None).unwrap_err().message,
            "Unexpected end of input in a macro argument, expected '}'"
        );
        let mut e = Expander::new("ab", Mode::Math).unwrap();
        let stop = Token {
            kind: Char("."),
            span: 0..0,
        };
        assert_eq!(
            e.consume_arg(Some(&[stop])).unwrap_err().message,
            "Unexpected end of input in a macro argument, expected '.'"
        );
    }

    #[test]
    fn scanning_an_argument_marks_its_end() {
        let mut e = Expander::new("[opt]{req}x", Mode::Math).unwrap();
        let optional = e.scan_argument(true).unwrap().unwrap();
        assert_eq!(optional.span, 0..5);
        let mut read = Vec::new();
        loop {
            let token = e.pop_token().unwrap();
            if token.kind == Eof {
                break;
            }
            read.push(token_text(&token.kind));
        }
        assert_eq!(read, ["o", "p", "t"]);
        let required = e.scan_argument(false).unwrap().unwrap();
        assert_eq!(required.tokens.len(), 3);
        // No optional argument: nothing is consumed but spaces.
        let mut e = Expander::new(" x", Mode::Math).unwrap();
        assert!(e.scan_argument(true).unwrap().is_none());
        assert_eq!(e.pop_token().unwrap().kind, Char("x"));
    }

    #[test]
    fn runaway_expansion_is_stopped() {
        let e = error("\\def\\a{\\a}\\a");
        assert!(e.starts_with("Too many expansions"), "{e}");
        let e = error("\\def\\a{\\a\\a}\\a");
        assert!(
            e.starts_with("Too many expansions") || e.starts_with("Macro expansion produced"),
            "{e}"
        );
        let big = "x".repeat(1000);
        let src = format!(
            "\\def\\a{{{big}{big}{big}{big}}}\\def\\b{{\\a\\a\\a\\a\\a\\a\\a\\a}}\\def\\c{{\\b\\b\\b\\b\\b\\b\\b\\b}}\\def\\d{{\\c\\c\\c\\c\\c\\c\\c\\c}}\\d"
        );
        let e = error(&src);
        assert!(
            e.starts_with("Macro expansion produced more than 1048576"),
            "{e}"
        );
    }

    #[test]
    fn a_body_that_repeats_a_long_argument_is_stopped_before_it_is_built() {
        // 2,000 copies of a 2,000 byte argument would be 4 MB; it stops at the limit.
        let src = format!(
            "\\newcommand\\f[1]{{{}}}\\f{{{}}}",
            "#1".repeat(2_000),
            "x".repeat(2_000)
        );
        let e = error(&src);
        assert!(
            e.starts_with("Macro expansion produced more than 1048576"),
            "{e}"
        );
    }

    #[test]
    fn delimiters_are_bounded() {
        let fits = "a".repeat(limits::MAX_DELIMITERS);
        assert_eq!(
            expand(&format!("\\def\\f#1{fits}{{[#1]}}\\f x{fits}")).unwrap(),
            "[x]"
        );
        let long = "a".repeat(limits::MAX_DELIMITERS + 1);
        assert_eq!(
            error(&format!("\\def\\f#1{long}{{}}")),
            "Macro parameter text longer than 256 tokens"
        );
    }

    #[test]
    fn groups_are_bounded() {
        let mut e = Expander::new("", Mode::Math).unwrap();
        for _ in 0..limits::MAX_DEPTH {
            e.begin_group(&(0..0)).unwrap();
        }
        assert_eq!(
            e.begin_group(&(0..0)).unwrap_err().message,
            format!("groups nested more than {} deep", limits::MAX_DEPTH)
        );
    }

    #[test]
    fn every_predefined_macro_lexes_and_the_function_list_is_known() {
        let call = 0..0;
        for (name, _) in macros_data::CONTROLS {
            let key = if name.chars().count() == 1 && !name.chars().all(|c| c.is_ascii_alphabetic())
            {
                Key::Symbol(name.chars().next().unwrap())
            } else {
                Key::Word(name)
            };
            Expander::predefined(&key, &call).unwrap().unwrap();
        }
        for (name, _) in macros_data::CHARS {
            Expander::predefined(&Key::Char(name), &call)
                .unwrap()
                .unwrap();
        }
        // Every function macro of KaTeX is implemented.
        for name in macros_data::FUNCTIONS {
            assert!(Builtin::named(name).is_some(), "{name} is not implemented");
        }
    }

    #[test]
    fn the_predefined_tables_are_sorted() {
        for table in [macros_data::CONTROLS, macros_data::CHARS] {
            assert!(table.windows(2).all(|w| w[0].0 < w[1].0));
        }
        assert_eq!(macros_data::CONTROLS.len(), 277);
        assert_eq!(macros_data::CHARS.len(), 42);
        assert_eq!(macros_data::FUNCTIONS.len(), 20);
    }

    #[test]
    fn counting_arguments() {
        assert_eq!(count_args("abc"), 0);
        assert_eq!(count_args("#1"), 1);
        assert_eq!(count_args("#1 and #2, #1"), 2);
        assert_eq!(count_args("##1"), 0);
        assert_eq!(count_args("#2"), 0);
    }
}
