//! TeX source as one [`Equation`] (LEMB-3): a bare formula, or one inside the wrappers a TeX file
//! or a `\begin{document}` puts around a formula. The equation's source is the formula alone, and
//! every diagnostic's range is moved back into the text the user wrote.

use std::ops::Range;

use rowan::{TextRange, TextSize};

use super::{Equation, Flavor};
use crate::diagnostic::Diagnostic;

/// The error for a source with a preamble or `\begin{document}` that is not one math block.
pub(crate) const DOCUMENT: &str = "full LaTeX documents are not supported; give one formula";

/// The environments of a document's one math block that are passed to the typesetter whole.
const KEPT: &[&str] = &[
    "equation",
    "equation*",
    "align",
    "align*",
    "alignat",
    "alignat*",
    "aligned",
    "gather",
    "gather*",
    "gathered",
    "multline",
    "multline*",
    "flalign",
    "flalign*",
    "split",
    "eqnarray",
    "eqnarray*",
    "CD",
];

/// A TeX file's formula (see [`Equation::tex`]).
pub(super) fn lower(src: &str) -> (Equation, Vec<Diagnostic>) {
    let whole = 0..src.len();
    let body = if is_document(src) {
        match document(src) {
            Some(body) => body,
            None => return refused(src),
        }
    } else {
        let code = trim(src, whole);
        wrapper(src, code.clone()).unwrap_or(code)
    };
    equation(src, body, false)
}

/// A formula that is all of `src` (see [`Equation::formula`]).
pub(super) fn bare(src: &str, inline: bool) -> (Equation, Vec<Diagnostic>) {
    equation(src, 0..src.len(), inline)
}

/// The equation of `src[body]`, with the typesetter's errors in `src`.
fn equation(src: &str, body: Range<usize>, inline: bool) -> (Equation, Vec<Diagnostic>) {
    let equation = Equation {
        source: src[body.clone()].to_owned(),
        flavor: Flavor::Tex,
        inline,
    };
    let mut d = Vec::new();
    diagnose(&equation, body, &mut d);
    (equation, d)
}

#[cfg(feature = "math")]
fn diagnose(equation: &Equation, body: Range<usize>, d: &mut Vec<Diagnostic>) {
    if let Err(e) = equation.typeset() {
        let end = body.start + equation.source.len();
        let at = |o: usize| (body.start + o).min(end);
        let (start, stop) = (at(e.span.start), at(e.span.end));
        // A token's span takes the whitespace after it.
        let kept = equation.source.get(start - body.start..stop - body.start);
        let stop = kept.map_or(stop, |t| {
            let trimmed = t.trim_end();
            start
                + if trimmed.is_empty() {
                    t.len()
                } else {
                    trimmed.len()
                }
        });
        d.push(Diagnostic::new(range(start, stop.max(start)), e.message));
    }
}

#[cfg(not(feature = "math"))]
fn diagnose(_: &Equation, body: Range<usize>, d: &mut Vec<Diagnostic>) {
    d.push(Diagnostic::warning(
        range(body.start, body.end),
        "math support is off in this build; the formula is drawn as its source",
    ));
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(
        TextSize::new(u32::try_from(start).unwrap_or(u32::MAX)),
        TextSize::new(u32::try_from(end).unwrap_or(u32::MAX)),
    )
}

/// The result for a document that is not one formula: its text on one line, and one error on its
/// first line.
fn refused(src: &str) -> (Equation, Vec<Diagnostic>) {
    let text = src.trim_start_matches(crate::lines::BOM);
    let start = src.len() - text.len();
    let end = text.find(['\n', '\r']).map_or(src.len(), |i| start + i);
    let source = text.lines().map(str::trim).collect::<Vec<_>>().join(" ");
    let equation = Equation {
        source,
        flavor: Flavor::Tex,
        inline: false,
    };
    (equation, vec![Diagnostic::new(range(start, end), DOCUMENT)])
}

/// The code of each line of `src[within]` as a range of `src`: the line up to its first
/// unescaped `%`, which starts a comment.
fn code(src: &str, within: Range<usize>) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut at = 0;
    for raw in src.split_inclusive('\n') {
        let line = raw.trim_end_matches(['\n', '\r']);
        let (start, end) = (at.max(within.start), (at + comment(line)).min(within.end));
        if start < end {
            out.push(start..end);
        }
        at += raw.len();
    }
    out
}

/// The length of the part of `line` before its comment.
fn comment(line: &str) -> usize {
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'%' => return i,
            _ => i += 1,
        }
    }
    line.len()
}

/// `within` without the whitespace and comment-only lines at both ends, and without a trailing
/// comment.
fn trim(src: &str, within: Range<usize>) -> Range<usize> {
    let (mut first, mut last) = (None, None);
    for r in code(src, within.clone()) {
        let text = &src[r.clone()];
        let kept = text.trim_start();
        if kept.is_empty() {
            continue;
        }
        first.get_or_insert(r.start + text.len() - kept.len());
        last = Some(r.start + text.trim_end().len());
    }
    match (first, last) {
        (Some(a), Some(b)) => a..b,
        _ => within.start..within.start,
    }
}

/// Whether `src` has a preamble or a document in its code.
fn is_document(src: &str) -> bool {
    code(src, 0..src.len()).iter().any(|r| {
        let t = &src[r.clone()];
        t.contains("\\documentclass") || t.contains("\\begin{document}")
    })
}

/// The one math block between `\begin{document}` and `\end{document}`, as the formula's range.
fn document(src: &str) -> Option<Range<usize>> {
    let find = |needle: &str, from: usize| {
        code(src, from..src.len()).into_iter().find_map(|r| {
            src[r.clone()]
                .find(needle)
                .map(|i| (r.start + i, r.start + i + needle.len()))
        })
    };
    let (_, open) = find("\\begin{document}", 0)?;
    let (close, _) = find("\\end{document}", open)?;
    let block = trim(src, open..close);
    let s = &src[block.clone()];
    if let Some(inner) = wrapper(src, block.clone()) {
        return Some(inner);
    }
    // An environment that the typesetter takes whole.
    let name = s.strip_prefix("\\begin{")?.split_once('}')?.0;
    (KEPT.contains(&name) && s.ends_with(&format!("\\end{{{name}}}"))).then_some(block)
}

/// The formula inside the delimiters that make all of `src[r]` one math block: `$$...$$`,
/// `$...$`, `\[...\]`, `\(...\)`, and the `displaymath` and `math` environments.
fn wrapper(src: &str, r: Range<usize>) -> Option<Range<usize>> {
    let s = &src[r.clone()];
    // The delimiters make one block when the closing one is not also inside it.
    let strip = |open: &str, close: &str| {
        let whole = s.len() >= open.len() + close.len()
            && s.starts_with(open)
            && s.ends_with(close)
            && !s[open.len()..s.len() - close.len()].contains(close);
        whole.then(|| r.start + open.len()..r.end - close.len())
    };
    let dollars = |n: usize| {
        let inner = strip(&"$".repeat(n), &"$".repeat(n))?;
        let escaped = backslashes(&s[..s.len() - n]) % 2 == 1;
        (!escaped && !has_dollar(src, inner.clone())).then_some(inner)
    };
    dollars(2)
        .or_else(|| dollars(1))
        .or_else(|| strip("\\[", "\\]"))
        .or_else(|| strip("\\(", "\\)"))
        .or_else(|| strip("\\begin{displaymath}", "\\end{displaymath}"))
        .or_else(|| strip("\\begin{math}", "\\end{math}"))
}

/// How many backslashes end `s`.
fn backslashes(s: &str) -> usize {
    s.bytes().rev().take_while(|b| *b == b'\\').count()
}

/// Whether the code of `src[r]` has an unescaped `$`.
fn has_dollar(src: &str, r: Range<usize>) -> bool {
    code(src, r).iter().any(|r| {
        let b = src[r.clone()].as_bytes();
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                b'\\' => i += 2,
                b'$' => return true,
                _ => i += 1,
            }
        }
        false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_and_trimming() {
        assert_eq!(comment("a % b"), 2);
        assert_eq!(comment("a \\% b"), 6);
        assert_eq!(comment("a \\\\% b"), 4);
        assert_eq!(comment("\u{e9}\\"), 3);
        assert_eq!(trim("% c\n  $x$  % d\n\n", 0..16), 6..9);
        assert_eq!(trim("  \n", 0..3), 0..0);
    }

    #[test]
    fn wrappers() {
        for (src, inner) in [
            ("$$ x $$", " x "),
            ("$x$", "x"),
            ("\\[x\\]", "x"),
            ("\\(x\\)", "x"),
            ("\\begin{math}x\\end{math}", "x"),
            ("\\begin{displaymath}x\\end{displaymath}", "x"),
        ] {
            let r = wrapper(src, 0..src.len()).expect(src);
            assert_eq!(&src[r], inner, "{src}");
        }
        for src in ["x", "$a$ b $c$", "$x\\$", "$$a$ b$$", "\\[x"] {
            assert_eq!(wrapper(src, 0..src.len()), None, "{src}");
        }
    }
}
