//! The part of PlantUML's Creole that sirene reads (PCREO-4): `<latex>...</latex>` formulas
//! inside label text, the `\n` line breaks around them, and the labels of a lowered graph.
//!
//! A formula is an inline run of a label line, so its source is a [`Span`] with `math` set and
//! its tags kept in the text, as `$$...$$` formulas of Mermaid labels are (LEMB-2). `<math>` is
//! AsciiMath and stays literal text until that front end exists.

use std::ops::Range;

use rowan::TextRange;

use crate::diagnostic::Diagnostic;
use crate::flowchart::{Delim, Flowchart, Label, Span};
use crate::syntax::SyntaxNode;

const OPEN: &str = "<latex>";
const CLOSE: &str = "</latex>";

/// The byte ranges of the `<latex>...</latex>` formulas in one line, tags included. The tags are
/// matched in any case and the body is opaque up to the first closing tag (no nesting). An
/// unclosed tag, and a tag with an empty or blank body, are text.
pub(crate) fn latex_tags(line: &str) -> Vec<Range<usize>> {
    // ASCII lowercasing keeps byte offsets.
    let lower = line.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(open) = lower[at..].find(OPEN).map(|i| at + i) {
        let body = open + OPEN.len();
        let Some(close) = lower[body..].find(CLOSE).map(|i| body + i) else {
            break;
        };
        let end = close + CLOSE.len();
        if !line[body..close].trim().is_empty() {
            out.push(open..end);
        }
        at = end;
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

#[cfg(feature = "math")]
fn diagnose(labels: Vec<(Label, Vec<TextRange>)>, root: &SyntaxNode) -> Vec<Diagnostic> {
    use rowan::TextSize;
    use std::collections::HashSet;
    let src = root.text().to_string();
    let mut used = HashSet::new();
    let mut out = Vec::new();
    for (label, at) in labels {
        let lines = label.text.split('\n').map(crate::flowchart::clean);
        for (body, e) in crate::flowchart::formula::failures(lines, Delim::Latex) {
            // The first place in the label's source that has the body and has no diagnostic.
            let found = at.iter().find_map(|r| {
                let text = src.get(usize::from(r.start())..usize::from(r.end()))?;
                let mut from = 0;
                while let Some(i) = text.get(from..)?.find(&body) {
                    let abs = usize::from(r.start()) + from + i;
                    if used.insert(abs) {
                        return Some(abs);
                    }
                    from += i + body.chars().next().map_or(1, char::len_utf8);
                }
                None
            });
            let range = match found {
                Some(p) => {
                    let to = e.span.end.clamp(e.span.start, body.len());
                    let at = |o: usize| TextSize::new((p + o) as u32);
                    TextRange::new(at(e.span.start.min(body.len())), at(to))
                }
                None => at.first().copied().unwrap_or_else(|| root.text_range()),
            };
            out.push(Diagnostic::new(range, e.message));
        }
    }
    out
}

#[cfg(not(feature = "math"))]
fn diagnose(_: Vec<(Label, Vec<TextRange>)>, _: &SyntaxNode) -> Vec<Diagnostic> {
    Vec::new()
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
    fn other_tags_and_math_are_text() {
        assert!(found("<b>x</b> <math>x</math> <i>y</i>").is_empty());
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

    #[test]
    fn backslash_n_breaks_lines_outside_formulas() {
        assert_eq!(newlines("a\\nb"), "a\nb");
        assert_eq!(
            newlines("a\\n<latex>x \\neq y</latex>\\nb"),
            "a\n<latex>x \\neq y</latex>\nb"
        );
        assert_eq!(newlines("<latex>a\\nb"), "<latex>a\nb");
        assert_eq!(newlines("plain"), "plain");
    }
}
