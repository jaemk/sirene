//! The lexer: formula source to tokens (LMATH-2, step 1).
//!
//! It reads the token classes KaTeX's lexer reads: a run of whitespace, a control word
//! (`\frac`, with the whitespace after it), a control symbol (`\\`, `\,`, `\ `), a `\verb`
//! form, and a single character with its combining marks. A `%` comment is skipped to the end
//! of its line. Tokens borrow the source and carry their byte range in it.

use std::ops::Range;

use super::{MathError, limits};

/// What a token is. Control words and symbols are typed, so later stages match on a kind and a
/// name instead of comparing text that starts with a backslash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TokenKind<'a> {
    /// A run of space, tab, carriage return, and newline.
    Space,
    /// `\name`: letters and `@`, without the backslash. The span includes the whitespace after
    /// the name, which TeX ignores.
    ControlWord(&'a str),
    /// `\` and one character: `\\`, `\,`, `\{`, and `\ ` (a backslash before whitespace).
    ControlSymbol(char),
    /// One character with any combining marks after it.
    Char(&'a str),
    /// `\verb|text|` and `\verb*|text|`.
    Verb {
        star: bool,
        delimiter: char,
        body: &'a str,
    },
    /// The end of the source.
    Eof,
}

/// A token and the byte range it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Token<'a> {
    pub kind: TokenKind<'a>,
    pub span: Range<usize>,
}

/// Reads tokens one at a time from a source.
pub(crate) struct Lexer<'a> {
    src: &'a str,
    at: usize,
}

fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n')
}

/// The characters KaTeX's lexer accepts as a token of their own: printable ASCII except the
/// backslash, and every other scalar except controls, the line and paragraph separators, and the
/// private use area.
fn is_plain(c: char) -> bool {
    matches!(
        u32::from(c),
        0x21..=0x5B | 0x5D..=0x2027 | 0x202A..=0xD7FF | 0xF900..=0xFFFF | 0x1_0000..
    )
}

fn is_combining_mark(c: char) -> bool {
    matches!(u32::from(c), 0x0300..=0x036F)
}

/// What the `.` of a JavaScript regular expression does not match.
fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

impl<'a> Lexer<'a> {
    /// A lexer over `src`, which must be within `limits::MAX_SOURCE`.
    pub(crate) fn new(src: &'a str) -> Result<Self, MathError> {
        limits::check_source(src)?;
        Ok(Self { src, at: 0 })
    }

    /// The next token, with `Eof` forever at the end of the source. An error leaves the lexer at
    /// the offending character (after any comment before it), so asking again gives the same
    /// error, and a caller that reports it and stops has no further state to reason about.
    pub(crate) fn next_token(&mut self) -> Result<Token<'a>, MathError> {
        loop {
            let start = self.at;
            let Some(c) = self.src[start..].chars().next() else {
                return Ok(Token {
                    kind: TokenKind::Eof,
                    span: start..start,
                });
            };
            let token = match c {
                c if is_space(c) => self.space(start),
                '\\' => self.control(start)?,
                c if is_plain(c) => {
                    let token = self.character(start, c);
                    if matches!(token.kind, TokenKind::Char("%")) {
                        // A comment runs to the end of its line, and the newline goes with it.
                        self.at = match self.src[self.at..].find('\n') {
                            Some(newline) => self.at + newline + 1,
                            None => self.src.len(),
                        };
                        continue;
                    }
                    token
                }
                c => {
                    return Err(MathError::new(
                        format!("Unexpected character: '{c}'"),
                        start..start + c.len_utf8(),
                    ));
                }
            };
            return Ok(token);
        }
    }

    fn space(&mut self, start: usize) -> Token<'a> {
        self.at = self.skip_while(start, is_space);
        Token {
            kind: TokenKind::Space,
            span: start..self.at,
        }
    }

    /// One character `c` at `start` and the combining marks after it.
    fn character(&mut self, start: usize, c: char) -> Token<'a> {
        self.at = self.skip_while(start + c.len_utf8(), is_combining_mark);
        Token {
            kind: TokenKind::Char(&self.src[start..self.at]),
            span: start..self.at,
        }
    }

    /// The offset after the run of characters from `from` that satisfy `keep`.
    fn skip_while(&self, from: usize, keep: impl Fn(char) -> bool) -> usize {
        let rest = &self.src[from..];
        from + rest.find(|c| !keep(c)).unwrap_or(rest.len())
    }

    /// A token that starts with a backslash, at `start`.
    fn control(&mut self, start: usize) -> Result<Token<'a>, MathError> {
        let after = start + 1;
        let rest = &self.src[after..];
        let Some(next) = rest.chars().next() else {
            return Err(unexpected_backslash(start));
        };

        // A backslash before whitespace is a control space; it swallows the whitespace after it.
        if is_space(next) {
            self.at = self.control_space(after);
            return Ok(Token {
                kind: TokenKind::ControlSymbol(' '),
                span: start..self.at,
            });
        }

        if let Some(rest) = rest.strip_prefix("verb")
            && let Some((token, end)) = verb(rest, after + 4)
        {
            self.at = end;
            return Ok(Token {
                kind: token,
                span: start..end,
            });
        }

        if next.is_ascii_alphabetic() || next == '@' {
            let name_end = self.skip_while(after, |c| c.is_ascii_alphabetic() || c == '@');
            let end = self.skip_while(name_end, is_space);
            self.at = end;
            return Ok(Token {
                kind: TokenKind::ControlWord(&self.src[after..name_end]),
                span: start..end,
            });
        }

        // KaTeX's control symbols are limited to the Basic Multilingual Plane.
        if u32::from(next) > 0xFFFF {
            return Err(unexpected_backslash(start));
        }
        self.at = after + next.len_utf8();
        Ok(Token {
            kind: TokenKind::ControlSymbol(next),
            span: start..self.at,
        })
    }

    /// The end of a control space whose whitespace starts at `from`, where the character is
    /// whitespace: a newline, or spaces, tabs, and carriage returns then an optional newline, then
    /// more spaces, tabs, and carriage returns.
    fn control_space(&self, from: usize) -> usize {
        let blank = |c: char| matches!(c, ' ' | '\t' | '\r');
        let run = self.skip_while(from, blank);
        let after_newline = if self.src[run..].starts_with('\n') {
            run + 1
        } else {
            run
        };
        self.skip_while(after_newline, blank)
    }
}

fn unexpected_backslash(start: usize) -> MathError {
    MathError::new("Unexpected character: '\\'", start..start + 1)
}

/// `\verb` forms. `rest` is the source after `\verb`, which starts at `offset`. Returns the token
/// and the offset after its closing delimiter, or `None` when there is no closing delimiter on the
/// line (then the lexer reads `\verb` as a control word, as KaTeX's does).
fn verb(rest: &str, offset: usize) -> Option<(TokenKind<'_>, usize)> {
    let (star, after_star) = match rest.strip_prefix('*') {
        Some(after) => (true, after),
        None => (false, rest),
    };
    let delimiter = after_star.chars().next()?;
    // Unstarred, the delimiter is not `*` and not a letter, or it would be part of a name.
    if !star && (delimiter == '*' || delimiter.is_ascii_alphabetic()) {
        return None;
    }
    let body_start = after_star.strip_prefix(delimiter)?;
    let mut length = 0;
    for c in body_start.chars() {
        if c == delimiter {
            let consumed = usize::from(star) + delimiter.len_utf8() + length + delimiter.len_utf8();
            return Some((
                TokenKind::Verb {
                    star,
                    delimiter,
                    body: &body_start[..length],
                },
                offset + consumed,
            ));
        }
        if is_line_terminator(c) {
            return None;
        }
        length += c.len_utf8();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use TokenKind::*;

    fn kinds(src: &str) -> Vec<TokenKind<'_>> {
        tokens(src).into_iter().map(|t| t.kind).collect()
    }

    fn tokens(src: &str) -> Vec<Token<'_>> {
        let mut lexer = Lexer::new(src).unwrap();
        let mut out = Vec::new();
        loop {
            let token = lexer.next_token().unwrap();
            if token.kind == Eof {
                return out;
            }
            out.push(token);
        }
    }

    fn error(src: &str) -> MathError {
        let mut lexer = Lexer::new(src).unwrap();
        loop {
            match lexer.next_token() {
                Err(e) => return e,
                Ok(t) if t.kind == Eof => panic!("no error in {src:?}"),
                Ok(_) => {}
            }
        }
    }

    #[test]
    fn characters_and_whitespace() {
        assert_eq!(kinds("a+ b"), [Char("a"), Char("+"), Space, Char("b")]);
        assert_eq!(kinds("a \t\r\n b"), [Char("a"), Space, Char("b")]);
        assert_eq!(kinds(""), []);
    }

    #[test]
    fn control_words_take_the_whitespace_after_them() {
        let t = tokens("\\alpha  x");
        assert_eq!(t[0].kind, ControlWord("alpha"));
        assert_eq!(t[0].span, 0..8);
        assert_eq!(t[1].kind, Char("x"));
        // Only letters and `@` make a name.
        assert_eq!(
            kinds("\\a1\\@x\\Gamma_"),
            [
                ControlWord("a"),
                Char("1"),
                ControlWord("@x"),
                ControlWord("Gamma"),
                Char("_")
            ]
        );
    }

    #[test]
    fn control_symbols() {
        assert_eq!(
            kinds("\\\\\\,\\{\\%"),
            [
                ControlSymbol('\\'),
                ControlSymbol(','),
                ControlSymbol('{'),
                ControlSymbol('%')
            ]
        );
        // A symbol takes one character, not the combining mark after it.
        assert_eq!(kinds("\\^\u{301}"), [ControlSymbol('^'), Char("\u{301}")]);
    }

    #[test]
    fn a_backslash_before_whitespace_is_a_control_space() {
        for src in ["\\ x", "\\\nx", "\\ \t\n  x", "\\\r\n x"] {
            let t = tokens(src);
            assert_eq!(t[0].kind, ControlSymbol(' '), "{src:?}");
            assert_eq!(t.last().unwrap().kind, Char("x"), "{src:?}");
            assert_eq!(t.len(), 2, "{src:?}");
        }
        // After the newline, only blanks are swallowed, not another newline.
        assert_eq!(kinds("\\ \n\nx"), [ControlSymbol(' '), Space, Char("x")]);
    }

    #[test]
    fn comments_run_to_the_end_of_the_line() {
        assert_eq!(kinds("a%b\nc"), [Char("a"), Char("c")]);
        assert_eq!(kinds("a%b"), [Char("a")]);
        assert_eq!(kinds("%\n"), []);
        assert_eq!(
            kinds("a \\% b"),
            [Char("a"), Space, ControlSymbol('%'), Space, Char("b")]
        );
    }

    #[test]
    fn combining_marks_stay_with_their_character() {
        let t = tokens("e\u{301}\u{302}x");
        assert_eq!(t[0].kind, Char("e\u{301}\u{302}"));
        assert_eq!(t[0].span, 0..5);
        assert_eq!(t[1].kind, Char("x"));
        // Astral characters are one token.
        assert_eq!(kinds("\u{1d6fc}"), [Char("\u{1d6fc}")]);
    }

    #[test]
    fn verb_forms() {
        let t = tokens("\\verb|a b|x");
        assert_eq!(
            t[0].kind,
            Verb {
                star: false,
                delimiter: '|',
                body: "a b"
            }
        );
        assert_eq!(t[0].span, 0..10);
        assert_eq!(
            kinds("\\verb*!x y!"),
            [Verb {
                star: true,
                delimiter: '!',
                body: "x y"
            }]
        );
        assert_eq!(
            kinds("\\verb||"),
            [Verb {
                star: false,
                delimiter: '|',
                body: ""
            }]
        );
    }

    #[test]
    fn an_unclosed_verb_is_a_control_word() {
        assert_eq!(
            kinds("\\verb|ab"),
            [ControlWord("verb"), Char("|"), Char("a"), Char("b")]
        );
        // The body does not cross a line.
        assert_eq!(
            kinds("\\verb|a\nb|"),
            [
                ControlWord("verb"),
                Char("|"),
                Char("a"),
                Space,
                Char("b"),
                Char("|")
            ]
        );
        // A letter cannot delimit an unstarred `\verb`: it reads as a longer name.
        assert_eq!(kinds("\\verbatim"), [ControlWord("verbatim")]);
    }

    #[test]
    fn unexpected_characters() {
        let e = error("a\u{1}b");
        assert_eq!(e.message, "Unexpected character: '\u{1}'");
        assert_eq!(e.span, 1..2);
        assert_eq!(error("\u{e000}").span, 0..3);
        assert_eq!(
            error("x\u{2028}").message,
            "Unexpected character: '\u{2028}'"
        );
        // A lone backslash, and a backslash before a character outside the BMP.
        assert_eq!(error("x\\").span, 1..2);
        assert_eq!(error("\\\u{1d6fc}").span, 0..1);
    }

    #[test]
    fn eof_repeats() {
        let mut lexer = Lexer::new("a").unwrap();
        assert_eq!(lexer.next_token().unwrap().kind, Char("a"));
        for _ in 0..3 {
            let token = lexer.next_token().unwrap();
            assert_eq!(token.kind, Eof);
            assert_eq!(token.span, 1..1);
        }
    }

    #[test]
    fn a_source_past_the_limit_is_refused() {
        let src = "x".repeat(limits::MAX_SOURCE + 1);
        assert!(Lexer::new(&src).is_err());
    }

    mod props {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// Whatever the input, the lexer returns tokens or an error and never panics, its
            /// spans are in order, non-empty (but for `Eof`), on character boundaries, and
            /// inside the source, and `Eof` ends at the end of the source.
            #[test]
            fn spans_are_ordered_and_valid(src in "[ -~\\n\\t\\r\\\\%\u{300}-\u{302}\u{e9}\u{1d6fc}]{0,80}") {
                let mut lexer = Lexer::new(&src).unwrap();
                let mut end = 0;
                while let Ok(token) = lexer.next_token() {
                    prop_assert!(token.span.start >= end, "{:?}", token);
                    prop_assert!(token.span.end >= token.span.start);
                    prop_assert!(src.get(token.span.clone()).is_some(), "{:?}", token);
                    end = token.span.end;
                    if token.kind == Eof {
                        prop_assert_eq!(end, src.len());
                        break;
                    }
                    prop_assert!(token.span.end > token.span.start, "{:?}", token);
                }
            }
        }
    }
}
