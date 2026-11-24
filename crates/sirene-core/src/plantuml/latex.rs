//! PlantUML `@startlatex` (PCREO-3): the body between the markers is one formula, set in
//! display style. Its lines are trimmed and joined with a space each, and `'` starts no comment.

use rowan::TextRange;

use super::lines;
use crate::diagnostic::Diagnostic;
use crate::equation::Equation;
use crate::syntax::SyntaxNode;

pub(crate) fn lower(root: &SyntaxNode) -> (Equation, Vec<Diagnostic>) {
    let body = lines::read(root);
    let source = body
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let mut d = Vec::new();
    if let (Some(first), Some(last)) = (body.first(), body.last()) {
        diagnose(&body, &source, first.range.cover(last.range), &mut d);
    }
    (Equation { source }, d)
}

#[cfg(feature = "math")]
fn diagnose(body: &[lines::Line], source: &str, _: TextRange, d: &mut Vec<Diagnostic>) {
    use crate::math::{MathStyle, typeset};
    if let Err(e) = typeset(source, MathStyle::Display) {
        let range = TextRange::new(
            position(body, e.span.start, false),
            position(body, e.span.end, true),
        );
        d.push(Diagnostic::new(range, e.message));
    }
}

#[cfg(not(feature = "math"))]
fn diagnose(_: &[lines::Line], _: &str, whole: TextRange, d: &mut Vec<Diagnostic>) {
    d.push(Diagnostic::warning(
        whole,
        "math support is off in this build; the formula is drawn as its source",
    ));
}

/// The source position of byte `at` of the joined body. The end of a range belongs to the line
/// it ends in, not to the one it touches.
#[cfg(feature = "math")]
fn position(body: &[lines::Line], at: usize, end: bool) -> rowan::TextSize {
    use rowan::TextSize;
    let mut start = 0;
    let mut found = None;
    for line in body {
        let reached = if end { start < at } else { start <= at };
        if found.is_some() && !reached {
            break;
        }
        found = Some((start, line));
        start += line.text.len() + 1;
    }
    let Some((start, line)) = found else {
        return TextSize::new(0);
    };
    let off = at.saturating_sub(start).min(line.text.len());
    line.range.start() + TextSize::new(off as u32)
}
