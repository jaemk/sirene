//! Line-oriented parser for ZenUML sequence diagrams, producing a lossless rowan tree (MZEN-1):
//! participants, async messages `A->B: text`, sync calls `A.method(args)` and creations
//! `new A()` with an optional `{` ... `}` body, `return`, and fragments such as `if` ... `else`.

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::{Base, is_end};
use crate::syntax::SyntaxKind::{self, *};

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser(Base::new(src));
    p.start(ROOT);
    p.header(HEADERS, "zenuml", word_len);
    Base::block(&mut p, 0, "block", STATEMENT, Parser::statement);
    p.finish();
    p.0.finish_tree()
}

pub(crate) const HEADERS: &[&str] = &["zenuml"];

pub(crate) const STATEMENT: &str = "expected a participant, a message such as `A->B: text` or \
                                    `A.method()`, a fragment such as `if(x) {`, or `title`";

/// Keywords that open a fragment.
pub(crate) const FRAGMENTS: &[&str] = &[
    "if", "while", "for", "forEach", "foreach", "loop", "opt", "par", "try",
];

struct Parser<'a>(Base<'a>);

impl<'a> Deref for Parser<'a> {
    type Target = Base<'a>;
    fn deref(&self) -> &Base<'a> {
        &self.0
    }
}

impl DerefMut for Parser<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

fn word_len(s: &str) -> usize {
    s.find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(s.len())
}

fn keyword(s: &str, kw: &str) -> bool {
    s.starts_with(kw) && word_len(s) == kw.len()
}

/// The length of a participant name at the start of `s`: a word or a `"..."` string on one
/// line; 0 when there is none.
fn name_len(s: &str) -> usize {
    if let Some(body) = s.strip_prefix('"') {
        let line = &body[..scan::line_len(body)];
        return line.find('"').map_or(0, |i| i + 2);
    }
    word_len(s)
}

/// The length of `(` ... `)` at the start of `s`, nested and skipping strings, on one line.
fn parens_len(s: &str) -> Option<usize> {
    let line = &s[..scan::line_len(s)];
    let mut depth = 0;
    let mut quoted = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// The length of the assignee before `=` in `a = A.m()` or `Type a = new A()`, without the
/// `=`; 0 when the line does not assign.
fn assignee_len(line: &str) -> usize {
    let Some(eq) = line.find('=') else {
        return 0;
    };
    if line[eq + 1..].starts_with('=') {
        return 0;
    }
    let before = &line[..eq];
    let ok = before
        .chars()
        .all(|c| c.is_alphanumeric() || " \t_<>[],".contains(c));
    if ok && !before.trim().is_empty() {
        scan::trim_end_len(before, before.len())
    } else {
        0
    }
}

/// What a line that starts with a name is.
#[derive(Debug, PartialEq)]
enum Line {
    Call,
    Async,
    Participant,
    Other,
}

fn classify(line: &str) -> Line {
    let a = assignee_len(line);
    let t = if a > 0 {
        line[a..].trim_start_matches([' ', '\t'])[1..].trim_start()
    } else {
        line
    };
    if keyword(t, "new") {
        return Line::Call;
    }
    let n = name_len(t);
    if n == 0 {
        return Line::Other;
    }
    let u = t[n..].trim_start();
    if let Some(v) = u.strip_prefix("->") {
        let v = v.trim_start();
        let m = name_len(v);
        if m == 0 {
            return Line::Other;
        }
        return if v[m..].trim_start().starts_with('.') {
            Line::Call
        } else if a > 0 {
            Line::Other
        } else {
            Line::Async
        };
    }
    if u.starts_with('.') {
        Line::Call
    } else if a == 0 && (is_end(u) || keyword(u, "as")) {
        Line::Participant
    } else {
        Line::Other
    }
}

impl Parser<'_> {
    fn name_token(&mut self) -> bool {
        let r = self.rest();
        let n = name_len(r);
        if n == 0 {
            return false;
        }
        let kind = if r.starts_with('"') { STRING } else { IDENT };
        self.token(kind, n);
        true
    }

    fn statement(&mut self, depth: usize) {
        let r = self.rest();
        if r.starts_with("//") {
            self.token(COMMENT, scan::line_len(r));
        } else if keyword(r, "title") {
            self.start(TITLE_STMT);
            self.token(TITLE_KW, 5);
            self.ws();
            self.rest_of_line(TEXT);
            self.finish();
        } else if r.starts_with('@') {
            self.annotation();
        } else if let Some(kw) = FRAGMENTS.iter().find(|kw| keyword(r, kw)) {
            self.fragment(kw, depth);
        } else if keyword(r, "return") {
            self.start(SEQ_STMT);
            self.token(ZEN_KW, 6);
            self.ws();
            self.rest_of_line(TEXT);
            self.finish();
        } else {
            match classify(&r[..scan::line_len(r)]) {
                Line::Call => self.call(depth),
                Line::Async => self.message(),
                Line::Participant => self.participant(),
                Line::Other => {}
            }
        }
    }

    /// `@Actor A`, `@Starter(A)`, or `@return` / `@reply` before an async message.
    fn annotation(&mut self) {
        let r = &self.rest()[1..];
        let n = word_len(r);
        let name = &r[..n];
        if name == "return" || name == "reply" {
            self.start(ANNOTATION_STMT);
            self.token(AT, 1);
            self.token(IDENT, n);
            self.end();
            self.finish();
            return;
        }
        self.start(PARTICIPANT_STMT);
        self.token(AT, 1);
        if n == 0 {
            self.fail("expected an annotator such as `@Actor` after `@`");
            self.finish();
            return;
        }
        self.token(IDENT, n);
        if name == "Starter" && self.rest().starts_with('(') {
            self.token(L_PAREN, 1);
            self.ws();
            if !self.name_token() {
                self.fail("expected a participant in `@Starter(...)`");
                self.finish();
                return;
            }
            self.ws();
            if !self.rest().starts_with(')') {
                self.fail("expected `)` after the starter");
                self.finish();
                return;
            }
            self.token(R_PAREN, 1);
            self.end();
            self.finish();
            return;
        }
        self.ws();
        self.participant_body();
        self.finish();
    }

    fn participant(&mut self) {
        self.start(PARTICIPANT_STMT);
        self.participant_body();
        self.finish();
    }

    /// `A` or `A as Label`.
    fn participant_body(&mut self) {
        if !self.name_token() {
            self.fail("expected a participant");
            return;
        }
        self.ws();
        if keyword(self.rest(), "as") {
            self.token(AS_KW, 2);
            self.ws();
            if !self.name_token() {
                self.fail("expected a label after `as`");
                return;
            }
        }
        self.end();
    }

    /// `A->B: text`.
    fn message(&mut self) {
        self.start(MESSAGE_STMT);
        self.name_token();
        self.ws();
        self.token(ARROW, 2);
        self.ws();
        self.name_token();
        self.ws();
        if self.rest().starts_with(':') {
            self.token(COLON, 1);
            self.ws();
            self.rest_of_line(TEXT);
        } else {
            self.end();
        }
        self.finish();
    }

    /// `[Type] a = ` before `[A->]B.method(args)` or `new B(args)`, then an optional body.
    fn call(&mut self, depth: usize) {
        self.start(CALL);
        let a = assignee_len(&self.rest()[..scan::line_len(self.rest())]);
        if a > 0 {
            self.start(LABEL);
            self.token(TEXT, a);
            self.finish();
            self.ws();
            self.token(EQ, 1);
            self.ws();
        }
        if keyword(self.rest(), "new") {
            self.token(ZEN_KW, 3);
            self.ws();
            if !self.name_token() {
                self.fail("expected a participant after `new`");
                self.finish();
                return;
            }
        } else {
            self.name_token();
            self.ws();
            if self.rest().starts_with("->") {
                self.token(ARROW, 2);
                self.ws();
                self.name_token();
                self.ws();
            }
            self.token(DOT, 1);
            self.ws();
            let n = word_len(self.rest());
            if n == 0 {
                self.fail("expected a method name after `.`");
                self.finish();
                return;
            }
            self.token(IDENT, n);
        }
        self.ws();
        if self.rest().starts_with('(') && !self.args() {
            self.finish();
            return;
        }
        self.body(depth);
        self.finish();
    }

    /// `(args)`, kept as one `TEXT`. Returns whether it is closed.
    fn args(&mut self) -> bool {
        let Some(n) = parens_len(self.rest()) else {
            self.fail("expected `)` to close the arguments");
            return false;
        };
        self.token(L_PAREN, 1);
        self.token(TEXT, n - 2);
        self.token(R_PAREN, 1);
        true
    }

    /// The end of the line, or `{`, statements, and `}`.
    fn body(&mut self, depth: usize) {
        self.ws();
        if self.rest().starts_with('{') {
            self.token(L_BRACE, 1);
            self.end();
            Base::block(self, depth + 1, "block", STATEMENT, Parser::statement);
        } else {
            self.end();
        }
    }

    /// `if(c) {` ... `} else if(c) {` ... `} else {` ... `}`, a loop, `opt`, `par`, or
    /// `try` ... `catch` ... `finally`.
    fn fragment(&mut self, kw: &str, depth: usize) {
        self.start(FRAGMENT);
        if !self.branch(&[kw], depth) {
            self.finish();
            return;
        }
        let next: &[&str] = match kw {
            "if" => &["else"],
            "try" => &["catch", "finally"],
            _ => &[],
        };
        loop {
            let r = self.rest();
            let ws = scan::ws_len(r);
            let Some(kw) = next.iter().find(|k| keyword(&r[ws..], k)) else {
                break;
            };
            self.ws();
            let words: &[&str] = if *kw == "else" && keyword(r[ws + 4..].trim_start(), "if") {
                &["else", "if"]
            } else {
                &[kw]
            };
            if !self.branch(words, depth) {
                break;
            }
        }
        if !self.at_eof() {
            self.end();
        }
        self.finish();
    }

    /// One branch: its keywords, an optional `(condition)`, and its body in braces. Returns
    /// whether it closed.
    fn branch(&mut self, words: &[&str], depth: usize) -> bool {
        self.start(BRANCH);
        for (i, w) in words.iter().enumerate() {
            if i > 0 {
                self.ws();
            }
            self.token(ZEN_KW, w.len());
        }
        self.ws();
        if self.rest().starts_with('(') && !self.args() {
            self.finish();
            return false;
        }
        self.ws();
        if !self.rest().starts_with('{') {
            self.fail("expected `{`");
            self.finish();
            return false;
        }
        self.token(L_BRACE, 1);
        self.end();
        let before = self.diagnostics.len();
        Base::block(self, depth + 1, "block", STATEMENT, Parser::statement);
        let closed = !self.diagnostics[before..]
            .iter()
            .any(|d| d.message.starts_with("unclosed block"));
        self.finish();
        closed
    }
}

/// A name's text without its quotes.
pub(crate) fn unquote(kind: SyntaxKind, text: &str) -> &str {
    if kind == STRING {
        text.trim_matches('"')
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::SyntaxNode;

    fn tree(src: &str) -> (String, Vec<String>) {
        let (green, d) = parse(src);
        let root = SyntaxNode::new_root(green);
        assert_eq!(root.to_string(), src);
        (
            format!("{root:#?}"),
            d.into_iter().map(|d| d.message).collect(),
        )
    }

    #[test]
    fn lines() {
        assert_eq!(classify("A.m()"), Line::Call);
        assert_eq!(classify("x = A.m()"), Line::Call);
        assert_eq!(classify("List<A> x = new A()"), Line::Call);
        assert_eq!(classify("C->A.m(1, 2) {"), Line::Call);
        assert_eq!(classify("A->B: hi"), Line::Async);
        assert_eq!(classify("A->B"), Line::Async);
        assert_eq!(classify("\"A B\"->C: hi"), Line::Async);
        assert_eq!(classify("A"), Line::Participant);
        assert_eq!(classify("A as Alice"), Line::Participant);
        assert_eq!(classify("A B"), Line::Other);
        assert_eq!(classify("a == b"), Line::Other);
        assert_eq!(parens_len("(a(b), \")\") x"), Some(11));
        assert_eq!(parens_len("(a"), None);
    }

    #[test]
    fn statements() {
        let (t, d) = tree(
            "zenuml\ntitle Demo\n// a comment\n@Actor Alice\nB as Bob\n@Starter(Alice)\n\
             Alice->Bob: hi\nx = Bob.get(1) {\n  new C()\n  if(a) {\n    return 1\n  } else if(b) {\n  \
             } else {\n  }\n}\ntry {\n} catch(e) {\n} finally {\n}\n@return\nBob->Alice: done\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("PARTICIPANT_STMT@").count(), 3);
        assert_eq!(t.matches("MESSAGE_STMT@").count(), 2);
        assert_eq!(t.matches("CALL@").count(), 2);
        assert_eq!(t.matches("FRAGMENT@").count(), 2);
        assert_eq!(t.matches("BRANCH@").count(), 6);
        assert_eq!(t.matches("ANNOTATION_STMT@").count(), 1);
        assert_eq!(t.matches("SEQ_STMT@").count(), 1);
        assert_eq!(t.matches("COMMENT@").count(), 1);
    }

    #[test]
    fn errors() {
        let e = |s: &str| tree(&format!("zenuml\n{s}\n")).1;
        assert_eq!(e("A.m(1"), ["expected `)` to close the arguments"]);
        assert_eq!(e("A."), ["expected a method name after `.`"]);
        assert_eq!(e("new"), ["expected a participant after `new`"]);
        assert_eq!(e("if(a)"), ["expected `{`"]);
        assert_eq!(e("if(a) {\nA.m()"), ["unclosed block: expected `}`"]);
        assert_eq!(e("A as"), ["expected a label after `as`"]);
        assert_eq!(e("@"), ["expected an annotator such as `@Actor` after `@`"]);
        assert_eq!(e("@Starter(A"), ["expected `)` after the starter"]);
        assert_eq!(e("A.m() x"), ["expected the end of the line"]);
        assert_eq!(e("}"), ["unexpected `}`"]);
        assert_eq!(e("A B"), [STATEMENT]);
    }
}
