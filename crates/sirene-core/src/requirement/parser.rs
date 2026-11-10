//! Line-oriented parser for Mermaid requirement diagrams, producing a lossless rowan tree
//! (MREQ-1).

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::Base;
use crate::syntax::SyntaxKind::*;

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser(Base::new(src));
    p.start(ROOT);
    p.header(HEADERS, "requirementDiagram", word_len);
    Base::block(&mut p, 0, "block", STATEMENT, Parser::statement);
    p.finish();
    p.0.finish_tree()
}

pub(crate) const HEADERS: &[&str] = &["requirementDiagram"];

const STATEMENT: &str = "expected a requirement, element, relationship, or other statement";

/// The requirement types and `element`, lowercase; they match in any case, as in Mermaid.
pub(crate) const KINDS: &[&str] = &[
    "requirement",
    "functionalrequirement",
    "interfacerequirement",
    "performancerequirement",
    "physicalrequirement",
    "designconstraint",
    "element",
];

/// The relationship types, lowercase; they match in any case.
pub(crate) const RELATIONS: &[&str] = &[
    "contains",
    "copies",
    "derives",
    "satisfies",
    "verifies",
    "refines",
    "traces",
];

const BAD_RELATION: &str = "expected a relationship type: `contains`, `copies`, `derives`, \
     `satisfies`, `verifies`, `refines`, or `traces`";

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
    s.find(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_'))
        .unwrap_or(s.len())
}

fn ident_len(s: &str) -> usize {
    s.find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(s.len())
}

fn keyword(s: &str, kw: &str) -> bool {
    s.starts_with(kw) && word_len(s) == kw.len()
}

/// The length of the word at the start of `s` when it is one of `words`, ignoring case.
fn one_of(s: &str, words: &[&str]) -> usize {
    let n = ident_len(s);
    let w = s[..n].to_ascii_lowercase();
    if words.contains(&w.as_str()) { n } else { 0 }
}

/// The length of an unquoted name: from a word character up to one of `:,{<>-=` or the end of
/// the line, without trailing whitespace.
pub(crate) fn name_len(s: &str) -> usize {
    if !s.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
        return 0;
    }
    let line = &s[..scan::line_len(s)];
    let end = line
        .find([':', ',', '{', '<', '>', '-', '=', ';'])
        .unwrap_or(line.len());
    scan::trim_end_len(line, end)
}

impl Parser<'_> {
    fn statement(&mut self, _depth: usize) {
        let r = self.rest();
        let kind = one_of(r, KINDS);
        if kind > 0 && r[kind..].starts_with([' ', '\t']) {
            self.definition(kind);
        } else if keyword(r, "direction") {
            self.direction();
        } else if keyword(r, "classDef") {
            self.class_def(ident_len);
        } else if keyword(r, "class") {
            self.class_stmt(ident_len, "a requirement or element name");
        } else if keyword(r, "style") {
            self.style(ident_len, "a requirement or element name");
        } else if keyword(r, "accTitle") || keyword(r, "accDescr") {
            self.acc();
        } else {
            self.reference_stmt();
        }
    }

    /// A name: a `"string"` or unquoted text. Returns whether one is here.
    fn name(&mut self) -> bool {
        if let Some(n) = self.string_len() {
            self.token(STRING, n);
            return true;
        }
        let n = name_len(self.rest());
        self.token(IDENT, n);
        n > 0
    }

    /// `:::a,b` after a name, if here.
    fn classes(&mut self) {
        if !self.rest().starts_with(":::") {
            return;
        }
        self.start(CLASS_SHORTHAND);
        self.token(TRIPLE_COLON, 3);
        self.id_list(ident_len);
        self.finish();
    }

    /// `requirement NAME[:::classes] { fields }`, or `element ...`.
    fn definition(&mut self, kind: usize) {
        self.start(REQUIREMENT);
        self.token(REQ_KW, kind);
        self.ws();
        if !self.name() {
            self.fail("expected a name");
            self.finish();
            return;
        }
        self.ws();
        self.classes();
        self.ws();
        if !self.rest().starts_with('{') {
            self.fail("expected `{` and the fields");
            self.finish();
            return;
        }
        self.token(L_BRACE, 1);
        self.fields();
        self.finish();
    }

    /// `key: value` lines up to the closing `}`.
    fn fields(&mut self) {
        loop {
            self.trivia();
            let r = self.rest();
            if r.is_empty() {
                let at = self.pos;
                self.error(at, at, "unclosed block: expected `}`");
                return;
            }
            if r.starts_with('}') {
                self.token(R_BRACE, 1);
                self.end();
                return;
            }
            let n = ident_len(r);
            if n == 0 {
                self.fail("expected a field such as `id: 1` or `}`");
                continue;
            }
            self.start(FIELD);
            self.token(ATTR_KEY, n);
            self.ws();
            if self.rest().starts_with(':') {
                self.token(COLON, 1);
                self.ws();
                match self.string_len() {
                    Some(n) => self.token(STRING, n),
                    None => {
                        let line = &self.rest()[..scan::line_len(self.rest())];
                        let end = line.find('}').unwrap_or(line.len());
                        let n = scan::trim_end_len(line, end);
                        self.token(TEXT, n);
                    }
                }
            } else {
                self.fail("expected `:` and a value");
            }
            self.finish();
        }
    }

    /// A relationship (`a - satisfies -> b` or `b <- satisfies - a`), or `name:::classes`.
    fn reference_stmt(&mut self) {
        let checkpoint = self.builder.checkpoint();
        if !self.name() {
            return;
        }
        let ws = scan::ws_len(self.rest());
        let after = &self.rest()[ws..];
        if after.starts_with(":::") {
            self.builder.start_node_at(checkpoint, CLASS_STMT.into());
            self.ws();
            self.classes();
            self.end();
            self.finish();
            return;
        }
        self.builder.start_node_at(checkpoint, RELATION_STMT.into());
        self.ws();
        let (open, close) = if self.rest().starts_with("<-") {
            ("<-", "-")
        } else if self.rest().starts_with('-') {
            ("-", "->")
        } else {
            self.fail("expected `-` or `<-` and a relationship");
            self.finish();
            return;
        };
        self.token(ARROW, open.len());
        self.ws();
        let n = one_of(self.rest(), RELATIONS);
        if n == 0 {
            self.fail(BAD_RELATION);
            self.finish();
            return;
        }
        self.token(RELATION, n);
        self.ws();
        if !self.rest().starts_with(close) || (close == "-" && self.rest().starts_with("->")) {
            self.fail(&format!("expected `{close}`"));
            self.finish();
            return;
        }
        self.token(ARROW, close.len());
        self.ws();
        if !self.name() {
            self.fail("expected a name after the relationship");
            self.finish();
            return;
        }
        self.end();
        self.finish();
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
    fn names() {
        assert_eq!(name_len("test req {"), 8);
        assert_eq!(name_len("a - satisfies -> b"), 1);
        assert_eq!(name_len("b <- x"), 1);
        assert_eq!(name_len("a:::c"), 1);
        assert_eq!(name_len("\"q\""), 0);
        assert_eq!(name_len("-x"), 0);
    }

    #[test]
    fn statements() {
        let (t, d) = tree(
            "requirementDiagram\n  direction LR\n  functionalRequirement test req:::a,b {\n    \
             id: 1.2\n    text: \"the test, text\"\n    Risk: High\n    verifymethod: test\n  }\n  \
             element \"E\" {\n    type: simulation\n  }\n  \"E\" - Satisfies -> test req\n  \
             test req <- traces - E\n  E:::c\n  classDef c fill:#f00\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("REQUIREMENT@").count(), 2);
        assert_eq!(t.matches("FIELD@").count(), 5);
        assert_eq!(t.matches("RELATION_STMT@").count(), 2);
        assert!(t.contains("REQ_KW@") && t.contains("\"functionalRequirement\""));
        assert!(t.contains("RELATION@") && t.contains("\"Satisfies\""));
        assert!(t.contains("IDENT@") && t.contains("\"test req\""));
        assert!(t.contains("CLASS_STMT@") && t.contains("CLASSDEF_STMT@"));
        assert!(t.contains("DIRECTION_STMT@"));
    }

    #[test]
    fn errors() {
        let e = |s: &str| tree(&format!("requirementDiagram\n  {s}\n")).1;
        assert_eq!(e("requirement {"), ["expected a name"]);
        assert_eq!(e("requirement r"), ["expected `{` and the fields"]);
        assert_eq!(
            e("element e {\n type"),
            ["expected `:` and a value", "unclosed block: expected `}`"]
        );
        assert_eq!(
            e("element e {\n - x\n}"),
            ["expected a field such as `id: 1` or `}`"]
        );
        assert_eq!(e("a b"), ["expected `-` or `<-` and a relationship"]);
        assert_eq!(e("a - likes -> b"), [BAD_RELATION]);
        assert_eq!(e("a - traces - b"), ["expected `->`"]);
        assert_eq!(e("a <- traces -> b"), ["expected `-`"]);
        assert_eq!(
            e("a - traces ->"),
            ["expected a name after the relationship"]
        );
        assert_eq!(e("- x"), [STATEMENT]);
    }
}
