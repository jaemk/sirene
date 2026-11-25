//! The part of PlantUML's Creole that sirene reads (PCREO-4): `<latex>...</latex>` (TeX) and
//! `<math>...</math>` (AsciiMath) formulas inside label text, the `\n` line breaks around them,
//! and the labels of a lowered graph.
//!
//! A formula is an inline run of a label line, so its source is a [`Span`] with `math` set and
//! its tags kept in the text, as `$$...$$` formulas of Mermaid labels are (LEMB-2); the tag says
//! the language ([`Span::is_asciimath`]).

use std::collections::HashSet;
use std::ops::Range;

use rowan::TextRange;

use crate::decor::Run;
use crate::diagnostic::Diagnostic;
use crate::flowchart::{Delim, Flowchart, Label, Span};
use crate::syntax::SyntaxNode;

/// The formula tags: the opening tag and its closing tag, lowercase. `<latex>` holds TeX and
/// `<math>` AsciiMath.
const TAGS: [(&str, &str); 2] = [("<latex>", "</latex>"), ("<math>", "</math>")];

/// The byte ranges of the `<latex>...</latex>` and `<math>...</math>` formulas in one line, tags
/// included. The tags are matched in any case and the body is opaque up to the first closing
/// tag of the same name (no nesting). An unclosed tag, and a tag with an empty or blank body,
/// are text.
pub(crate) fn latex_tags(line: &str) -> Vec<Range<usize>> {
    /// One tag name's place in the line. Positions only move forward, so each is searched for at
    /// most once past where it was found, and a name whose closing tag is missing is done: no
    /// later opening tag of it can close either.
    struct Tag {
        open: &'static str,
        close: &'static str,
        /// The first opening tag at or after the scan position, once searched for.
        opener: Option<usize>,
        /// The first closing tag after the last opening tag used, once searched for.
        closer: Option<usize>,
        done: bool,
    }
    // ASCII lowercasing keeps byte offsets.
    let lower = line.to_ascii_lowercase();
    let mut tags = TAGS.map(|(open, close)| Tag {
        open,
        close,
        opener: None,
        closer: None,
        done: false,
    });
    let mut out = Vec::new();
    let mut at = 0;
    loop {
        for t in tags.iter_mut().filter(|t| !t.done) {
            if t.opener.is_some_and(|o| o >= at) {
                continue;
            }
            match lower[at..].find(t.open) {
                Some(i) => t.opener = Some(at + i),
                None => t.done = true,
            }
        }
        // The first opening tag from `at`, of either name.
        let next = tags
            .iter_mut()
            .filter(|t| !t.done)
            .filter_map(|t| t.opener.map(|o| (o, t)))
            .min_by_key(|&(o, _)| o);
        let Some((open, t)) = next else {
            break;
        };
        let body = open + t.open.len();
        let closer = match t.closer {
            Some(c) if c >= body => Some(c),
            _ => lower[body..].find(t.close).map(|i| body + i),
        };
        let Some(end) = closer else {
            // No closing tag: this is text, and a tag of the other name may still follow.
            t.done = true;
            at = body;
            continue;
        };
        t.closer = Some(end);
        let after = end + t.close.len();
        if !line[body..end].trim().is_empty() {
            out.push(open..after);
        }
        at = after;
    }
    out
}

/// The byte offset of the first `pat` in `s` that is not inside a formula, so a `->`, `:`, `#`, or
/// `,` written in a formula is not a statement's.
pub(crate) fn find_outside(s: &str, pat: &str) -> Option<usize> {
    let tags = latex_tags(s);
    let mut from = 0;
    while let Some(i) = s[from..].find(pat) {
        let at = from + i;
        match tags.iter().find(|r| r.contains(&at)) {
            Some(r) => from = r.end,
            None => return Some(at),
        }
    }
    None
}

/// `s` split at each `sep` outside a formula.
pub(crate) fn split_outside(s: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = s;
    let mut buf = [0; 4];
    let sep = sep.encode_utf8(&mut buf);
    while let Some(i) = find_outside(rest, sep) {
        out.push(&rest[..i]);
        rest = &rest[i + sep.len()..];
    }
    out.push(rest);
    out
}

/// `line` as a row of text runs and formula runs. Without the `math` feature, or without a
/// formula, one plain run.
pub(crate) fn runs(line: &str) -> Vec<Span> {
    crate::flowchart::formula::runs(line, Delim::Latex)
}

/// `line` as styled runs: the text and formula runs of [`runs`], each with the Creole style in
/// force where it starts (PCREO-3). Read: `**bold**`, `//italic//`, `__underline__`,
/// `--strike--`, `""monospace""`, `<b>`, `<i>`, `<u>`, `<s>`, `<color:c>`, `<size:N>`, and
/// `<back:c>`, with their closing tags. A style carries across a formula. `bold_all` makes the
/// whole line bold (a heading). A marker with no closing marker later in the line, or followed
/// by a space, is text, so `A --> B` and `a__b` are not styled.
pub(crate) fn styled(line: &str, bold_all: bool) -> Vec<Run> {
    let mut state = State {
        bold: bold_all,
        ..State::default()
    };
    let mut out = Vec::new();
    let mut at = 0;
    for piece in runs(line) {
        let len = piece.text.len();
        if piece.math {
            out.push(Run {
                span: piece,
                color: state.color.clone(),
                ..Run::default()
            });
        } else {
            state.read(&line[at..], len, bold_all, &mut out);
        }
        at += len;
    }
    out
}

/// The style in force while a line is read.
#[derive(Default)]
struct State {
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    mono: bool,
    color: Option<String>,
    size: Option<u32>,
    back: Option<String>,
}

/// A style marker that is a pair of characters.
const PAIRS: [&str; 3] = ["__", "--", "\"\""];

impl State {
    /// Reads the first `len` bytes of `tail` (the rest of the line from here), adding its runs
    /// to `out` and changing the state at each marker.
    fn read(&mut self, tail: &str, len: usize, bold_all: bool, out: &mut Vec<Run>) {
        let text = &tail[..len];
        let mut buf = String::new();
        let mut i = 0;
        while i < text.len() {
            let rest = &tail[i..];
            let before = if i == 0 {
                None
            } else {
                text[..i].chars().next_back()
            };
            match self.marker(rest, before, bold_all) {
                Some((n, change)) => {
                    self.push(&mut buf, out);
                    self.apply(change);
                    i += n;
                }
                None => {
                    let c = rest.chars().next().map_or(1, char::len_utf8);
                    buf.push_str(&rest[..c]);
                    i += c;
                }
            }
        }
        self.push(&mut buf, out);
    }

    /// The marker `rest` starts with, its length and what it changes.
    fn marker(&self, rest: &str, before: Option<char>, bold_all: bool) -> Option<(usize, Change)> {
        let lower = rest
            .get(..rest.len().min(8))
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        let tag = |t: &str| lower.starts_with(t).then_some(t.len());
        if rest.starts_with("**") {
            return Some((2, Change::Bold(!self.bold)));
        }
        if rest.starts_with("//") && before != Some(':') {
            return Some((2, Change::Italic(!self.italic)));
        }
        for pair in PAIRS {
            if !rest.starts_with(pair) {
                continue;
            }
            let on = match pair {
                "__" => self.underline,
                "--" => self.strike,
                _ => self.mono,
            };
            // An opener has text after it and a closer later; a closer closes.
            let after = &rest[2..];
            let opens = after.chars().next().is_some_and(|c| {
                !c.is_whitespace()
                    && c != '>'
                    && after.get(c.len_utf8()..).is_some_and(|r| r.contains(pair))
            });
            if !on && !opens {
                return None;
            }
            let change = match pair {
                "__" => Change::Underline(!on),
                "--" => Change::Strike(!on),
                _ => Change::Mono(!on),
            };
            return Some((2, change));
        }
        if let Some(n) = tag("<b>") {
            return Some((n, Change::Bold(true)));
        }
        if let Some(n) = tag("</b>") {
            return Some((n, Change::Bold(bold_all)));
        }
        if let Some(n) = tag("<i>") {
            return Some((n, Change::Italic(true)));
        }
        if let Some(n) = tag("</i>") {
            return Some((n, Change::Italic(false)));
        }
        if let Some(n) = tag("<u>") {
            return Some((n, Change::Underline(true)));
        }
        if let Some(n) = tag("</u>") {
            return Some((n, Change::Underline(false)));
        }
        if let Some(n) = tag("<s>").or_else(|| tag("<del>")) {
            return Some((n, Change::Strike(true)));
        }
        if let Some(n) = tag("</s>").or_else(|| tag("</del>")) {
            return Some((n, Change::Strike(false)));
        }
        if let Some(n) = tag("</color>") {
            return Some((n, Change::Color(None)));
        }
        if let Some(n) = tag("</size>") {
            return Some((n, Change::Size(None)));
        }
        if let Some(n) = tag("</back>") {
            return Some((n, Change::Back(None)));
        }
        // `<color:red>`, `<size:14>`, `<back:#ff0>`: the tag, a colon, the value, a `>`.
        for name in ["color", "size", "back"] {
            let open = format!("<{name}:");
            if !lower.starts_with(&open) {
                continue;
            }
            let close = rest.find('>')?;
            let value = rest[open.len()..close].trim().trim_start_matches('#');
            let change = match name {
                "color" => Change::Color(Some(value.to_owned())),
                "size" => Change::Size(Some(value.parse().ok().filter(|n| *n > 0)?)),
                _ => Change::Back(Some(value.to_owned())),
            };
            return Some((close + 1, change));
        }
        None
    }

    fn apply(&mut self, change: Change) {
        match change {
            Change::Bold(b) => self.bold = b,
            Change::Italic(b) => self.italic = b,
            Change::Underline(b) => self.underline = b,
            Change::Strike(b) => self.strike = b,
            Change::Mono(b) => self.mono = b,
            Change::Color(c) => self.color = c,
            Change::Size(s) => self.size = s,
            Change::Back(c) => self.back = c,
        }
    }

    fn push(&self, buf: &mut String, out: &mut Vec<Run>) {
        if buf.is_empty() {
            return;
        }
        out.push(Run {
            span: Span {
                text: std::mem::take(buf),
                bold: self.bold,
                italic: self.italic,
                ..Span::default()
            },
            color: self.color.clone(),
            underline: self.underline,
            strike: self.strike,
            mono: self.mono,
            size: self.size,
            back: self.back.clone(),
        });
    }
}

enum Change {
    Bold(bool),
    Italic(bool),
    Underline(bool),
    Strike(bool),
    Mono(bool),
    Color(Option<String>),
    Size(Option<u32>),
    Back(Option<String>),
}

/// `s` with each literal `\n` made a line break, except inside a formula: `\neq` and `\nu` are
/// TeX commands, not breaks.
pub(crate) fn newlines(s: &str) -> String {
    if !s.contains("\\n") {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut at = 0;
    for r in latex_tags(s) {
        out.push_str(&s[at..r.start].replace("\\n", "\n"));
        out.push_str(&s[r.clone()]);
        at = r.end;
    }
    out.push_str(&s[at..].replace("\\n", "\n"));
    out
}

/// Marks every label of a lowered graph as PlantUML text, so `<latex>` tags in it are formulas
/// and `$$` is not, and reports each formula that does not typeset on its body in the source.
pub(crate) fn mark_labels(chart: &mut Flowchart, root: &SyntaxNode) -> Vec<Diagnostic> {
    let mut found = Vec::new();
    each_label(chart, |label, at| {
        label.delim = Delim::Latex;
        if label.has_formula() {
            found.push((label.clone(), at.to_vec()));
        }
    });
    // Compartment rows (class members, state bodies) are read with their node's label delimiters.
    // A row has no range of its own, so its formulas are looked for in the whole source, after
    // the labels have taken the places they are written in.
    let whole = [root.text_range()];
    for cell in chart
        .nodes
        .iter()
        .flat_map(|n| n.sections.iter().flatten().flat_map(|r| &r.cells))
    {
        let label = Label {
            text: cell.clone(),
            markdown: false,
            delim: Delim::Latex,
        };
        if label.has_formula() {
            found.push((label, whole.to_vec()));
        }
    }
    // The document title has no range of its own either, so it is looked for in the whole source.
    chart.title_delim = Delim::Latex;
    if let Some(title) = &chart.title {
        let label = Label {
            text: title.clone(),
            markdown: false,
            delim: Delim::Latex,
        };
        if label.has_formula() {
            found.push((label, whole.to_vec()));
        }
    }
    diagnose(found, root)
}

/// A diagnostic for each formula of `texts` that does not typeset (and without the `math`
/// feature, for each formula), for a kind that keeps its text as plain strings: each text comes
/// with the source ranges it may be written in, and its formulas are found there as written,
/// each taking a place of its own in order.
pub(crate) fn check_texts(
    root: &SyntaxNode,
    texts: impl IntoIterator<Item = (String, Vec<TextRange>)>,
) -> Vec<Diagnostic> {
    let found = texts
        .into_iter()
        .map(|(text, at)| {
            let label = Label {
                text,
                markdown: false,
                delim: Delim::Latex,
            };
            (label, at)
        })
        .filter(|(label, _)| label.has_formula())
        .collect();
    diagnose(found, root)
}

/// Calls `f` with every label of `chart` and the source ranges it may be written in.
fn each_label(chart: &mut Flowchart, mut f: impl FnMut(&mut Label, &[TextRange])) {
    for n in &mut chart.nodes {
        f(&mut n.label, &n.mentions);
    }
    for e in &mut chart.edges {
        let at = [e.range];
        for l in [&mut e.label, &mut e.start_label, &mut e.end_label]
            .into_iter()
            .flatten()
        {
            f(l, &at);
        }
    }
    for s in &mut chart.subgraphs {
        f(&mut s.title, &[s.range]);
    }
    for n in &mut chart.notes {
        f(&mut n.label, &[n.range]);
    }
}

/// A diagnostic for each formula in `labels` that does not typeset (and without the `math`
/// feature, for each formula), on the formula's body where the label's source has it as written.
fn diagnose(labels: Vec<(Label, Vec<TextRange>)>, root: &SyntaxNode) -> Vec<Diagnostic> {
    let src = root.text().to_string();
    let mut used = HashSet::new();
    let mut out = Vec::new();
    for (label, at) in labels {
        let lines = label.text.split('\n').map(crate::flowchart::clean);
        out.extend(crate::flowchart::formula::errors_among(
            &src,
            lines,
            &at,
            &mut used,
            root.text_range(),
            Delim::Latex,
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(line: &str) -> Vec<&str> {
        latex_tags(line).into_iter().map(|r| &line[r]).collect()
    }

    #[test]
    fn finds_closed_tags() {
        assert_eq!(found("a <latex>x</latex> b"), ["<latex>x</latex>"]);
        assert_eq!(
            found("<latex>a</latex>+<latex>b</latex>"),
            ["<latex>a</latex>", "<latex>b</latex>"]
        );
        assert!(found("no formula here").is_empty());
    }

    #[test]
    fn tags_match_in_any_case() {
        assert_eq!(found("<LaTeX>x</LATEX>"), ["<LaTeX>x</LATEX>"]);
    }

    #[test]
    fn unclosed_and_empty_tags_are_text() {
        assert!(found("<latex>x").is_empty());
        assert!(found("x</latex>").is_empty());
        assert!(found("<latex></latex>").is_empty());
        assert!(found("<latex>  </latex>").is_empty());
        assert_eq!(
            found("<latex></latex> <latex>y</latex>"),
            ["<latex>y</latex>"]
        );
        // An unclosed second tag does not touch the first formula.
        assert_eq!(found("<latex>x</latex> <latex>y"), ["<latex>x</latex>"]);
    }

    #[test]
    fn body_is_opaque_up_to_the_first_closing_tag() {
        assert_eq!(
            found("<latex>a<latex>b</latex>c</latex>"),
            ["<latex>a<latex>b</latex>"]
        );
        assert_eq!(found("<latex>$$x$$</latex>"), ["<latex>$$x$$</latex>"]);
    }

    #[test]
    fn other_tags_are_text() {
        assert!(found("<b>x</b> <i>y</i> <mathx>z</mathx>").is_empty());
    }

    #[test]
    fn math_tags_are_formulas() {
        assert_eq!(
            found("a <math>x^2</math> b <MATH>y</Math>"),
            ["<math>x^2</math>", "<MATH>y</Math>"]
        );
        assert_eq!(
            found("<latex>a</latex><math>b</math>"),
            ["<latex>a</latex>", "<math>b</math>"]
        );
    }

    #[test]
    fn each_tag_closes_with_its_own_name() {
        // The body is opaque up to the closing tag of the same name.
        assert_eq!(
            found("<math>a</latex>b</math>"),
            ["<math>a</latex>b</math>"]
        );
        assert_eq!(
            found("<latex>a</math>b</latex>"),
            ["<latex>a</math>b</latex>"]
        );
    }

    #[test]
    fn an_unclosed_tag_leaves_the_other_name_alone() {
        assert_eq!(found("<latex>x <math>y</math>"), ["<math>y</math>"]);
        assert!(found("<math>x").is_empty());
        assert!(found("<math></math>").is_empty());
        assert!(found("<math> </math>").is_empty());
    }

    /// Many unclosed tags are scanned once, not once per tag: this was quadratic (50000 tags
    /// took over two minutes). The bound is generous; the quadratic scan misses it by far.
    #[test]
    fn many_unclosed_tags_are_scanned_in_linear_time() {
        use std::time::{Duration, Instant};
        let start = Instant::now();
        assert!(found(&"<latex>x ".repeat(50_000)).is_empty());
        assert!(found(&"<math>x ".repeat(50_000)).is_empty());
        assert!(found(&"<latex>a <math>b ".repeat(50_000)).is_empty());
        // Another name's formula after the unclosed ones is still found.
        let line = format!("{}<math>y</math>", "<latex>".repeat(50_000));
        assert_eq!(found(&line), ["<math>y</math>"]);
        // Another name's closer does not close a tag.
        assert!(found(&"<latex>a</math> ".repeat(50_000)).is_empty());
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "took {:?}",
            start.elapsed()
        );
    }

    #[test]
    fn a_closed_tag_after_many_blank_ones_is_found() {
        let line = format!("{}<latex>x</latex>", "<latex> </latex>".repeat(1_000));
        assert_eq!(found(&line), ["<latex>x</latex>"]);
    }

    #[test]
    fn multibyte_text_keeps_offsets() {
        let line = "\u{e9}\u{e9} <latex>\\alpha</latex> \u{e9}";
        assert_eq!(found(line), ["<latex>\\alpha</latex>"]);
    }

    #[cfg(feature = "math")]
    #[test]
    fn runs_split_text_and_formulas() {
        let r = runs("see <latex>x^2</latex> and <LATEX>y</LATEX>!");
        let shown: Vec<(&str, bool)> = r.iter().map(|s| (s.text.as_str(), s.math)).collect();
        assert_eq!(
            shown,
            [
                ("see ", false),
                ("<latex>x^2</latex>", true),
                (" and ", false),
                ("<LATEX>y</LATEX>", true),
                ("!", false),
            ]
        );
        assert_eq!(r[1].formula().as_deref(), Some("x^2"));
        assert_eq!(r[3].formula().as_deref(), Some("y"));
    }

    #[cfg(feature = "math")]
    #[test]
    fn dollars_are_not_math_here() {
        let r = runs("$$x$$");
        assert_eq!(r.len(), 1);
        assert!(!r[0].math);
    }

    #[cfg(feature = "math")]
    #[test]
    fn math_runs_are_asciimath() {
        let r = runs("a <math>x^2</math> <latex>y</latex>");
        assert_eq!(r[1].formula().as_deref(), Some("x^2"));
        assert!(r[1].is_asciimath());
        assert!(!r[3].is_asciimath());
        assert!(r[1].typeset().unwrap().is_ok());
    }

    #[test]
    fn statement_punctuation_in_a_formula_is_not_the_statements() {
        assert_eq!(find_outside("a -> b", "->"), Some(2));
        assert_eq!(find_outside("<math>a->b</math> -> c", "->"), Some(18));
        assert_eq!(find_outside("<math>a->b</math>", "->"), None);
        // An unclosed tag is text.
        assert_eq!(find_outside("<math>a->b", "->"), Some(7));
        assert_eq!(
            split_outside("a, <math>f(x,y)</math>, b", ','),
            ["a", " <math>f(x,y)</math>", " b"]
        );
        assert_eq!(split_outside("", ','), [""]);
        // A caption's `:` is outside the formula.
        let (head, caption) = super::super::lines::caption("x : <latex>a: b</latex>");
        assert_eq!(
            (head, caption.as_deref()),
            ("x", Some("<latex>a: b</latex>"))
        );
        let (head, caption) = super::super::lines::caption("<latex>a: b</latex>");
        assert_eq!((head, caption.as_deref()), ("<latex>a: b</latex>", None));
    }

    #[test]
    fn backslash_n_breaks_lines_outside_formulas() {
        assert_eq!(newlines("a\\nb"), "a\nb");
        assert_eq!(
            newlines("a\\n<latex>x \\neq y</latex>\\nb"),
            "a\n<latex>x \\neq y</latex>\nb"
        );
        assert_eq!(newlines("<latex>a\\nb"), "<latex>a\nb");
        assert_eq!(
            newlines("a\\n<math>x \\n y</math>"),
            "a\n<math>x \\n y</math>"
        );
        assert_eq!(newlines("plain"), "plain");
    }
}
