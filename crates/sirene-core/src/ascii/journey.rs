//! User journeys as text (MJOUR-4): each section's tasks with a score bar and their actors.

use super::{Charset, Options, width};
use crate::journey::{Journey, MAX_SCORE};

/// A five-step bar for `score`, rounded to the nearest step.
fn bar(score: f64, charset: Charset) -> String {
    let (on, off) = match charset {
        Charset::Unicode => ('●', '○'),
        Charset::Ascii => ('*', '.'),
    };
    let n = score.round().clamp(0.0, MAX_SCORE) as usize;
    let mut out: String = std::iter::repeat_n(on, n).collect();
    out.extend(std::iter::repeat_n(off, MAX_SCORE as usize - n));
    out
}

/// Renders `journey` as text (MJOUR-4).
pub fn render(journey: &Journey, options: &Options) -> String {
    let named = journey.sections.iter().any(|s| s.name.is_some());
    let indent = if named { "  " } else { "" };
    let name_w = journey
        .tasks()
        .map(|(_, t)| width(&t.name))
        .max()
        .unwrap_or(0);
    let score_w = journey
        .tasks()
        .map(|(_, t)| t.score.to_string().len())
        .max()
        .unwrap_or(0);
    let mut rows = Vec::new();
    for s in &journey.sections {
        if s.tasks.is_empty() {
            continue;
        }
        if let Some(n) = &s.name {
            rows.push(n.clone());
        }
        for t in &s.tasks {
            let pad = " ".repeat((name_w - width(&t.name)) as usize);
            rows.push(format!(
                "{indent}{}{pad}  {} {:<score_w$}  {}",
                t.name,
                bar(t.score, options.charset),
                t.score.to_string(),
                t.actors.join(", ")
            ));
        }
    }
    let mut out = String::new();
    if let Some(t) = journey
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let w = rows.iter().map(|r| width(r.trim_end())).max().unwrap_or(0);
        let pad = (w - width(t)).max(0) / 2;
        out.push_str(&" ".repeat(pad as usize));
        out.push_str(t);
        out.push('\n');
        if !rows.is_empty() {
            out.push('\n');
        }
    }
    for r in rows {
        out.push_str(r.trim_end());
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars() {
        assert_eq!(bar(5.0, Charset::Unicode), "●●●●●");
        assert_eq!(bar(2.5, Charset::Ascii), "***..");
        assert_eq!(bar(1.0, Charset::Ascii), "*....");
    }
}
