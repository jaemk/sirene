//! PlantUML regular expressions (PREGEX-*): each `@startregex` line is a regex, drawn as an
//! unnamed railroad rule (MRAIL-2).

use rowan::{TextRange, TextSize};

use super::lines::{self, Common, Cursor, Line};
use crate::diagnostic::Diagnostic;
use crate::lines::MAX_DEPTH;
use crate::railroad::{Expr, Railroad, Rule};
use crate::syntax::SyntaxNode;

/// A problem between two byte offsets of a line.
type Error = (usize, usize, String);
/// A warning between two byte offsets of a line.
type Warning = (usize, usize, &'static str);

/// A regex reader. At the first error it records it and stops, keeping what it read.
struct Reader<'a> {
    s: &'a str,
    i: usize,
    depth: usize,
    error: Option<Error>,
    /// Spans read but not drawn as written: lookarounds and inline flags.
    warnings: Vec<Warning>,
}

/// The drawn name of an escape such as `\d`; `None` for an escaped literal.
fn escape(c: char) -> Option<&'static str> {
    Some(match c {
        'd' => "digit",
        'D' => "not a digit",
        'w' => "word character",
        'W' => "not a word character",
        's' => "whitespace",
        'S' => "not whitespace",
        'b' => "word boundary",
        'B' => "not a word boundary",
        'n' => "new line",
        'r' => "carriage return",
        't' => "tab",
        'A' => "start of input",
        'z' | 'Z' => "end of input",
        _ => return None,
    })
}

fn sequence(mut items: Vec<Expr>) -> Expr {
    if items.len() == 1 {
        items.pop().expect("one item")
    } else {
        Expr::Sequence(items)
    }
}

impl Reader<'_> {
    fn peek(&self) -> Option<char> {
        self.s[self.i..].chars().next()
    }

    fn fail(&mut self, start: usize, end: usize, message: impl Into<String>) {
        if self.error.is_none() {
            self.error = Some((start, end, message.into()));
        }
        self.i = self.s.len();
    }

    /// Alternatives separated by `|`.
    fn choice(&mut self) -> Expr {
        let mut alts = vec![self.seq()];
        while self.peek() == Some('|') {
            self.i += 1;
            alts.push(self.seq());
        }
        if alts.len() == 1 {
            alts.pop().expect("one alternative")
        } else {
            Expr::Choice(alts)
        }
    }

    /// Atoms and their quantifiers up to `|`, `)`, or the end; adjacent literals join.
    fn seq(&mut self) -> Expr {
        let mut items: Vec<Expr> = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let start = self.i;
            let Some(atom) = self.atom() else { break };
            let item = self.quantifier(atom, start);
            match (items.last_mut(), &item) {
                // An empty group, such as `(?i)`, draws nothing.
                (_, Expr::Sequence(v)) if v.is_empty() => {}
                (Some(Expr::Terminal(prev)), Expr::Terminal(t)) => prev.push_str(t),
                _ => items.push(item),
            }
        }
        sequence(items)
    }

    fn atom(&mut self) -> Option<Expr> {
        let start = self.i;
        let c = self.peek()?;
        self.i += c.len_utf8();
        Some(match c {
            '(' => {
                self.depth += 1;
                if self.depth > MAX_DEPTH {
                    self.fail(
                        start,
                        start + 1,
                        format!("groups nest more than {MAX_DEPTH} levels deep"),
                    );
                    return None;
                }
                let rest = &self.s[self.i..];
                if let Some(r) = rest.strip_prefix("?:").or_else(|| rest.strip_prefix("?>")) {
                    // Non-capturing and atomic groups.
                    self.i += rest.len() - r.len();
                } else if let Some(r) = rest.strip_prefix("?<").or_else(|| rest.strip_prefix("?P<"))
                    && !r.starts_with(['=', '!'])
                    && let Some(end) = r.find('>')
                {
                    // A named group.
                    self.i += rest.len() - r.len() + end + 1;
                } else if let Some(r) = rest.strip_prefix('?') {
                    // Inline flags: `(?i)` or `(?i:...)`.
                    let n = r
                        .find(|c: char| !(c.is_ascii_alphabetic() || c == '-'))
                        .unwrap_or(r.len());
                    if n > 0 && r[n..].starts_with([')', ':']) {
                        let colon = usize::from(r[n..].starts_with(':'));
                        self.warnings
                            .push((start, self.i + 1 + n, "inline flags are not drawn"));
                        self.i += 1 + n + colon;
                    }
                }
                let look = ["?=", "?!", "?<=", "?<!"]
                    .into_iter()
                    .find(|p| self.s[self.i..].starts_with(p));
                if let Some(look) = look {
                    self.warnings.push((
                        start,
                        self.i + look.len(),
                        "a lookaround is drawn as a group",
                    ));
                    self.i += look.len();
                }
                let mut inner = self.choice();
                if look.is_some() {
                    // Kept apart from the literals beside it.
                    inner = Expr::Sequence(vec![inner]);
                }
                self.depth -= 1;
                if self.peek() == Some(')') {
                    self.i += 1;
                } else {
                    self.fail(start, start + 1, "the group has no closing `)`");
                }
                inner
            }
            '[' => self.class(start)?,
            '\\' => match self.peek() {
                Some(e) => {
                    self.i += e.len_utf8();
                    match escape(e) {
                        Some(name) => Expr::NonTerminal(name.into()),
                        None => Expr::Terminal(e.to_string()),
                    }
                }
                None => {
                    self.fail(start, start + 1, "`\\` escapes nothing");
                    return None;
                }
            },
            '.' => Expr::NonTerminal("any character".into()),
            '^' => Expr::NonTerminal("start of line".into()),
            '$' => Expr::NonTerminal("end of line".into()),
            '*' | '+' | '?' | '{' if c != '{' || self.bounds(start).is_some() => {
                self.fail(
                    start,
                    self.i,
                    format!("`{c}` follows nothing it can repeat"),
                );
                return None;
            }
            c => Expr::Terminal(c.to_string()),
        })
    }

    /// A class after its `[`: its characters as written, as one box.
    fn class(&mut self, start: usize) -> Option<Expr> {
        let rest = &self.s[self.i..];
        let (negated, body_at) = match rest.strip_prefix('^') {
            Some(_) => (true, 1),
            None => (false, 0),
        };
        let body = &rest[body_at..];
        let mut end = None;
        // Java nests classes, as in `[a[bc]]` and `[[:alpha:]]`.
        let mut depth = 0usize;
        let mut chars = body.char_indices();
        while let Some((k, c)) = chars.next() {
            match c {
                '\\' => {
                    chars.next();
                }
                '[' => depth += 1,
                ']' if depth > 0 => depth -= 1,
                ']' if k > 0 => {
                    end = Some(k);
                    break;
                }
                _ => {}
            }
        }
        let Some(end) = end else {
            self.fail(start, start + 1, "the class has no closing `]`");
            return None;
        };
        self.i += body_at + end + 1;
        let text = &body[..end];
        Some(Expr::NonTerminal(if negated {
            format!("none of {text}")
        } else {
            format!("one of {text}")
        }))
    }

    /// `{n}`, `{n,}`, or `{n,m}` at `at`: its bounds and length.
    fn bounds(&self, at: usize) -> Option<(u32, Option<u32>, usize)> {
        let inner = self.s[at..].strip_prefix('{')?;
        // A repeat contains only digits and an optional comma. Stop at the first other
        // character, so runs of literal `{` do not each scan the remaining line.
        let close = inner
            .bytes()
            .position(|b| !b.is_ascii_digit() && b != b',')?;
        if inner.as_bytes()[close] != b'}' {
            return None;
        }
        let spec = &inner[..close];
        // Digits only: `{ 2 }` and `{+2}` are literal text, as in Java.
        let number = |s: &str| -> Option<u32> {
            s.bytes()
                .all(|b| b.is_ascii_digit())
                .then(|| s.parse().ok())
                .flatten()
        };
        let (min, max) = match spec.split_once(',') {
            Some((a, b)) => (
                number(a)?,
                if b.is_empty() { None } else { Some(number(b)?) },
            ),
            None => {
                let n = number(spec)?;
                (n, Some(n))
            }
        };
        Some((min, max, close + 2))
    }

    fn quantifier(&mut self, atom: Expr, start: usize) -> Expr {
        let (min, max) = match self.peek() {
            Some('*') => (0, None),
            Some('+') => (1, None),
            Some('?') => {
                self.i += 1;
                self.lazy();
                return Expr::Optional(Box::new(atom));
            }
            Some('{') => match self.bounds(self.i) {
                Some((min, max, len)) => {
                    if max.is_some_and(|m| m < min) {
                        let end = self.i + len;
                        self.fail(start, end, "the repeat's maximum is below its minimum");
                        return atom;
                    }
                    self.i += len - 1;
                    (min, max)
                }
                None => return atom,
            },
            _ => return atom,
        };
        self.i += 1;
        self.lazy();
        if (min, max) == (1, Some(1)) {
            return atom;
        }
        Expr::Repeat {
            expr: Box::new(atom),
            min,
            max,
        }
    }

    /// A lazy or possessive quantifier's `?` or `+`, which draws the same.
    fn lazy(&mut self) {
        if matches!(self.peek(), Some('?' | '+')) {
            self.i += 1;
        }
    }
}

/// One regex line: its drawing, the first error, and its warnings.
fn regex(s: &str) -> (Expr, Option<Error>, Vec<Warning>) {
    let mut r = Reader {
        s,
        i: 0,
        depth: 0,
        error: None,
        warnings: Vec::new(),
    };
    let e = r.choice();
    if r.i < s.len() {
        // `choice` stops only at the end or at a `)` with no group open.
        let at = r.i;
        r.fail(at, at + 1, "`)` closes no group");
    }
    (e, r.error, r.warnings)
}

fn range(line: &Line, start: usize, end: usize) -> TextRange {
    let n = |x: usize| TextSize::try_from(x).expect("line fits");
    TextRange::new(line.range.start() + n(start), line.range.start() + n(end))
}

pub(crate) fn lower(root: &SyntaxNode) -> (Railroad, Vec<Diagnostic>) {
    let mut d = Vec::new();
    let mut chart = Railroad::default();
    let mut cursor = Cursor::new(lines::read(root));
    while let Some(line) = cursor.next() {
        // Only `title TEXT` and `skinparam ...` are settings; any other line is a regex, even
        // one starting `set`, `page`, `<style>`, or `!`.
        let word = line.word();
        let setting = matches!(word.as_str(), "title" | "skinparam")
            && line.text[word.len()..].starts_with(char::is_whitespace)
            && !line.text[word.len()..].trim().is_empty();
        if setting {
            match lines::common(&line, &mut cursor, &mut d) {
                Common::No => {}
                Common::Title(t) => {
                    chart.title = Some(t);
                    continue;
                }
                Common::Done | Common::Direction(_) => continue,
            }
        }
        let (definition, error, warnings) = regex(&line.text);
        for (s, e, message) in warnings {
            d.push(Diagnostic::warning(range(&line, s, e), message));
        }
        if let Some((s, e, message)) = error {
            d.push(Diagnostic::new(range(&line, s, e), message));
        }
        chart.rules.push(Rule {
            name: String::new(),
            definition,
        });
    }
    (chart, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Expr {
        Expr::Terminal(s.into())
    }

    fn nt(s: &str) -> Expr {
        Expr::NonTerminal(s.into())
    }

    fn ok(s: &str) -> Expr {
        let (e, err, _) = regex(s);
        assert_eq!(err, None, "{s}");
        e
    }

    fn err(s: &str) -> String {
        regex(s).1.expect("an error").2
    }

    #[test]
    fn literals_join_and_quantifiers_bind() {
        assert_eq!(ok("abc"), t("abc"));
        assert_eq!(
            ok("ab+"),
            Expr::Sequence(vec![
                t("a"),
                Expr::Repeat {
                    expr: Box::new(t("b")),
                    min: 1,
                    max: None
                }
            ])
        );
        assert_eq!(ok("a?"), Expr::Optional(Box::new(t("a"))));
        assert_eq!(
            ok("x{2,3}?"),
            Expr::Repeat {
                expr: Box::new(t("x")),
                min: 2,
                max: Some(3)
            }
        );
        assert_eq!(ok("x{1}"), t("x"));
        assert_eq!(
            ok("x{2,}"),
            Expr::Repeat {
                expr: Box::new(t("x")),
                min: 2,
                max: None
            }
        );
        // A brace that is not a repeat is a literal.
        assert_eq!(ok("a{b"), t("a{b"));
    }

    #[test]
    fn classes_escapes_and_groups() {
        assert_eq!(
            ok("^[a-z]\\d.\\.$"),
            Expr::Sequence(vec![
                nt("start of line"),
                nt("one of a-z"),
                nt("digit"),
                nt("any character"),
                t("."),
                nt("end of line"),
            ])
        );
        assert_eq!(ok("[^]x]"), nt("none of ]x"));
        assert_eq!(ok("[[:alpha:]]"), nt("one of [:alpha:]"));
        assert_eq!(ok("[a[bc]]"), nt("one of a[bc]"));
        assert!(matches!(ok("a{2}"), Expr::Repeat { min: 2, .. }));
        for literal in ["a{ 2 }", "a{2, 3}"] {
            let (e, _, _) = regex(literal);
            assert!(!format!("{e:?}").contains("Repeat"), "{literal}: {e:?}");
        }
        assert_eq!(
            ok("(?:ab|c)*"),
            Expr::Repeat {
                expr: Box::new(Expr::Choice(vec![t("ab"), t("c")])),
                min: 0,
                max: None
            }
        );
        assert_eq!(ok("(?<year>\\d)"), nt("digit"));
        let (_, _, looks) = regex("a(?=b)");
        assert_eq!(looks, [(1, 4, "a lookaround is drawn as a group")]);
        assert_eq!(ok("(?P<y>a)"), t("a"));
        assert_eq!(ok("(?>ab)"), t("ab"));
        let (e, err, w) = regex("(?i)ab(?s:c)");
        assert_eq!(err, None);
        assert_eq!(w.len(), 2);
        assert_eq!(w[0], (0, 3, "inline flags are not drawn"));
        assert_eq!(e, t("abc"));
        assert_eq!(ok("a|"), Expr::Choice(vec![t("a"), Expr::Sequence(vec![])]));
    }

    #[test]
    fn setting_words_are_regexes() {
        let src = "@startregex\ntitle Words\nset|get\npage\\d+\n<style>\ntitle\n@endregex\n";
        let (diagram, d) = super::super::parse(src).lower();
        let crate::diagram::Diagram::Railroad(r) = diagram else {
            panic!("not a railroad")
        };
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(r.title.as_deref(), Some("Words"));
        assert_eq!(r.rules.len(), 4);
    }

    #[test]
    fn literal_braces_and_repeat_bound_prefixes() {
        let literal = "{".repeat(100_000);
        assert_eq!(ok(&literal), t(&literal));
        for literal in ["a{2x}", "a{2,3x}", "a{4294967296}", "a{2,,3}"] {
            assert_eq!(ok(literal), t(literal));
        }
        for (src, min, max) in [
            ("a{00000000000000000000002}", 2, Some(2)),
            ("a{2,}", 2, None),
            ("a{2,3}", 2, Some(3)),
        ] {
            assert_eq!(
                ok(src),
                Expr::Repeat {
                    expr: Box::new(t("a")),
                    min,
                    max
                }
            );
        }
    }

    #[test]
    fn errors() {
        assert_eq!(err("(ab"), "the group has no closing `)`");
        assert_eq!(err("ab)"), "`)` closes no group");
        assert_eq!(err("[ab"), "the class has no closing `]`");
        assert_eq!(err("*a"), "`*` follows nothing it can repeat");
        assert_eq!(err("a\\"), "`\\` escapes nothing");
        assert_eq!(err("a{3,1}"), "the repeat's maximum is below its minimum");
        assert!(err(&"(".repeat(MAX_DEPTH + 1)).contains("nest more than"));
        // What was read before the error is kept.
        assert_eq!(
            regex("ab(c|d").0,
            Expr::Sequence(vec![t("ab"), Expr::Choice(vec![t("c"), t("d")])])
        );
        assert_eq!(regex("ab[c").0, t("ab"));
    }
}
