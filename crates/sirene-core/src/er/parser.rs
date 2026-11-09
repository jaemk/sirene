//! Line-oriented parser for Mermaid entity relationship diagrams, producing a lossless rowan
//! tree (LANGS-2).

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::Base;
use crate::syntax::SyntaxKind::*;

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser(Base::new(src));
    p.start(ROOT);
    p.header(HEADERS, "erDiagram", id_len);
    Base::block(&mut p, 0, "block", STATEMENT, Parser::statement);
    p.finish();
    p.0.finish_tree()
}

pub(crate) const HEADERS: &[&str] = &["erDiagram"];

const STATEMENT: &str = "expected an entity, relationship, or other statement";

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

/// Length of an entity name at the start of `s`: up to whitespace, punctuation, or a
/// relationship's line.
pub(crate) fn id_len(s: &str) -> usize {
    let mut end = 0;
    for (i, c) in s.char_indices() {
        if c.is_whitespace()
            || matches!(c, ':' | '{' | '}' | '[' | ']' | '"' | '|' | ',' | ';')
            || ["--", "..", ".-", "-."]
                .iter()
                .any(|l| s[i..].starts_with(l))
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

/// The cardinality words, longest first, as `(words, symbol)` with the symbol of the left end.
pub(crate) const WORDS: &[(&str, &str)] = &[
    ("zero or one", "|o"),
    ("one or zero", "|o"),
    ("zero or more", "}o"),
    ("zero or many", "}o"),
    ("one or more", "}|"),
    ("one or many", "}|"),
    ("only one", "||"),
    ("many(0)", "}o"),
    ("many(1)", "}|"),
    ("many", "}o"),
    ("one", "||"),
    ("0+", "}o"),
    ("1+", "}|"),
    ("1", "||"),
];

/// Length of the cardinality words at the start of `s`, with whole-word matching.
fn words_len(s: &str) -> usize {
    WORDS
        .iter()
        .find(|(w, _)| {
            s.starts_with(w)
                && !s[w.len()..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_')
        })
        .map_or(0, |(w, _)| w.len())
}

/// Length of a relationship at the start of `s`: symbols such as `||--o{`, or words such as
/// `one or more to zero or many` (`optionally to` for a non-identifying one).
pub(crate) fn relation_len(s: &str) -> usize {
    let left = ["|o", "||", "}o", "}|"];
    let line = ["--", "..", ".-", "-."];
    let right = ["o|", "||", "o{", "|{"];
    if s.len() >= 6
        && s.is_char_boundary(6)
        && left.contains(&&s[..2])
        && line.contains(&&s[2..4])
        && right.contains(&&s[4..6])
    {
        return 6;
    }
    let a = words_len(s);
    if a == 0 {
        return 0;
    }
    let mut at = a + scan::ws_len(&s[a..]);
    let connector = ["optionally to", "to"].into_iter().find(|c| {
        keyword(&s[at..], c) || s[at..].starts_with(c) && s[at + c.len()..].starts_with(' ')
    });
    let Some(c) = connector else {
        return 0;
    };
    at += c.len();
    at += scan::ws_len(&s[at..]);
    let b = words_len(&s[at..]);
    if b == 0 { 0 } else { at + b }
}

impl Parser<'_> {
    fn statement(&mut self, _depth: usize) {
        let r = self.rest();
        if keyword(r, "direction") {
            self.direction();
        } else if keyword(r, "classDef") {
            self.class_def(id_len);
        } else if keyword(r, "class") {
            self.class_stmt(id_len, "an entity name");
        } else if keyword(r, "style") {
            self.style(id_len, "an entity name");
        } else if keyword(r, "accTitle") || keyword(r, "accDescr") {
            self.acc();
        } else {
            self.entity_stmt();
        }
    }

    /// An entity: a name or `"quoted name"`, an optional `[alias]`, and an optional `:::name`.
    fn reference(&mut self) -> bool {
        let r = self.rest();
        let (kind, n) = if r.starts_with('"') {
            match self.string_len() {
                Some(n) => (STRING, n),
                None => return false,
            }
        } else {
            (IDENT, id_len(r))
        };
        if n == 0 {
            return false;
        }
        self.start(ENTITY_REF);
        self.token(kind, n);
        if self.rest().starts_with('[') {
            self.token(SHAPE_OPEN, 1);
            let r = self.rest();
            let (kind, n) = if r.starts_with('"') {
                (STRING, self.string_len().unwrap_or(0))
            } else {
                (IDENT, id_len(r))
            };
            self.token(kind, n);
            if self.rest().starts_with(']') {
                self.token(SHAPE_CLOSE, 1);
            }
        }
        if self.rest().starts_with(":::") {
            self.token(TRIPLE_COLON, 3);
            let n = id_len(self.rest());
            self.token(IDENT, n);
        }
        self.finish();
        true
    }

    /// A relationship, an entity with an attribute block, or a bare entity.
    fn entity_stmt(&mut self) {
        let checkpoint = self.builder.checkpoint();
        if !self.reference() {
            return;
        }
        let n = scan::ws_len(self.rest());
        let after = &self.rest()[n..];
        if relation_len(after) > 0 {
            self.builder.start_node_at(checkpoint, RELATION_STMT.into());
            self.ws();
            let n = relation_len(self.rest());
            self.token(RELATION, n);
            self.ws();
            if !self.reference() {
                self.fail("expected an entity after the relationship");
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
        } else if after.starts_with('{') {
            self.builder.start_node_at(checkpoint, ENTITY.into());
            self.ws();
            self.token(L_BRACE, 1);
            self.attributes();
            self.finish();
        } else if after.starts_with(['|', '}', '-', '.']) {
            self.builder.start_node_at(checkpoint, RELATION_STMT.into());
            self.fail("expected a relationship such as `||--o{`");
            self.finish();
        } else {
            self.builder.start_node_at(checkpoint, ENTITY.into());
            self.end();
            self.finish();
        }
    }

    /// Attributes, each `type name [PK, FK] ["comment"]`, up to the closing `}`.
    fn attributes(&mut self) {
        loop {
            self.trivia();
            let r = self.rest();
            if r.is_empty() {
                let at = self.pos;
                self.error(at, at, "unclosed attribute block: expected `}`");
                return;
            }
            if r.starts_with('}') {
                self.token(R_BRACE, 1);
                return;
            }
            self.start(ATTRIBUTE);
            let word = |s: &str| {
                s.find(|c: char| c.is_whitespace() || c == '}')
                    .unwrap_or(s.len())
            };
            let n = word(r);
            self.token(IDENT, n);
            self.ws();
            let n = word(self.rest());
            if n == 0 || self.rest().starts_with('"') {
                self.fail("expected an attribute name after its type");
                self.finish();
                continue;
            }
            self.token(IDENT, n);
            self.ws();
            let keys = keys_len(self.rest());
            if keys > 0 {
                self.token(KEYS, keys);
                self.ws();
            }
            if self.rest().starts_with('"') {
                match self.string_len() {
                    Some(n) => self.token(STRING, n),
                    None => self.fail("unclosed comment: expected `\"`"),
                }
            }
            self.finish();
        }
    }
}

/// Length of a key list such as `PK, FK` at the start of `s`.
fn keys_len(s: &str) -> usize {
    let mut at = 0;
    let mut end = 0;
    loop {
        let r = &s[at..];
        let Some(k) = ["PK", "FK", "UK"]
            .into_iter()
            .find(|k| r.starts_with(k) && !r[2..].starts_with(|c: char| c.is_alphanumeric()))
        else {
            return end;
        };
        at += k.len();
        end = at;
        let r = &s[at..];
        let ws = scan::ws_len(r);
        if !r[ws..].starts_with(',') {
            return end;
        }
        at += ws + 1;
        at += scan::ws_len(&s[at..]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relations() {
        for (s, n) in [
            ("||--o{ B", 6),
            ("}|..|{ B", 6),
            ("|o.-o| B", 6),
            ("||-- B", 0),
            ("one or more to zero or many B", 27),
            ("1 to many(1) B", 12),
            ("only one optionally to one B", 26),
            ("zero or one to B", 0),
            ("onex to one", 0),
            ("many to manyx", 0),
        ] {
            assert_eq!(relation_len(s), n, "{s}");
        }
    }

    #[test]
    fn keys() {
        assert_eq!(keys_len("PK"), 2);
        assert_eq!(keys_len("PK, FK \"c\""), 6);
        assert_eq!(keys_len("PK,FK,UK"), 8);
        assert_eq!(keys_len("PKX"), 0);
        assert_eq!(keys_len("\"c\""), 0);
        assert_eq!(keys_len("FK ,"), 2);
    }

    #[test]
    fn ids() {
        assert_eq!(id_len("LINE-ITEM ||--o{ X"), 9);
        assert_eq!(id_len("A||--o{B"), 1);
        assert_eq!(id_len("a.b c"), 3);
    }
}
