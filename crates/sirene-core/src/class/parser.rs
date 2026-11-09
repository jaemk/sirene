//! Line-oriented parser for Mermaid class diagrams, producing a lossless rowan tree (LANGS-2).

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::Base;
use crate::syntax::SyntaxKind::*;

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser(Base::new(src));
    p.start(ROOT);
    p.header(HEADERS, "classDiagram", id_len);
    Base::block(&mut p, 0, "namespace", STATEMENT, Parser::statement);
    p.finish();
    p.0.finish_tree()
}

/// The header keywords, longest first.
pub(crate) const HEADERS: &[&str] = &["classDiagram-v2", "classDiagram"];

const STATEMENT: &str = "expected a class, relation, or other statement";

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

/// Length of a class id at the start of `s`: up to whitespace, punctuation, or a relation's
/// line (`--`, `..`).
pub(crate) fn id_len(s: &str) -> usize {
    let mut end = 0;
    for (i, c) in s.char_indices() {
        if c.is_whitespace()
            || matches!(
                c,
                ':' | '{'
                    | '}'
                    | '"'
                    | '~'
                    | '<'
                    | '>'
                    | '|'
                    | '*'
                    | ','
                    | ';'
                    | '['
                    | ']'
                    | '('
                    | ')'
            )
            || s[i..].starts_with("--")
            || s[i..].starts_with("..")
        {
            break;
        }
        end = i + c.len_utf8();
    }
    end
}

fn keyword(s: &str, kw: &str) -> bool {
    s.starts_with(kw) && id_len(s) == kw.len()
}

/// Length of a relation at the start of `s`: an optional left end (`<|`, `*`, `o`, `<`), a line
/// (`--` or `..`), and an optional right end (`|>`, `*`, `o`, `>`).
pub(crate) fn relation_len(s: &str) -> usize {
    let left = ["<|", "*", "o", "<"]
        .into_iter()
        .find(|l| {
            s.strip_prefix(l)
                .is_some_and(|r| r.starts_with("--") || r.starts_with(".."))
        })
        .map_or(0, str::len);
    let r = &s[left..];
    if !(r.starts_with("--") || r.starts_with("..")) {
        return 0;
    }
    let r = &r[2..];
    let right = ["|>", ">", "*", "o"]
        .into_iter()
        .find(|e| {
            r.strip_prefix(e).is_some_and(|after| {
                after.is_empty() || after.starts_with(char::is_whitespace) || after.starts_with('"')
            })
        })
        .map_or(0, str::len);
    left + 2 + right
}

impl Parser<'_> {
    fn statement(&mut self, depth: usize) {
        let r = self.rest();
        if keyword(r, "direction") {
            self.direction();
        } else if keyword(r, "classDef") {
            self.class_def(id_len);
        } else if keyword(r, "style") {
            self.style(id_len, "a class id");
        } else if keyword(r, "accTitle") || keyword(r, "accDescr") {
            self.acc();
        } else if keyword(r, "click") || keyword(r, "callback") || keyword(r, "link") {
            let n = id_len(r);
            self.start(CLICK_STMT);
            self.token(CLICK_KW, n);
            self.ws();
            self.rest_of_line(TEXT);
            self.finish();
        } else if keyword(r, "cssClass") {
            self.css_class();
        } else if keyword(r, "note") {
            self.note();
        } else if keyword(r, "namespace") {
            self.namespace(depth);
        } else if keyword(r, "class") {
            self.class();
        } else if r.starts_with("<<") {
            self.start(ANNOTATION_STMT);
            self.stereotype();
            self.ws();
            if !self.reference() {
                self.fail("expected a class id");
            } else {
                self.end();
            }
            self.finish();
        } else {
            self.reference_stmt();
        }
    }

    /// `<<interface>>` and the like.
    fn stereotype(&mut self) {
        let r = self.rest();
        let line = scan::line_len(r);
        let len = r[..line].find(">>").map_or(line, |i| i + 2);
        self.token(STEREOTYPE, len);
    }

    /// `cssClass "A,B" name`.
    fn css_class(&mut self) {
        self.start(CSSCLASS_STMT);
        self.token(CSSCLASS_KW, "cssClass".len());
        self.ws();
        match self.string_len() {
            Some(n) => {
                self.token(STRING, n);
                self.ws();
                let n = id_len(self.rest());
                if n > 0 {
                    self.token(IDENT, n);
                    self.end();
                } else {
                    self.fail("expected a class name");
                }
            }
            None => self.fail("expected a quoted list of class ids"),
        }
        self.finish();
    }

    /// `note "text"` or `note for A "text"`.
    fn note(&mut self) {
        self.start(NOTE_STMT);
        self.token(NOTE_KW, 4);
        self.ws();
        if keyword(self.rest(), "for") {
            self.token(FOR_KW, 3);
            self.ws();
            let n = id_len(self.rest());
            if n == 0 {
                self.fail("expected a class id");
                self.finish();
                return;
            }
            self.token(IDENT, n);
            self.ws();
        }
        match self.string_len() {
            Some(n) => {
                self.token(STRING, n);
                self.end();
            }
            None => self.fail("expected a quoted note"),
        }
        self.finish();
    }

    fn namespace(&mut self, depth: usize) {
        self.start(NAMESPACE);
        self.token(NAMESPACE_KW, "namespace".len());
        self.ws();
        let n = id_len(self.rest());
        if n == 0 {
            self.fail("expected a namespace name");
            self.finish();
            return;
        }
        self.token(IDENT, n);
        self.ws();
        if self.rest().starts_with('{') {
            self.token(L_BRACE, 1);
            Base::block(self, depth + 1, "namespace", STATEMENT, Parser::statement);
        } else {
            self.fail("expected `{`");
        }
        self.finish();
    }

    /// `class A`, with `~T~`, `["Label"]`, `:::name`, and a `{ ... }` body.
    fn class(&mut self) {
        self.start(CLASS_DECL);
        self.token(CLASS_KW, 5);
        self.ws();
        if !self.reference() {
            self.fail("expected a class id");
            self.finish();
            return;
        }
        if self.rest().starts_with("[\"") {
            self.token(SHAPE_OPEN, 1);
            if let Some(n) = self.string_len() {
                self.token(STRING, n);
            }
            if self.rest().starts_with(']') {
                self.token(SHAPE_CLOSE, 1);
            } else {
                self.fail("expected `\"]`");
                self.finish();
                return;
            }
        }
        self.ws();
        if self.rest().starts_with('{') {
            self.token(L_BRACE, 1);
            self.body();
        } else {
            self.end();
        }
        self.finish();
    }

    /// Member lines and annotations up to the `}` that closes a class body.
    fn body(&mut self) {
        loop {
            self.trivia();
            let r = self.rest();
            if r.is_empty() {
                let at = self.pos;
                self.error(at, at, "unclosed class body: expected `}`");
                return;
            }
            if r.starts_with('}') {
                self.token(R_BRACE, 1);
                return;
            }
            if r.starts_with("<<") {
                self.stereotype();
                continue;
            }
            // A member runs to the end of the line, or to a `}` that closes the body on it.
            let line = &r[..scan::line_len(r)];
            let len = match line.rfind('}') {
                Some(i) if line[i + 1..].trim().is_empty() && !line[..i].trim().is_empty() => i,
                _ => line.len(),
            };
            let len = scan::trim_end_len(r, len);
            self.token(MEMBER, len);
        }
    }

    /// A class reference: an id, an optional `~T~`, and an optional `:::name`.
    fn reference(&mut self) -> bool {
        let n = id_len(self.rest());
        if n == 0 {
            return false;
        }
        self.start(CLASS_REF);
        self.token(IDENT, n);
        let r = self.rest();
        if let Some(body) = r.strip_prefix('~')
            && let Some(i) = body[..scan::line_len(body)].find('~')
        {
            self.token(GENERIC, i + 2);
        }
        if self.rest().starts_with(":::") {
            self.token(TRIPLE_COLON, 3);
            let n = id_len(self.rest());
            self.token(IDENT, n);
        }
        self.finish();
        true
    }

    /// A statement starting with a class: a relation, a member (`A : +int id`), or a bare
    /// class.
    fn reference_stmt(&mut self) {
        let checkpoint = self.builder.checkpoint();
        if !self.reference() {
            return;
        }
        let n = scan::ws_len(self.rest());
        let after = &self.rest()[n..];
        if after.starts_with('"') || relation_len(after) > 0 {
            self.builder.start_node_at(checkpoint, RELATION_STMT.into());
            self.ws();
            self.cardinality();
            let n = relation_len(self.rest());
            if n == 0 {
                self.fail("expected a relation such as `-->` or `<|--`");
                self.finish();
                return;
            }
            self.token(RELATION, n);
            self.ws();
            self.cardinality();
            if !self.reference() {
                self.fail("expected a class after the relation");
                self.finish();
                return;
            }
            self.ws();
            if self.rest().starts_with(':') {
                self.token(COLON, 1);
                self.ws();
                self.rest_of_line(TEXT);
            } else {
                self.end();
            }
            self.finish();
        } else if after.starts_with(':') {
            self.builder.start_node_at(checkpoint, MEMBER_STMT.into());
            self.ws();
            self.token(COLON, 1);
            self.ws();
            self.rest_of_line(MEMBER);
            self.finish();
        } else {
            self.builder.start_node_at(checkpoint, CLASS_DECL.into());
            self.end();
            self.finish();
        }
    }

    /// An optional quoted cardinality, such as `"1..*"`.
    fn cardinality(&mut self) {
        if let Some(n) = self.string_len().filter(|_| self.rest().starts_with('"')) {
            self.token(STRING, n);
            self.ws();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relations() {
        for (s, n) in [
            ("<|-- B", 4),
            ("--|> B", 4),
            ("*-- B", 3),
            ("o-- B", 3),
            ("--> B", 3),
            ("-- B", 2),
            ("..> B", 3),
            ("..|> B", 4),
            (".. B", 2),
            ("<|--|> B", 6),
            ("--o B", 3),
            ("--*\"1\" B", 3),
            ("- B", 0),
            ("o B", 0),
            ("-->", 3),
        ] {
            assert_eq!(relation_len(s), n, "{s}");
        }
    }

    #[test]
    fn ids() {
        assert_eq!(id_len("Animal <|-- Duck"), 6);
        assert_eq!(id_len("my-class --> B"), 8);
        assert_eq!(id_len("A--B"), 1);
        assert_eq!(id_len("List~T~"), 4);
        assert_eq!(id_len("A:::c"), 1);
        assert_eq!(id_len("a.b"), 3);
    }
}
