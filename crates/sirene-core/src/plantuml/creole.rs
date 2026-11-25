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

/// `line` as a row of text runs and formula runs. Without the `math` feature, or without a
/// formula, one plain run.
#[allow(dead_code)]
pub(crate) fn runs(line: &str) -> Vec<Span> {
    crate::flowchart::formula::runs(line, Delim::Latex)
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
