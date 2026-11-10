//! Line-oriented parser for Mermaid C4 diagrams, producing a lossless rowan tree (MC4-1): each
//! statement is a call such as `Person(a, "A")` or `Rel(a, b, "uses")`; a boundary call is
//! followed by `{`, its statements, and `}`.

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::Base;
use crate::syntax::SyntaxKind::*;

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser(Base::new(src));
    p.start(ROOT);
    p.header(HEADERS, "C4Context", word_len);
    Base::block(&mut p, 0, "boundary", STATEMENT, Parser::statement);
    p.finish();
    p.0.finish_tree()
}

pub(crate) const HEADERS: &[&str] = &[
    "C4Context",
    "C4Container",
    "C4Component",
    "C4Dynamic",
    "C4Deployment",
];

const STATEMENT: &str = "expected an element, boundary, or relation such as `Person(a, \"A\")` or \
                         `Rel(a, b, \"uses\")`, or `title`";

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

impl Parser<'_> {
    fn statement(&mut self, depth: usize) {
        let r = self.rest();
        if keyword(r, "accTitle") || keyword(r, "accDescr") {
            self.acc();
        } else if keyword(r, "title") {
            self.start(TITLE_STMT);
            self.token(TITLE_KW, 5);
            self.ws();
            self.rest_of_line(TEXT);
            self.finish();
        } else if word_len(r) > 0
            && r[word_len(r)..]
                .trim_start_matches([' ', '\t'])
                .starts_with('(')
        {
            self.call(depth);
        }
    }

    /// `Name(arg, $key="value", ...)`, and `{` ... `}` after it for a boundary.
    fn call(&mut self, depth: usize) {
        let ws = scan::ws_len(&self.rest()[word_len(self.rest())..]);
        let after = word_len(self.rest()) + ws;
        let r = self.rest();
        // A `{` after the closing paren, on the same line, opens a boundary.
        let line = &r[..scan::line_len(r)];
        let close = args_end(&line[after..]).map(|n| after + n);
        let braced = close.is_some_and(|c| line[c..].trim_start().starts_with('{'));
        if braced {
            self.start(COMPOSITE);
        }
        self.start(ENTITY);
        let name = word_len(self.rest());
        self.token(IDENT, name);
        self.ws();
        self.token(L_PAREN, 1);
        loop {
            self.ws();
            let r = self.rest();
            if r.starts_with(')') {
                self.token(R_PAREN, 1);
                break;
            }
            if scan::line_len(r) == 0 {
                self.fail("expected `)` to close the arguments");
                self.finish();
                if braced {
                    self.finish();
                }
                return;
            }
            if !self.arg() {
                self.finish();
                if braced {
                    self.finish();
                }
                return;
            }
            self.ws();
            let r = self.rest();
            if r.starts_with(',') {
                self.token(COMMA, 1);
            } else if !r.starts_with(')') {
                self.fail("expected `,` or `)` after an argument");
                self.finish();
                if braced {
                    self.finish();
                }
                return;
            }
        }
        self.finish();
        if braced {
            self.ws();
            self.token(L_BRACE, 1);
            self.end();
            Base::block(self, depth + 1, "boundary", STATEMENT, Parser::statement);
            self.finish();
        } else {
            self.end();
        }
    }

    /// One argument: `"text"`, `$key="text"`, or bare text up to `,` or `)`. Returns whether it
    /// is whole.
    fn arg(&mut self) -> bool {
        self.start(ATTR);
        let r = self.rest();
        if let Some(name) = r.strip_prefix('$') {
            let n = 1 + word_len(name);
            let eq = r[n..].trim_start_matches([' ', '\t']);
            if !eq.starts_with('=') {
                self.fail("expected `=` after the argument's name");
                self.finish();
                return false;
            }
            let len = r.len() - eq.len() + 1;
            self.token(ATTR_KEY, len);
            self.ws();
        }
        let r = self.rest();
        if r.starts_with('"') {
            let Some(n) = self.string_len() else {
                self.fail("unclosed string: expected `\"`");
                self.finish();
                return false;
            };
            self.token(STRING, n);
        } else {
            let line = &r[..scan::line_len(r)];
            let n = line.find([',', ')']).unwrap_or(line.len());
            let n = scan::trim_end_len(line, n);
            self.token(TEXT, n);
        }
        self.finish();
        true
    }
}

/// The length up to and including the `)` closing the arguments that start at `s`, skipping
/// quoted strings.
fn args_end(s: &str) -> Option<usize> {
    let mut quoted = false;
    for (i, c) in s.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ')' if !quoted => return Some(i + 1),
            _ => {}
        }
    }
    None
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
    fn statements() {
        let (t, d) = tree(
            "C4Context\ntitle System Context\nPerson(customer, \"Customer\", \"A customer\")\n\
             Enterprise_Boundary(b0, \"Bank\") {\n  System(sys, \"Banking\", $descr=\"Core (main)\")\n\
             \n  System_Boundary(b1, \"Inner\") {\n    SystemDb(db, Database)\n  }\n}\n\
             Rel(customer, sys, \"Uses\", \"HTTPS\")\nUpdateLayoutConfig($c4ShapeInRow=\"3\")\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("ENTITY@").count(), 7);
        assert_eq!(t.matches("COMPOSITE@").count(), 2);
        assert_eq!(t.matches("ATTR_KEY@").count(), 2);
        assert_eq!(t.matches("ATTR@").count(), 17);
    }

    #[test]
    fn errors() {
        let e = |s: &str| tree(&format!("C4Context\n{s}\n")).1;
        assert_eq!(e("Person(a, \"A"), ["unclosed string: expected `\"`"]);
        assert_eq!(e("Person(a, "), ["expected `)` to close the arguments"]);
        assert_eq!(e("Person(a, b"), ["expected `,` or `)` after an argument"]);
        assert_eq!(
            e("Person(\"a\" \"A\")"),
            ["expected `,` or `)` after an argument"]
        );
        assert_eq!(
            e("Person(a, $descr \"A\")"),
            ["expected `=` after the argument's name"]
        );
        assert_eq!(e("Person(a) x"), ["expected the end of the line"]);
        assert_eq!(
            e("Boundary(b, \"B\") {\nPerson(a)"),
            ["unclosed boundary: expected `}`"]
        );
        assert_eq!(e("}"), ["unexpected `}`"]);
        assert_eq!(e("Person"), [STATEMENT]);
    }
}
