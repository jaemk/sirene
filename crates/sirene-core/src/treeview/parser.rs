//! Line-oriented parser for Mermaid tree views, producing a lossless rowan tree (MTREEV-1).

use std::ops::{Deref, DerefMut};

use rowan::GreenNode;

use crate::diagnostic::Diagnostic;
use crate::flowchart::scan;
use crate::lines::Base;
use crate::syntax::SyntaxKind::*;

pub(crate) const HEADERS: &[&str] = &["treeView-beta"];

/// Box-drawing characters that mark the tree's structure.
const BOX: &[char] = &['─', '━', '│', '┃', '└', '┗', '├', '┣'];
/// The characters that start a branch: `├`, `└`, and their heavy forms.
const BRANCH: &[char] = &['└', '┗', '├', '┣'];

pub(crate) fn parse(src: &str) -> (GreenNode, Vec<Diagnostic>) {
    let mut p = Parser {
        base: Base::new(src),
        boxed: false,
    };
    p.start(ROOT);
    p.header(HEADERS, "treeView-beta", header_len);
    if !p.diagnostics.is_empty() {
        // Without the header, the rest is not read.
        let len = p.rest().len();
        p.start(ERROR);
        p.token(ERROR_TOKEN, len);
        p.finish();
    }
    p.boxed = boxed(p.rest());
    loop {
        p.trivia();
        if p.at_eof() {
            break;
        }
        p.line();
    }
    p.finish();
    p.base.finish_tree()
}

fn header_len(s: &str) -> usize {
    s.find(|c: char| c.is_whitespace()).unwrap_or(s.len())
}

/// Whether the lines in `body` draw the tree with box-drawing characters, as Mermaid detects
/// it: any line other than a comment, `title`, `accTitle`, `accDescr`, or one holding only `│`
/// and whitespace has one.
pub(crate) fn boxed(body: &str) -> bool {
    body.lines().any(|line| {
        let t = line.trim();
        !(t.is_empty()
            || t.starts_with("%%")
            || metadata(t)
            || line
                .chars()
                .all(|c| c.is_whitespace() || c == '│' || c == '┃'))
            && line.contains(BOX)
    })
}

/// Whether trimmed line `t` is a `title`, `accTitle`, or `accDescr` statement.
fn metadata(t: &str) -> bool {
    let after = |kw: &str| t.strip_prefix(kw).map(str::trim_start);
    t.strip_prefix("title")
        .is_some_and(|r| r.starts_with([' ', '\t']))
        || after("accTitle").is_some_and(|r| r.starts_with(':'))
        || after("accDescr").is_some_and(|r| r.starts_with([':', '{']))
}

/// The length of a bare name: up to the end of the line or an annotation (` :::class`,
/// ` icon(`, or ` ##`), without trailing whitespace.
fn bare_len(s: &str) -> usize {
    let line = &s[..scan::line_len(s)];
    let mut end = line.len();
    for (i, c) in line.char_indices() {
        if (c == ' ' || c == '\t') && i > 0 && stops(&line[i..]) {
            end = i;
            break;
        }
    }
    scan::trim_end_len(s, end)
}

/// Whether whitespace at `s` starts an annotation, as Mermaid's bare names stop for one even
/// when it does not parse.
fn stops(s: &str) -> bool {
    let r = &s[scan::ws_len(s)..];
    if let Some(body) = r.strip_prefix(":::") {
        let body = &body[scan::ws_len(body)..];
        return body.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_');
    }
    r.starts_with("icon(") || r.starts_with("##")
}

/// An annotation at `s`, after its leading whitespace: its kind and its length.
fn annotation_len(s: &str) -> Option<(crate::syntax::SyntaxKind, usize)> {
    let ws = scan::ws_len(s);
    if ws == 0 {
        return None;
    }
    let r = &s[ws..];
    let line = &r[..scan::line_len(r)];
    if let Some(body) = line.strip_prefix(":::") {
        let lead = scan::ws_len(body);
        let name = &body[lead..];
        let mut chars = name.char_indices();
        match chars.next() {
            Some((_, c)) if c.is_ascii_alphabetic() || c == '_' => {}
            _ => return None,
        }
        let n = chars
            .find(|&(_, c)| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
            .map_or(name.len(), |(i, _)| i);
        return Some((CLASS_SHORTHAND, 3 + lead + n));
    }
    if let Some(body) = line.strip_prefix("icon(") {
        let word = |s: &str| {
            s.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
                .unwrap_or(s.len())
        };
        let mut n = word(body);
        if body[n..].starts_with(':') {
            let m = word(&body[n + 1..]);
            if m == 0 {
                return None;
            }
            n += 1 + m;
        }
        return body[n..].starts_with(')').then_some((TREE_ICON, 5 + n + 1));
    }
    if line.starts_with("##") {
        return Some((TREE_DESC, scan::trim_end_len(r, line.len())));
    }
    None
}

struct Parser<'a> {
    base: Base<'a>,
    /// Whether the tree is drawn with box-drawing characters.
    boxed: bool,
}

impl<'a> Deref for Parser<'a> {
    type Target = Base<'a>;
    fn deref(&self) -> &Base<'a> {
        &self.base
    }
}

impl DerefMut for Parser<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl Parser<'_> {
    fn line(&mut self) {
        let r = self.rest();
        let line = &r[..scan::line_len(r)];
        if metadata(line.trim_end()) {
            if line.starts_with("title") {
                self.title();
            } else {
                self.acc();
            }
            return;
        }
        if self.boxed {
            if let Some(i) = line.find(BRANCH) {
                // The prefix runs to the branch character, its dashes, and the spaces after.
                let after = &line[i..];
                let mut n = i + after.chars().next().map_or(0, char::len_utf8);
                n += line[n..]
                    .find(|c: char| c != '─' && c != '━')
                    .unwrap_or(line.len() - n);
                n += line[n..].find(|c: char| c != ' ').unwrap_or(line.len() - n);
                self.start(NODE);
                self.token(TREE_BRANCH, n);
                if self.at_line_end() {
                    self.fail("expected a name after the branch");
                } else {
                    self.node();
                }
                self.finish();
                return;
            }
            if line.chars().all(|c| c.is_whitespace() || BOX.contains(&c)) {
                // Decoration, such as a `│` alone, is skipped.
                let n = scan::trim_end_len(r, line.len());
                self.token(TREE_BRANCH, n);
                return;
            }
        }
        self.start(NODE);
        self.node();
        self.finish();
    }

    fn at_line_end(&self) -> bool {
        let r = self.rest();
        scan::trim_end_len(r, scan::line_len(r)) == 0
    }

    /// A name, quoted or bare, then its annotations.
    fn node(&mut self) {
        let r = self.rest();
        let quote = r.chars().next().filter(|c| *c == '"' || *c == '\'');
        match quote {
            Some(q) => {
                let line = &r[1..scan::line_len(r)];
                match line.find(q) {
                    Some(i) => self.token(STRING, i + 2),
                    None => {
                        self.fail("unclosed quote: expected a closing quote on the same line");
                        return;
                    }
                }
            }
            None => {
                let n = bare_len(r);
                self.token(TEXT, n);
            }
        }
        loop {
            let r = self.rest();
            if let Some((kind, n)) = annotation_len(r) {
                self.ws();
                self.token(kind, n);
                continue;
            }
            break;
        }
        let n = scan::ws_len(self.rest());
        if !lines_end(&self.rest()[n..]) {
            self.fail("expected `:::class`, `icon(name)`, or `## description`");
        }
    }

    /// `title text`.
    fn title(&mut self) {
        self.start(TITLE_STMT);
        self.token(TITLE_KW, "title".len());
        self.ws();
        self.rest_of_line(TEXT);
        self.finish();
    }
}

/// Whether `s` starts at the end of a line.
fn lines_end(s: &str) -> bool {
    scan::line_len(s) == 0
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
    fn indented() {
        let (t, d) = tree(
            "treeView-beta\n  title Files\n  %% c\n  src/\n    \"a b\" :::highlight icon(logos:react) \
             ## main  thing\n    'c'\n  x  y z\n",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("NODE@").count(), 4);
        assert!(t.contains("TITLE_STMT"));
        assert!(
            t.contains("CLASS_SHORTHAND@") && t.contains("\":::highlight\""),
            "{t}"
        );
        assert!(
            t.contains("TREE_ICON@") && t.contains("\"icon(logos:react)\""),
            "{t}"
        );
        assert!(t.contains("\"## main  thing\""), "{t}");
        assert!(t.contains("TEXT@") && t.contains("\"x  y z\""), "{t}");
    }

    #[test]
    fn box_drawing() {
        let src =
            "treeView-beta\n├── src/\n│   ├── index.ts ## entry\n│\n│   └── b\n└── README.md\n";
        assert!(boxed(&src[14..]));
        let (t, d) = tree(src);
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(t.matches("NODE@").count(), 4);
        assert!(t.contains("\"│   ├── \""), "{t}");
        assert!(!boxed("  a\n  │\n"));
        assert!(!boxed("  %% ├── a\n  b\n"));
    }

    #[test]
    fn errors() {
        let e = |src: &str| tree(&format!("treeView-beta\n{src}\n")).1;
        assert_eq!(tree("treeView\n").1, ["expected `treeView-beta` header"]);
        assert_eq!(
            e("\"open"),
            ["unclosed quote: expected a closing quote on the same line"]
        );
        assert_eq!(e("├──"), ["expected a name after the branch"]);
        assert_eq!(
            e("\"a\" b"),
            ["expected `:::class`, `icon(name)`, or `## description`"]
        );
        // A bare name stops at `icon(` even when it does not close.
        assert_eq!(
            e("a icon(b"),
            ["expected `:::class`, `icon(name)`, or `## description`"]
        );
        assert!(e("a:::b c##d").is_empty());
    }
}
