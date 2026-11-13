//! Line-oriented parser for Mermaid sequence diagrams, producing a lossless rowan tree (MSEQ-1).

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::Base;
use crate::syntax::SyntaxKind::*;

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser(Base::new(src));
    p.start(ROOT);
    p.header();
    Base::block(&mut p, 0, "block", STATEMENT, Parser::statement);
    p.finish();
    p.0.finish_tree()
}

pub(crate) const HEADERS: &[&str] = &["sequenceDiagram"];

const STATEMENT: &str = "expected a message such as `A->>B: text`, or a statement such as \
                         `participant`, `note`, or `loop`";

/// The statement keywords, matched without regard to case.
const KEYWORDS: &[&str] = &[
    "participant",
    "actor",
    "create",
    "destroy",
    "box",
    "end",
    "activate",
    "deactivate",
    "autonumber",
    "note",
    "loop",
    "alt",
    "else",
    "opt",
    "par",
    "par_over",
    "and",
    "critical",
    "option",
    "break",
    "rect",
    "title",
    "link",
    "links",
    "properties",
    "details",
];

/// The message arrows, longest first where one starts another.
pub(crate) const ARROWS: &[&str] = &[
    "<<-->>", "<<->>", "-->>", "->>", "-->", "->", "--x", "-x", "--)", "-)",
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

/// Length of a word at the start of `s`: letters, digits, `-`, and `_`.
fn word_len(s: &str) -> usize {
    s.find(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_'))
        .unwrap_or(s.len())
}

/// The keyword `s` starts with, as written in [`KEYWORDS`].
pub(crate) fn keyword(s: &str) -> Option<&'static str> {
    let word = &s[..word_len(s)];
    KEYWORDS
        .iter()
        .find(|k| k.eq_ignore_ascii_case(word))
        .copied()
}

/// Where the first message arrow in `s` starts, and its length.
pub(crate) fn find_arrow(s: &str) -> Option<(usize, usize)> {
    s.char_indices().find_map(|(i, _)| {
        ARROWS
            .iter()
            .find(|a| s[i..].starts_with(*a))
            .map(|a| (i, a.len()))
    })
}

/// Length up to the end of the statement: a newline, or a `;` that is not the end of an entity
/// such as `#59;` (Mermaid reads a bare `;` as a newline).
fn line_len(s: &str) -> usize {
    let line = &s[..scan::line_len(s)];
    let mut from = 0;
    while let Some(i) = line[from..].find(';') {
        let at = from + i;
        let before = &line[..at];
        let word = before.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_');
        if !(word.ends_with('#') && word.len() < before.len()) {
            return at;
        }
        from = at + 1;
    }
    line.len()
}

impl Parser<'_> {
    /// The rest of the statement as one `kind` token, without trailing whitespace.
    fn text(&mut self, kind: crate::syntax::SyntaxKind) {
        let r = self.rest();
        let len = scan::trim_end_len(r, line_len(r));
        self.token(kind, len);
    }
}

/// The length of `s` up to ` as ` (any case) or `@{`, without trailing whitespace: a
/// participant's name in its declaration.
fn name_len(s: &str) -> usize {
    let line = &s[..line_len(s)];
    let mut end = line.find("@{").unwrap_or(line.len());
    let lower = line.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find("as") {
        let at = from + i;
        let before = lower[..at].chars().next_back();
        let after = lower[at + 2..].chars().next();
        if at > 0
            && before.is_some_and(char::is_whitespace)
            && after.is_none_or(char::is_whitespace)
        {
            end = end.min(at);
            break;
        }
        from = at + 2;
    }
    scan::trim_end_len(line, end)
}

impl Parser<'_> {
    /// `sequenceDiagram`.
    fn header(&mut self) {
        self.frontmatter();
        self.trivia();
        let r = self.rest();
        if !(r.starts_with("sequenceDiagram") && word_len(r) == "sequenceDiagram".len()) {
            self.fail("expected `sequenceDiagram` header");
            return;
        }
        self.start(HEADER);
        self.token(GRAPH_KW, "sequenceDiagram".len());
        self.end();
        self.finish();
    }

    fn statement(&mut self, _depth: usize) {
        let r = self.rest();
        if r.starts_with("accTitle") || r.starts_with("accDescr") {
            let n = word_len(r);
            if n == "accTitle".len() {
                self.acc();
                return;
            }
        }
        match keyword(r) {
            Some("participant" | "actor" | "create") => self.participant(),
            Some("note") => self.note(),
            Some("title") => self.title(),
            Some(kw) => self.keyword_stmt(kw),
            None => self.message(),
        }
    }

    /// `[create] participant|actor NAME [@{...}] [as ALIAS]`.
    fn participant(&mut self) {
        self.start(PARTICIPANT_STMT);
        if keyword(self.rest()) == Some("create") {
            self.token(SEQ_KW, "create".len());
            self.ws();
            if !matches!(keyword(self.rest()), Some("participant" | "actor")) {
                self.fail("expected `participant` or `actor`");
                self.finish();
                return;
            }
        }
        let n = word_len(self.rest());
        self.token(SEQ_KW, n);
        self.ws();
        let n = name_len(self.rest());
        if n == 0 {
            self.fail("expected a participant name");
            self.finish();
            return;
        }
        self.token(IDENT, n);
        self.ws();
        if self.rest().starts_with("@{") {
            self.token(META_OPEN, 2);
            let r = self.rest();
            match r[..scan::line_len(r)].find('}') {
                // Metadata is JSON and may hold `;`, so it runs to the end of the line.
                Some(i) => {
                    self.token(META_BODY, i);
                    self.token(META_CLOSE, 1);
                    self.ws();
                }
                None => {
                    self.fail("unclosed `@{`: expected `}`");
                    self.finish();
                    return;
                }
            }
        }
        let r = self.rest();
        if r.len() >= 2 && r[..2].eq_ignore_ascii_case("as") && word_len(r) == 2 {
            self.token(AS_KW, 2);
            self.ws();
            self.text(TEXT);
        }
        self.end();
        self.finish();
    }

    /// `note left of A: text`, `note right of A: text`, or `note over A[, B]: text`.
    fn note(&mut self) {
        self.start(NOTE_STMT);
        self.token(NOTE_KW, "note".len());
        self.ws();
        let r = self.rest();
        let side = word_len(r);
        let word = r[..side].to_ascii_lowercase();
        if !matches!(word.as_str(), "left" | "right" | "over") {
            self.fail("expected `left of`, `right of`, or `over`");
            self.finish();
            return;
        }
        self.token(SIDE, side);
        self.ws();
        if word != "over" {
            let r = self.rest();
            if !(r.len() >= 2 && r[..2].eq_ignore_ascii_case("of") && word_len(r) == 2) {
                self.fail("expected `of`");
                self.finish();
                return;
            }
            self.token(OF_KW, 2);
            self.ws();
        }
        self.start(ID_LIST);
        loop {
            let r = self.rest();
            let line = &r[..line_len(r)];
            let stop = line.find([',', ':']).unwrap_or(line.len());
            let n = scan::trim_end_len(line, stop);
            if n == 0 {
                self.finish();
                self.fail("expected a participant name");
                self.finish();
                return;
            }
            self.token(IDENT, n);
            self.ws();
            if !self.rest().starts_with(',') {
                break;
            }
            self.token(COMMA, 1);
            self.ws();
        }
        self.finish();
        if self.rest().starts_with(':') {
            self.token(COLON, 1);
            self.ws();
            self.text(TEXT);
        } else {
            self.fail("expected `:` and the note text");
        }
        self.finish();
    }

    /// `title TEXT` or `title: TEXT`.
    fn title(&mut self) {
        self.start(TITLE_STMT);
        self.token(TITLE_KW, "title".len());
        self.ws();
        if self.rest().starts_with(':') {
            self.token(COLON, 1);
            self.ws();
        }
        self.text(TEXT);
        self.finish();
    }

    /// Any other keyword statement: its keyword, then a participant name or text.
    fn keyword_stmt(&mut self, kw: &str) {
        self.start(SEQ_STMT);
        let n = word_len(self.rest());
        self.token(SEQ_KW, n);
        match kw {
            "end" => self.end(),
            "destroy" | "activate" | "deactivate" => {
                self.ws();
                let r = self.rest();
                let n = scan::trim_end_len(r, line_len(r));
                if n == 0 {
                    self.fail("expected a participant name");
                } else {
                    self.token(IDENT, n);
                }
            }
            _ => {
                self.ws();
                self.text(TEXT);
            }
        }
        self.finish();
    }

    /// `FROM ARROW [+|-] TO [: TEXT]`. Consumes nothing when the line has no arrow.
    fn message(&mut self) {
        let r = self.rest();
        let line = &r[..line_len(r)];
        let head = &line[..line.find(':').unwrap_or(line.len())];
        let Some((at, len)) = find_arrow(head) else {
            return;
        };
        self.start(MESSAGE_STMT);
        let from = scan::trim_end_len(head, at);
        if from == 0 {
            self.fail("expected a participant before the arrow");
            self.finish();
            return;
        }
        self.token(IDENT, from);
        self.ws();
        self.token(ARROW, len);
        self.ws();
        let r = self.rest();
        if r.starts_with('+') || r.starts_with('-') {
            self.token(ACTIVATION, 1);
        }
        self.ws();
        let r = self.rest();
        let line = &r[..line_len(r)];
        let to = scan::trim_end_len(line, line.find(':').unwrap_or(line.len()));
        if to == 0 {
            self.fail("expected a participant after the arrow");
            self.finish();
            return;
        }
        self.token(IDENT, to);
        self.ws();
        if self.rest().starts_with(':') {
            self.token(COLON, 1);
            self.ws();
            self.text(TEXT);
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
    fn headers() {
        assert!(tree("sequenceDiagram\n").1.is_empty());
        assert_eq!(
            tree("sequenceDiagrams\n").1,
            ["expected `sequenceDiagram` header"]
        );
        assert_eq!(
            tree("sequenceDiagram LR\n").1,
            ["expected the end of the line"]
        );
    }

    #[test]
    fn statements() {
        let (t, d) = tree(
            "sequenceDiagram\n  participant A as Alice\n  actor B\n  create participant C@{ \"type\": \"database\" } as Store\n  \
             A->>+B: hi: there\n  B-->>-A: ok\n  Alice Smith -x B\n  Note over A,B: n\n  note left of A: m\n  \
             loop every second\n  A-)B: ping\n  end\n  activate A\n  autonumber 10 5\n  title: T\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("PARTICIPANT_STMT").count(), 3);
        assert_eq!(t.matches("MESSAGE_STMT").count(), 4);
        assert_eq!(t.matches("NOTE_STMT").count(), 2);
        assert_eq!(t.matches("SEQ_STMT").count(), 4);
        assert_eq!(t.matches("ACTIVATION").count(), 2);
        assert!(t.contains("\"Alice Smith\""));
        assert!(t.contains("\"hi: there\""));
    }

    #[test]
    fn errors() {
        let (_, d) = tree(
            "sequenceDiagram\n  participant\n  ->>B: x\n  A->>: y\n  note above A: z\n  note left A: z\n  \
             note over A\n  create note\n  activate\n  participant A@{ x\n  hello\n",
        );
        assert_eq!(
            d,
            [
                "expected a participant name",
                "expected a participant before the arrow",
                "expected a participant after the arrow",
                "expected `left of`, `right of`, or `over`",
                "expected `of`",
                "expected `:` and the note text",
                "expected `participant` or `actor`",
                "expected a participant name",
                "unclosed `@{`: expected `}`",
                STATEMENT,
            ]
        );
    }

    #[test]
    fn words() {
        assert_eq!(keyword("Note over"), Some("note"));
        assert_eq!(keyword("loop->>B"), None);
        assert_eq!(find_arrow("a-b-->>c"), Some((3, 4)));
        assert_eq!(find_arrow("a<<->>b"), Some((1, 5)));
        assert_eq!(find_arrow("a - b"), None);
        assert_eq!(name_len("Alice as A"), 5);
        assert_eq!(name_len("Basil@{ }"), 5);
        assert_eq!(name_len("Casa Blanca"), 11);
        assert_eq!(name_len("x as"), 1);
    }
}
