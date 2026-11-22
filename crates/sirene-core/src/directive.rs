//! The lenient JSON inside Mermaid configuration directives (MTHEME-1).

use std::{borrow::Cow, ops::Range};

use crate::lines::MAX_DEPTH;

pub(crate) enum Part<'a> {
    Key(Cow<'a, str>),
    ArrayItem,
}

/// Visits scalars with their enclosing keys and their byte ranges in the directive.
/// Callers commit settings only when the whole body parsed successfully.
pub(crate) fn read(
    directive: &str,
    mut visit: impl FnMut(&[Part<'_>], &str, Range<usize>),
) -> Option<()> {
    let src = directive.strip_prefix("%%{")?.strip_suffix("}%%")?;
    let mut parser = Parser { src, at: 0 };
    parser.object(None, &mut Vec::new(), &mut |path, value, range| {
        visit(path, value, range.start + 3..range.end + 3);
    })
}

pub(crate) fn is_init(key: &str) -> bool {
    matches!(key, "init" | "initialize")
}

/// The first key also identifies an init directive whose later values failed to parse.
pub(crate) fn starts_init(directive: &str) -> bool {
    let Some(src) = directive.strip_prefix("%%{") else {
        return false;
    };
    let mut parser = Parser { src, at: 0 };
    parser.scalar(true).is_some_and(|(key, _)| is_init(&key))
}

struct Parser<'a> {
    src: &'a str,
    at: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<char> {
        self.src[self.at..].chars().next()
    }

    fn skip_space(&mut self) {
        let rest = &self.src[self.at..];
        self.at += rest.len() - rest.trim_start().len();
    }

    fn eat(&mut self, c: char) -> bool {
        self.skip_space();
        if self.peek() == Some(c) {
            self.at += c.len_utf8();
            true
        } else {
            false
        }
    }

    /// Comma-separated items, with an optional trailing comma.
    fn items(
        &mut self,
        end: Option<char>,
        mut item: impl FnMut(&mut Self) -> Option<()>,
    ) -> Option<()> {
        loop {
            self.skip_space();
            if self.peek() == end {
                self.at += end.map_or(0, char::len_utf8);
                return Some(());
            }
            item(self)?;
            if !self.eat(',') {
                self.skip_space();
                if self.peek() != end {
                    return None;
                }
            }
        }
    }

    fn object(
        &mut self,
        end: Option<char>,
        path: &mut Vec<Part<'a>>,
        visit: &mut impl FnMut(&[Part<'a>], &str, Range<usize>),
    ) -> Option<()> {
        self.items(end, |p| {
            let (key, _) = p.scalar(true)?;
            if !p.eat(':') {
                return None;
            }
            path.push(Part::Key(key));
            let result = p.value(path, visit);
            path.pop();
            result
        })
    }

    fn value(
        &mut self,
        path: &mut Vec<Part<'a>>,
        visit: &mut impl FnMut(&[Part<'a>], &str, Range<usize>),
    ) -> Option<()> {
        self.skip_space();
        match self.peek()? {
            c @ ('{' | '[') => {
                if path.len() > MAX_DEPTH {
                    return None;
                }
                self.at += 1;
                if c == '{' {
                    self.object(Some('}'), path, visit)
                } else {
                    path.push(Part::ArrayItem);
                    let result = self.items(Some(']'), |p| p.value(path, visit));
                    path.pop();
                    result
                }
            }
            _ => {
                let (value, range) = self.scalar(false)?;
                visit(path, &value, range);
                Some(())
            }
        }
    }

    /// Single-line quoted text, or a bare word up to a delimiter.
    fn scalar(&mut self, key: bool) -> Option<(Cow<'a, str>, Range<usize>)> {
        self.skip_space();
        let start = self.at;
        if let quote @ ('"' | '\'') = self.peek()? {
            let rest = &self.src[start..];
            let len = crate::lines::quoted_len(rest)?;
            self.at += len;
            let range = start + 1..self.at - 1;
            return Some((decode(&self.src[range.clone()], quote)?, range));
        }
        let rest = &self.src[start..];
        let len = rest
            .find(|c: char| {
                c.is_whitespace()
                    || matches!(c, ',' | '{' | '}' | '[' | ']' | '"' | '\'')
                    || (key && c == ':')
            })
            .unwrap_or(rest.len());
        self.at += len;
        (len > 0).then(|| (Cow::Borrowed(&rest[..len]), start..start + len))
    }
}

fn decode(s: &str, quote: char) -> Option<Cow<'_, str>> {
    let special = |c: char| c == '\\' || c < '\u{20}';
    let Some(start) = s.find(special) else {
        return Some(Cow::Borrowed(s));
    };
    let mut out = String::with_capacity(s.len());
    out.push_str(&s[..start]);
    let mut chars = s[start..].chars();
    while let Some(c) = chars.next() {
        let c = match c {
            '\\' => match chars.next()? {
                c @ ('"' | '\\' | '/') => c,
                '\'' if quote == '\'' => '\'',
                'b' => '\u{8}',
                'f' => '\u{c}',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                'u' => unicode_escape(&mut chars)?,
                _ => return None,
            },
            c if c < '\u{20}' => return None,
            c => c,
        };
        out.push(c);
    }
    Some(Cow::Owned(out))
}

fn unicode_escape(chars: &mut std::str::Chars<'_>) -> Option<char> {
    let hex = |chars: &mut std::str::Chars<'_>| {
        (0..4).try_fold(0, |value, _| Some(value * 16 + chars.next()?.to_digit(16)?))
    };
    let first = hex(chars)?;
    if !(0xd800..=0xdbff).contains(&first) {
        return char::from_u32(first);
    }
    if chars.next()? != '\\' || chars.next()? != 'u' {
        return None;
    }
    let second = hex(chars)?;
    if !(0xdc00..=0xdfff).contains(&second) {
        return None;
    }
    char::from_u32(0x10000 + ((first - 0xd800) << 10) + second - 0xdc00)
}
