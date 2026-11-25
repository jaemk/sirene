//! Formulas in label text (LEMB-1, LEMB-2), shared by every diagram that has them: the runs of
//! a line and the diagnostics for formulas that do not typeset (LEMB-4, LEMB-5). Mermaid writes
//! a formula `$$...$$` and PlantUML `<latex>...</latex>` (TeX) or `<math>...</math>`
//! (AsciiMath): a [`Delim`] says which.

use std::collections::HashSet;
use std::ops::Range;

use rowan::{TextRange, TextSize};

use super::model::math_line;
use super::{Delim, Span};
use crate::diagnostic::Diagnostic;
use crate::syntax::SyntaxToken;

/// Whether `line` holds a formula in `delim`.
pub fn has(line: &str, delim: Delim) -> bool {
    !delim.ranges(line).is_empty()
}

/// `line` as a row of text runs and formula runs. A formula is a run of its own whether or not
/// math support is built in: without it the run draws as its source.
pub fn runs(line: &str, delim: Delim) -> Vec<Span> {
    if has(line, delim) {
        math_line(line, delim)
    } else {
        vec![Span::plain(line)]
    }
}

/// The offset in `body` of byte `at` of `body` with `\\` read as `\`.
fn uncollapsed(body: &str, at: usize) -> usize {
    let bytes = body.as_bytes();
    let (mut i, mut seen) = (0, 0);
    while i < bytes.len() && seen < at {
        i += if bytes[i] == b'\\' && bytes.get(i + 1) == Some(&b'\\') {
            2
        } else {
            1
        };
        seen += 1;
    }
    i
}

/// Where the body of the formula run `text` (delimiters included, `open` bytes of opening
/// delimiter) is in `raw`, searching from byte `from`: the offset of its body, when the whole run
/// is there as written.
///
/// Searching for the run and not the body alone keeps a body that also appears as plain text
/// (`\foo is $$\foo$$`) from being found in the wrong place.
pub(crate) fn locate(raw: &str, from: usize, text: &str, open: usize) -> Option<usize> {
    let i = raw.get(from..)?.find(text)?;
    Some(from + i + open)
}

/// The range in `raw` of the failing `span` of the formula whose body `body` starts at `p`.
/// `span` is in the TeX source ([`Delim::source`] of the body), so for Mermaid's `$$`, which
/// reads `\\` as `\`, it is mapped back onto the body as written. Offsets past the body are
/// clamped to its end.
pub(crate) fn source_range(delim: Delim, body: &str, p: usize, span: Range<usize>) -> Range<usize> {
    let (from, to) = match delim {
        Delim::Dollar => {
            let from = uncollapsed(body, span.start);
            (from, uncollapsed(body, span.end).max(from))
        }
        Delim::Latex => (span.start, span.end),
    };
    let s = from.min(body.len());
    let t = to.clamp(s, body.len());
    p + s..p + t
}

/// What is wrong with one formula.
pub(crate) struct Issue {
    pub message: String,
    /// In the TeX source of the body; see [`source_range`].
    pub span: Range<usize>,
    /// Whether it is a warning, not an error: the formula is fine but cannot be typeset here.
    pub warning: bool,
}

impl Issue {
    /// The diagnostic for this issue on `range`.
    pub(crate) fn at(self, range: TextRange) -> Diagnostic {
        if self.warning {
            Diagnostic::warning(range, self.message)
        } else {
            Diagnostic::new(range, self.message)
        }
    }
}

/// Each formula in `lines`, in order: the run with its delimiters and the lengths of its opening
/// and closing delimiters.
pub(crate) fn formulas(
    lines: impl IntoIterator<Item = String>,
    delim: Delim,
) -> Vec<(String, usize, usize)> {
    let mut out = Vec::new();
    for line in lines {
        for r in delim.ranges(&line) {
            let text = &line[r];
            let (open, close) = delim.lens(text);
            out.push((text.to_owned(), open, close));
        }
    }
    out
}

/// The body of `text`, a formula run, between its delimiters.
pub(crate) fn body_of(text: &str, open: usize, close: usize) -> &str {
    text.get(open..text.len().saturating_sub(close))
        .unwrap_or_default()
}

/// What is wrong with the formula run `text`: an error when it does not typeset, and without the
/// `math` feature a warning, since it is drawn as its source.
#[cfg(feature = "math")]
pub(crate) fn issue(delim: Delim, text: &str) -> Option<Issue> {
    let (open, close) = delim.lens(text);
    let src = delim.source(body_of(text, open, close));
    let e = delim.flavor(text).typeset(&src, delim.style()).err()?;
    Some(Issue {
        message: e.message,
        span: e.span,
        warning: false,
    })
}

#[cfg(not(feature = "math"))]
pub(crate) fn issue(_: Delim, _: &str) -> Option<Issue> {
    Some(Issue {
        message: NO_MATH.to_owned(),
        span: 0..usize::MAX,
        warning: true,
    })
}

/// The warning for a formula in a build without the `math` feature.
#[cfg(not(feature = "math"))]
const NO_MATH: &str = "math support is off in this build; the formula is drawn as its source";

/// A diagnostic for each formula in `lines` that does not typeset, in order.
///
/// `raw` is the source text the lines came from, starting at `start`: a formula is found in it
/// as written, delimiters and all, and the diagnostic is on the failing span inside the
/// delimiters. A formula not found as written (entity codes, say) is reported on `fallback`.
/// Without the `math` feature each formula is a warning on its body.
pub(crate) fn errors(
    start: TextSize,
    raw: &str,
    lines: impl IntoIterator<Item = String>,
    fallback: TextRange,
    delim: Delim,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut cursor = 0;
    for (text, open, close) in formulas(lines, delim) {
        let body = body_of(&text, open, close);
        // Where the body is in the source text, when it is there as written.
        let found = locate(raw, cursor, &text, open);
        if let Some(p) = found {
            cursor = p + body.len() + close;
        }
        let Some(issue) = issue(delim, &text) else {
            continue;
        };
        let range = match found {
            Some(p) => body_range(start, delim, body, p, issue.span.clone()),
            None => fallback,
        };
        out.push(issue.at(range));
    }
    out
}

/// The range of the failing `span` of the formula whose body `body` starts at byte `p` of the
/// source text that starts at `start` (see [`source_range`]).
fn body_range(
    start: TextSize,
    delim: Delim,
    body: &str,
    p: usize,
    span: Range<usize>,
) -> TextRange {
    let r = source_range(delim, body, p, span);
    let at = |o: usize| start + TextSize::new(o as u32);
    TextRange::new(at(r.start), at(r.end))
}

/// A diagnostic for each formula in `lines` that does not typeset, in order, found in `src` (the
/// whole source text) inside the `candidates` it may be written in.
///
/// Each formula takes the first place in a candidate, in order, that has it as written and is not
/// in `used` (offsets in `src` of formula bodies already taken, which this adds to). One not found
/// is reported on the first candidate, or on `fallback` with none.
pub(crate) fn errors_among(
    src: &str,
    lines: impl IntoIterator<Item = String>,
    candidates: &[TextRange],
    used: &mut HashSet<usize>,
    fallback: TextRange,
    delim: Delim,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (text, open, close) in formulas(lines, delim) {
        let Some(issue) = issue(delim, &text) else {
            continue;
        };
        let found = candidates.iter().find_map(|r| {
            let start = usize::from(r.start());
            let hay = src.get(start..usize::from(r.end()))?;
            let mut from = 0;
            while let Some(p) = locate(hay, from, &text, open) {
                if used.insert(start + p) {
                    return Some(start + p);
                }
                // Past the first byte of this match, to look for the next.
                from = p - open + 1;
            }
            None
        });
        let range = match found {
            Some(p) => {
                let body = body_of(&text, open, close);
                body_range(TextSize::new(0), delim, body, p, issue.span.clone())
            }
            None => candidates.first().copied().unwrap_or(fallback),
        };
        out.push(issue.at(range));
    }
    out
}

/// The diagnostics for the formulas of `text`, the lines of a piece of source text. `token` is
/// the source text they came from, when it has one: without it, and for a formula not found in
/// it, `fallback` is the range reported.
pub(crate) fn check_text(
    token: Option<&SyntaxToken>,
    text: &[String],
    fallback: TextRange,
    delim: Delim,
) -> Vec<Diagnostic> {
    if !text.iter().any(|l| has(l, delim)) {
        return Vec::new();
    }
    let raw = token.map_or("", |t| t.text());
    let start = token.map_or(fallback.start(), |t| t.text_range().start());
    errors(start, raw, text.iter().cloned(), fallback, delim)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(raw: &str, d: &Diagnostic) -> String {
        raw[usize::from(d.range.start())..usize::from(d.range.end())].to_owned()
    }

    fn lines(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| (*s).to_owned()).collect()
    }

    fn whole(raw: &str) -> TextRange {
        TextRange::new(0.into(), TextSize::new(raw.len() as u32))
    }

    #[test]
    fn locate_finds_the_delimited_run_not_the_bare_body() {
        let raw = r"\foo is $$\foo$$";
        let p = locate(raw, 0, r"$$\foo$$", 2).unwrap();
        assert_eq!(&raw[p..p + 4], r"\foo");
        assert_eq!(p, 10);
        // The bare body is at 0; the run is not.
        assert_eq!(locate(raw, 0, r"\foo", 0), Some(0));
        assert_eq!(locate(raw, 9, r"$$\foo$$", 2), None);
        assert_eq!(locate(raw, 99, "x", 0), None);
    }

    #[test]
    fn source_range_maps_and_clamps() {
        // `\\` is one source byte.
        assert_eq!(source_range(Delim::Dollar, r"\\foo", 10, 0..4), 10..15);
        assert_eq!(source_range(Delim::Latex, r"\\foo", 10, 0..4), 10..14);
        // Past the end, and reversed.
        assert_eq!(source_range(Delim::Latex, "ab", 5, 9..20), 7..7);
        let reversed = Range { start: 2, end: 1 };
        assert_eq!(source_range(Delim::Latex, "ab", 5, reversed), 7..7);
        assert_eq!(source_range(Delim::Dollar, "ab", 5, 1..30), 6..7);
    }

    #[cfg(feature = "math")]
    #[test]
    fn a_body_that_is_also_plain_text_is_found_inside_its_delimiters() {
        let raw = r"A[\foo is $$\foo$$]";
        let d = errors(
            0.into(),
            raw,
            lines(&[r"\foo is $$\foo$$"]),
            whole(raw),
            Delim::Dollar,
        );
        let [d] = d.as_slice() else {
            panic!("one diagnostic, got {d:?}");
        };
        assert_eq!(at(raw, d), r"\foo");
        assert_eq!(usize::from(d.range.start()), 12, "{d:?}");
    }

    #[cfg(feature = "math")]
    #[test]
    fn a_doubled_backslash_is_mapped_back_onto_the_source() {
        let raw = r"x $$\\foo$$ y";
        let d = errors(0.into(), raw, lines(&[raw]), whole(raw), Delim::Dollar);
        let [d] = d.as_slice() else {
            panic!("one diagnostic, got {d:?}");
        };
        assert_eq!(at(raw, d), r"\\foo");
    }

    #[cfg(feature = "math")]
    #[test]
    fn a_multibyte_prefix_keeps_the_offsets() {
        let raw = "\u{e9}\u{e9}\u{4e2d} $$\\foo$$";
        let d = errors(0.into(), raw, lines(&[raw]), whole(raw), Delim::Dollar);
        let [d] = d.as_slice() else {
            panic!("one diagnostic, got {d:?}");
        };
        assert_eq!(at(raw, d), r"\foo");
        // The start offset moves the range as a whole.
        let d = errors(
            TextSize::new(100),
            raw,
            lines(&[raw]),
            TextRange::new(100.into(), 110.into()),
            Delim::Dollar,
        );
        assert_eq!(
            usize::from(d[0].range.start()),
            100 + raw.find('\\').unwrap()
        );
    }

    #[cfg(feature = "math")]
    #[test]
    fn a_body_not_in_the_source_as_written_is_reported_on_the_fallback() {
        // The source writes the quote as an entity code; the decoded text has `"`.
        let raw = r"$$\foo #quot;x#quot;$$";
        let fallback = TextRange::new(3.into(), 8.into());
        let d = errors(
            0.into(),
            raw,
            lines(&[r#"$$\foo "x"$$"#]),
            fallback,
            Delim::Dollar,
        );
        let [d] = d.as_slice() else {
            panic!("one diagnostic, got {d:?}");
        };
        assert_eq!(d.range, fallback);
    }

    #[cfg(feature = "math")]
    #[test]
    fn a_repeated_failing_body_is_diagnosed_in_each_place() {
        let raw = r"$$\foo$$ and $$\foo$$";
        let d = errors(0.into(), raw, lines(&[raw]), whole(raw), Delim::Dollar);
        let starts: Vec<usize> = d.iter().map(|d| usize::from(d.range.start())).collect();
        assert_eq!(starts, [2, 15]);
        assert!(d.iter().all(|d| at(raw, d) == r"\foo"));
    }

    #[cfg(feature = "math")]
    #[test]
    fn equal_formulas_take_the_places_of_their_candidates_in_order() {
        let src = r"<latex>\foo</latex> <latex>\foo</latex> <latex>\foo</latex>";
        let (a, b) = (TextRange::new(0.into(), 19.into()), whole(src));
        let mut used = HashSet::new();
        let one = lines(&[r"<latex>\foo</latex>"]);
        let mut starts = Vec::new();
        for _ in 0..3 {
            let d = errors_among(src, one.clone(), &[a, b], &mut used, b, Delim::Latex);
            starts.push(usize::from(d[0].range.start()));
        }
        assert_eq!(starts, [7, 27, 47]);
        // All places taken: reported on the first candidate.
        let d = errors_among(src, one.clone(), &[a, b], &mut used, b, Delim::Latex);
        assert_eq!(d[0].range, a);
        // No candidates: the fallback.
        let fallback = TextRange::new(1.into(), 2.into());
        let d = errors_among(src, one, &[], &mut used, fallback, Delim::Latex);
        assert_eq!(d[0].range, fallback);
    }

    #[cfg(feature = "math")]
    #[test]
    fn no_source_text_reports_on_the_fallback() {
        let fallback = TextRange::new(4.into(), 9.into());
        let d = check_text(None, &lines(&[r"$$\foo$$"]), fallback, Delim::Dollar);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].range, fallback);
        // Text with no formula has nothing to report.
        assert!(check_text(None, &lines(&["plain"]), fallback, Delim::Dollar).is_empty());
        assert!(check_text(None, &lines(&["$$x$$"]), fallback, Delim::Dollar).is_empty());
    }

    #[cfg(not(feature = "math"))]
    #[test]
    fn without_math_each_formula_is_a_warning_on_its_body() {
        let raw = "a $$x$$ b <latex>y</latex>";
        let d = errors(0.into(), raw, lines(&[raw]), whole(raw), Delim::Dollar);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].severity, crate::diagnostic::Severity::Warning);
        assert_eq!(at(raw, &d[0]), "x");
        let d = errors(0.into(), raw, lines(&[raw]), whole(raw), Delim::Latex);
        assert_eq!(at(raw, &d[0]), "y");
    }

    #[test]
    fn runs_split_a_formula_off_in_any_build() {
        let r = runs("a $$x$$ b", Delim::Dollar);
        let shown: Vec<(&str, bool)> = r.iter().map(|s| (s.text.as_str(), s.math)).collect();
        assert_eq!(shown, [("a ", false), ("$$x$$", true), (" b", false)]);
    }
}
