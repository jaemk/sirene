//! Lexer and recursive-descent parser for Graphviz DOT, producing a lossless rowan tree (DOT-1).

use rowan::{GreenNode, GreenNodeBuilder, TextRange, TextSize};

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::{MAX_DEPTH, too_deep};
use crate::syntax::SyntaxKind::{self, *};

/// One token: its kind and its byte range in the source.
#[derive(Debug, Clone, Copy)]
struct Tok {
    kind: SyntaxKind,
    start: usize,
    end: usize,
}

const KEYWORDS: &[&str] = &["strict", "graph", "digraph", "node", "edge", "subgraph"];

fn is_name_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || !c.is_ascii()
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || !c.is_ascii()
}

/// The length of a numeral such as `-1.5` or `.5` at the start of `s`, or 0.
fn numeral_len(s: &str) -> usize {
    let sign = usize::from(s.starts_with('-'));
    let b = &s.as_bytes()[sign..];
    let int = b.iter().take_while(|c| c.is_ascii_digit()).count();
    let mut n = int;
    if b.get(n) == Some(&b'.') {
        let frac = b[n + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
        if int + frac == 0 {
            return 0;
        }
        n += 1 + frac;
    }
    if n == 0 { 0 } else { sign + n }
}

/// Splits a source into tokens; every byte belongs to exactly one.
struct Lexer<'a> {
    src: &'a str,
    at: usize,
    /// Whether the next token starts a line.
    line_start: bool,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            at: 0,
            line_start: true,
            diagnostics: Vec::new(),
        }
    }
}

impl Iterator for Lexer<'_> {
    type Item = Tok;

    fn next(&mut self) -> Option<Tok> {
        let src = self.src;
        if self.at >= src.len() {
            return None;
        }
        let line_start = self.line_start;
        let r = &src[self.at..];
        let c = r.chars().next().expect("not at the end");
        let (kind, len) = if c == '\n' {
            (NEWLINE, 1)
        } else if r.starts_with("\r\n") {
            (NEWLINE, 2)
        } else if scan::is_space(c) {
            let n = r
                .find(|c: char| !scan::is_space(c) || c == '\n' || c == '\r')
                .unwrap_or(r.len());
            (WHITESPACE, n.max(c.len_utf8()))
        } else if r.starts_with("//") || (c == '#' && line_start) {
            (COMMENT, r.find(['\n', '\r']).unwrap_or(r.len()))
        } else if let Some(body) = r.strip_prefix("/*") {
            match body.find("*/") {
                Some(i) => (COMMENT, i + 4),
                None => {
                    error(
                        &mut self.diagnostics,
                        self.at,
                        src.len(),
                        "unclosed comment: expected `*/`",
                    );
                    (COMMENT, r.len())
                }
            }
        } else if r.starts_with("->") || r.starts_with("--") {
            (ARROW, 2)
        } else if c == '"' {
            (
                STRING,
                string_len(r).unwrap_or_else(|| {
                    error(
                        &mut self.diagnostics,
                        self.at,
                        src.len(),
                        "unclosed string: expected `\"`",
                    );
                    r.len()
                }),
            )
        } else if c == '<' {
            (
                HTML_STRING,
                html_len(r).unwrap_or_else(|| {
                    error(
                        &mut self.diagnostics,
                        self.at,
                        src.len(),
                        "unclosed HTML string: expected `>`",
                    );
                    r.len()
                }),
            )
        } else if let Some(kind) = punct(c) {
            (kind, 1)
        } else if numeral_len(r) > 0 {
            (NUMBER, numeral_len(r))
        } else if is_name_start(c) {
            let n = r.find(|c: char| !is_name_char(c)).unwrap_or(r.len());
            let word = r[..n].to_ascii_lowercase();
            let kind = if KEYWORDS.contains(&word.as_str()) {
                DOT_KW
            } else {
                IDENT
            };
            (kind, n)
        } else {
            let n = c.len_utf8();
            error(
                &mut self.diagnostics,
                self.at,
                self.at + n,
                format!("unexpected `{c}`"),
            );
            (ERROR_TOKEN, n)
        };
        // Graphviz discards a line starting with `#` as C preprocessor output.
        self.line_start = kind == NEWLINE;
        let start = self.at;
        self.at += len;
        Some(Tok {
            kind,
            start,
            end: self.at,
        })
    }
}

fn punct(c: char) -> Option<SyntaxKind> {
    Some(match c {
        '{' => L_BRACE,
        '}' => R_BRACE,
        '[' => L_BRACKET,
        ']' => R_BRACKET,
        ';' => SEMI,
        ',' => COMMA,
        '=' => EQ,
        ':' => COLON,
        '+' => PLUS,
        _ => return None,
    })
}

/// The length of the double-quoted string at the start of `s`, quotes included; `None` when
/// unclosed. `\"` does not close it.
fn string_len(s: &str) -> Option<usize> {
    let mut escaped = false;
    for (i, c) in s.char_indices().skip(1) {
        match c {
            '"' if !escaped => return Some(i + 1),
            '\\' => escaped = !escaped,
            _ => escaped = false,
        }
    }
    None
}

/// The length of the HTML string `<...>` at the start of `s`, with its nested `<` and `>`
/// balanced; `None` when unclosed.
fn html_len(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => {
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

fn offset(n: usize) -> TextSize {
    TextSize::try_from(n).expect("source larger than 4 GiB")
}

fn error(d: &mut Vec<Diagnostic>, start: usize, end: usize, message: impl Into<String>) {
    d.push(Diagnostic::new(
        TextRange::new(offset(start), offset(end)),
        message,
    ));
}

/// Whether `src` starts, after whitespace and comments, with a DOT graph header: an optional
/// `strict`, `graph` or `digraph`, an optional ID, and `{` (LANGS-1).
pub(crate) fn is_dot(src: &str) -> bool {
    // Only the first few tokens are read: this runs on whole documents of any language.
    let mut it = Lexer::new(src)
        .filter(|t| !t.kind.is_trivia())
        .map(|t| (t.kind, src[t.start..t.end].to_ascii_lowercase()));
    let mut next = it.next();
    if next
        .as_ref()
        .is_some_and(|(k, w)| *k == DOT_KW && w == "strict")
    {
        next = it.next();
    }
    if !next
        .as_ref()
        .is_some_and(|(k, w)| *k == DOT_KW && (w == "graph" || w == "digraph"))
    {
        return false;
    }
    next = it.next();
    if next
        .as_ref()
        .is_some_and(|(k, _)| matches!(k, IDENT | NUMBER | STRING | HTML_STRING))
    {
        next = it.next();
    }
    next.is_some_and(|(k, _)| k == L_BRACE)
}

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut lexer = Lexer::new(src);
    let toks: Vec<Tok> = lexer.by_ref().collect();
    let mut p = Parser {
        src,
        toks,
        pos: 0,
        builder: GreenNodeBuilder::new(),
        diagnostics: lexer.diagnostics,
        directed: false,
        depth: 0,
    };
    p.builder.start_node(ROOT.into());
    p.trivia();
    if p.at_end() {
        p.error_here("expected `graph` or `digraph`");
    }
    while !p.at_end() {
        if p.at_kw("strict") || p.at_kw("graph") || p.at_kw("digraph") {
            p.graph();
        } else {
            p.skip_until(&[], "expected `graph` or `digraph`");
        }
        p.trivia();
    }
    p.builder.finish_node();
    (p.builder.finish(), p.diagnostics)
}

struct Parser<'a> {
    src: &'a str,
    toks: Vec<Tok>,
    pos: usize,
    builder: GreenNodeBuilder<'static>,
    diagnostics: Vec<Diagnostic>,
    /// Whether the graph being parsed is a `digraph`.
    directed: bool,
    /// How many subgraphs enclose the statement being parsed.
    depth: usize,
}

impl Parser<'_> {
    fn at_end(&self) -> bool {
        self.pos >= self.toks.len()
    }

    /// The kind of the next token that is not trivia.
    fn peek(&self) -> Option<SyntaxKind> {
        self.toks[self.pos..]
            .iter()
            .find(|t| !t.kind.is_trivia())
            .map(|t| t.kind)
    }

    /// The next token that is not trivia.
    fn peek_tok(&self) -> Option<Tok> {
        self.toks[self.pos..]
            .iter()
            .find(|t| !t.kind.is_trivia())
            .copied()
    }

    fn at(&self, kind: SyntaxKind) -> bool {
        self.peek() == Some(kind)
    }

    fn at_kw(&self, word: &str) -> bool {
        self.peek_tok().is_some_and(|t| {
            t.kind == DOT_KW && self.src[t.start..t.end].eq_ignore_ascii_case(word)
        })
    }

    fn at_id(&self) -> bool {
        matches!(self.peek(), Some(IDENT | NUMBER | STRING | HTML_STRING))
    }

    /// Adds the trivia tokens here to the current node.
    fn trivia(&mut self) {
        while let Some(t) = self.toks.get(self.pos)
            && t.kind.is_trivia()
        {
            self.bump_raw();
        }
    }

    fn bump_raw(&mut self) {
        let t = self.toks[self.pos];
        self.builder.token(t.kind.into(), &self.src[t.start..t.end]);
        self.pos += 1;
    }

    /// Adds the trivia here and then the next token.
    fn bump(&mut self) {
        self.trivia();
        if !self.at_end() {
            self.bump_raw();
        }
    }

    fn start(&mut self, kind: SyntaxKind) {
        self.trivia();
        self.builder.start_node(kind.into());
    }

    fn finish(&mut self) {
        self.builder.finish_node();
    }

    /// An error on the next token, or at the end of the source.
    fn error_here(&mut self, message: &str) {
        let (start, end) = match self.peek_tok() {
            Some(t) => (t.start, t.end),
            None => (self.src.len(), self.src.len()),
        };
        error(&mut self.diagnostics, start, end, message);
    }

    /// Expects `kind`, reporting `message` when it is missing.
    fn expect(&mut self, kind: SyntaxKind, message: &str) -> bool {
        if self.at(kind) {
            self.bump();
            true
        } else {
            self.error_here(message);
            false
        }
    }

    /// Wraps tokens in an `ERROR` node up to one in `stop`, `;`, `}`, or one that can start a
    /// statement, and reports `message` on them. Always moves past at least one token.
    fn skip_until(&mut self, stop: &[SyntaxKind], message: &str) {
        self.trivia();
        if self.at_end() {
            self.error_here(message);
            return;
        }
        let start = self.toks[self.pos].start;
        // The lexer has already reported a character it could not read.
        let reported = self.toks[self.pos].kind == ERROR_TOKEN;
        let mut end = start;
        self.builder.start_node(ERROR.into());
        loop {
            self.bump_raw();
            end = self.toks[self.pos - 1].end.max(end);
            self.trivia_inline();
            match self.toks.get(self.pos) {
                None => break,
                Some(t)
                    if stop.contains(&t.kind)
                        || matches!(t.kind, SEMI | R_BRACE | L_BRACE | DOT_KW) =>
                {
                    break;
                }
                Some(_) => {}
            }
        }
        self.finish();
        if !reported {
            error(&mut self.diagnostics, start, end, message);
        }
    }

    /// Like `trivia`, inside an error node.
    fn trivia_inline(&mut self) {
        while let Some(t) = self.toks.get(self.pos)
            && t.kind.is_trivia()
        {
            self.bump_raw();
        }
    }

    /// `[strict] (graph | digraph) [ID] { stmt_list }`.
    fn graph(&mut self) {
        self.start(DOT_GRAPH);
        if self.at_kw("strict") {
            self.bump();
        }
        self.directed = self.at_kw("digraph");
        if self.at_kw("graph") || self.at_kw("digraph") {
            self.bump();
        } else {
            self.error_here("expected `graph` or `digraph`");
        }
        if self.at_id() {
            self.id();
        }
        if self.expect(L_BRACE, "expected `{`") {
            self.stmt_list();
        }
        self.finish();
    }

    /// Statements up to and including the closing `}`.
    fn stmt_list(&mut self) {
        loop {
            match self.peek() {
                None => {
                    // Once for the innermost block, not for each one around it.
                    let message = "unclosed block: expected `}`";
                    let end = offset(self.src.len());
                    let reported = self
                        .diagnostics
                        .last()
                        .is_some_and(|d| d.message == message && d.range == TextRange::empty(end));
                    if !reported {
                        self.error_here(message);
                    }
                    return;
                }
                Some(R_BRACE) => {
                    self.bump();
                    return;
                }
                Some(SEMI) => self.bump(),
                Some(_) => self.stmt(),
            }
        }
    }

    fn stmt(&mut self) {
        if self.at_kw("graph") || self.at_kw("node") || self.at_kw("edge") {
            self.start(DOT_ATTR_STMT);
            self.bump();
            if self.at(L_BRACKET) {
                self.attr_lists();
            } else {
                self.error_here("expected `[` and attributes");
            }
            self.finish();
        } else if self.at_kw("subgraph") || self.at(L_BRACE) {
            self.operand_stmt();
        } else if self.at_id() {
            let checkpoint = self.checkpoint();
            self.id();
            if self.at(EQ) {
                self.builder.start_node_at(checkpoint, DOT_ASSIGN.into());
                self.bump();
                if self.at_id() {
                    self.id();
                } else {
                    self.error_here("expected a value after `=`");
                }
                self.finish();
                return;
            }
            self.builder.start_node_at(checkpoint, NODE.into());
            self.port();
            self.finish();
            self.after_operand(checkpoint);
        } else {
            self.skip_until(
                &[],
                "expected a node, edge, attribute, or subgraph statement",
            );
        }
    }

    fn checkpoint(&mut self) -> rowan::Checkpoint {
        self.trivia();
        self.builder.checkpoint()
    }

    /// A statement that starts with a subgraph: the subgraph alone, or an edge from it.
    fn operand_stmt(&mut self) {
        let checkpoint = self.checkpoint();
        self.subgraph();
        if self.at(ARROW) {
            self.after_operand(checkpoint);
        }
    }

    /// After a node ID or subgraph at `checkpoint`: an edge statement when an edge operator
    /// follows, otherwise a node statement with its attributes.
    fn after_operand(&mut self, checkpoint: rowan::Checkpoint) {
        if self.at(ARROW) {
            self.builder.start_node_at(checkpoint, DOT_EDGE_STMT.into());
            while self.at(ARROW) {
                self.edge_op();
                if self.at_kw("subgraph") || self.at(L_BRACE) {
                    self.subgraph();
                } else if self.at_id() {
                    self.node_id();
                } else {
                    self.error_here("expected a node or subgraph after the edge operator");
                    break;
                }
            }
        } else {
            self.builder.start_node_at(checkpoint, DOT_NODE_STMT.into());
        }
        if self.at(L_BRACKET) {
            self.attr_lists();
        }
        self.finish();
    }

    /// `->` in a digraph, `--` in a graph.
    fn edge_op(&mut self) {
        let t = self.peek_tok().expect("at an edge operator");
        let text = &self.src[t.start..t.end];
        let (want, other) = if self.directed {
            ("->", "a `digraph`")
        } else {
            ("--", "a `graph`")
        };
        if text != want {
            error(
                &mut self.diagnostics,
                t.start,
                t.end,
                format!("`{text}` in {other}: expected `{want}`"),
            );
        }
        self.bump();
    }

    /// `ID [: port [: compass]]`.
    fn node_id(&mut self) {
        self.start(NODE);
        self.id();
        self.port();
        self.finish();
    }

    /// `: port [: compass]`, if here.
    fn port(&mut self) {
        for _ in 0..2 {
            if !self.at(COLON) {
                return;
            }
            self.bump();
            if self.at_id() {
                self.id();
            } else {
                self.error_here("expected a port name after `:`");
                return;
            }
        }
    }

    /// `[subgraph [ID]] { stmt_list }`.
    fn subgraph(&mut self) {
        if self.depth == MAX_DEPTH {
            self.skip_subgraph();
            return;
        }
        self.depth += 1;
        self.start(SUBGRAPH);
        if self.at_kw("subgraph") {
            self.bump();
            if self.at_id() {
                self.id();
            }
        }
        if self.expect(L_BRACE, "expected `{`") {
            self.stmt_list();
        }
        self.finish();
        self.depth -= 1;
    }

    /// Wraps a subgraph nested too deep in an `ERROR` node, up to its closing `}`, and reports
    /// it on its head.
    fn skip_subgraph(&mut self) {
        self.start(ERROR);
        let start = self.toks.get(self.pos).map_or(self.src.len(), |t| t.start);
        if self.at_kw("subgraph") {
            self.bump();
            if self.at_id() {
                self.id();
            }
        }
        let end = match self.peek_tok() {
            Some(t) if t.kind == L_BRACE => t.end,
            _ => self.toks[self.pos.saturating_sub(1)].end,
        };
        if self.at(L_BRACE) {
            self.trivia();
            let mut open = 0usize;
            while let Some(t) = self.toks.get(self.pos) {
                match t.kind {
                    L_BRACE => open += 1,
                    R_BRACE => open -= 1,
                    _ => {}
                }
                self.bump_raw();
                if open == 0 {
                    break;
                }
            }
        }
        self.finish();
        error(
            &mut self.diagnostics,
            start,
            end.max(start),
            too_deep("subgraphs", MAX_DEPTH),
        );
    }

    /// One or more `[a = b, ...]` lists.
    fn attr_lists(&mut self) {
        while self.at(L_BRACKET) {
            self.start(DOT_ATTR_LIST);
            self.bump();
            loop {
                match self.peek() {
                    None | Some(L_BRACE | R_BRACE | DOT_KW) => {
                        self.error_here("unclosed attribute list: expected `]`");
                        break;
                    }
                    Some(R_BRACKET) => {
                        self.bump();
                        break;
                    }
                    Some(COMMA | SEMI) => self.bump(),
                    Some(IDENT | NUMBER | STRING | HTML_STRING) => self.attr(),
                    Some(_) => self.skip_until(
                        &[R_BRACKET, COMMA],
                        "expected an attribute such as `color=red`",
                    ),
                }
            }
            self.finish();
        }
    }

    /// `key = value`.
    fn attr(&mut self) {
        self.start(ATTR);
        self.id();
        if self.expect(EQ, "expected `=` and a value") {
            if self.at_id() {
                self.id();
            } else {
                self.error_here("expected a value after `=`");
            }
        }
        self.finish();
    }

    /// A name, numeral, HTML string, or quoted string with `+` concatenations.
    fn id(&mut self) {
        self.start(DOT_ID);
        let quoted = self.at(STRING);
        self.bump();
        while quoted && self.at(PLUS) {
            self.bump();
            if !self.expect(STRING, "expected a quoted string after `+`") {
                break;
            }
        }
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
    fn detects_dot() {
        assert!(is_dot("digraph { a }"));
        assert!(is_dot("  // c\n/* x */ strict Graph G {"));
        assert!(is_dot("# line\ngraph \"my g\" {}"));
        assert!(!is_dot("graph TD\n  A --> B"));
        assert!(!is_dot("flowchart LR"));
        assert!(!is_dot("graph"));
        assert!(!is_dot(""));
    }

    #[test]
    fn statements() {
        let (t, d) = tree(
            "strict digraph G {\n  rankdir=LR; node [shape=box, color=\"red\"]\n  \
             a -> b -> {c d} [label=<x<b>y</b>>];\n  subgraph cluster_0 { e:p:n; label=\"A\" + \"B\" }\n  \
             { f } -> g\n}\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("DOT_GRAPH@").count(), 1);
        assert_eq!(t.matches("DOT_ASSIGN@").count(), 2);
        assert_eq!(t.matches("DOT_ATTR_STMT@").count(), 1);
        assert_eq!(t.matches("DOT_EDGE_STMT@").count(), 2);
        assert_eq!(t.matches("DOT_NODE_STMT@").count(), 4);
        assert_eq!(t.matches("SUBGRAPH@").count(), 3);
        assert!(t.contains("HTML_STRING@") && t.contains("PLUS@"));
    }

    #[test]
    fn comments() {
        let (t, d) = tree("# pre\ndigraph { // a\n /* b */ a }\n");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("COMMENT@").count(), 3);
        // Only at the start of a line.
        assert_eq!(tree("digraph {\n  # x\n}").1, ["unexpected `#`"]);
    }

    #[test]
    fn errors() {
        let e = |s: &str| tree(s).1;
        assert_eq!(e(""), ["expected `graph` or `digraph`"]);
        assert_eq!(
            e("digraph { a -- b }"),
            ["`--` in a `digraph`: expected `->`"]
        );
        assert_eq!(e("graph { a -> b }"), ["`->` in a `graph`: expected `--`"]);
        assert_eq!(e("digraph { a"), ["unclosed block: expected `}`"]);
        assert_eq!(e("digraph { a [color] }"), ["expected `=` and a value"]);
        assert_eq!(e("digraph { a [color=] }"), ["expected a value after `=`"]);
        assert_eq!(
            e("digraph { a [b=c }"),
            ["unclosed attribute list: expected `]`"]
        );
        assert_eq!(
            e("digraph { a -> }"),
            ["expected a node or subgraph after the edge operator"]
        );
        assert_eq!(
            e("digraph { \"a }"),
            [
                "unclosed string: expected `\"`",
                "unclosed block: expected `}`"
            ]
        );
        assert_eq!(e("digraph { node }"), ["expected `[` and attributes"]);
        assert_eq!(
            e("digraph { = }"),
            ["expected a node, edge, attribute, or subgraph statement"]
        );
        assert_eq!(e("flowchart LR"), ["expected `graph` or `digraph`"]);
        // Once, however many blocks are open.
        assert_eq!(e("digraph { { { {"), ["unclosed block: expected `}`"]);
        assert_eq!(
            e("digraph { subgraph a { b -> { c"),
            ["unclosed block: expected `}`"]
        );
    }

    #[test]
    fn nesting_limit() {
        let nested = |n: usize| format!("digraph {{ {}a{} }}", "{ ".repeat(n), " }".repeat(n));
        let (t, d) = tree(&nested(MAX_DEPTH));
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("SUBGRAPH@").count(), MAX_DEPTH);

        let (t, d) = tree(&nested(MAX_DEPTH + 3));
        assert_eq!(d, ["subgraphs nested more than 64 deep"]);
        assert_eq!(t.matches("SUBGRAPH@").count(), MAX_DEPTH);
        assert_eq!(t.matches("ERROR@").count(), 1);

        // The statements after a skipped subgraph are still read.
        let src = format!(
            "digraph {{ {}subgraph x {{ a }} b -> {{ c }} d{} e }}",
            "{ ".repeat(MAX_DEPTH),
            " }".repeat(MAX_DEPTH)
        );
        let (t, d) = tree(&src);
        assert_eq!(d.len(), 2, "{d:?}");
        assert_eq!(t.matches("DOT_NODE_STMT@").count(), 2);
        assert_eq!(t.matches("DOT_EDGE_STMT@").count(), 1);

        // Openers alone neither overflow the stack nor report each level.
        let (green, d) = parse(&format!("digraph {}", "{ ".repeat(100_000)));
        let messages: Vec<_> = d.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(
            messages,
            [
                "subgraphs nested more than 64 deep",
                "unclosed block: expected `}`"
            ]
        );
        assert_eq!(usize::from(green.text_len()), "digraph ".len() + 200_000);
    }
}
