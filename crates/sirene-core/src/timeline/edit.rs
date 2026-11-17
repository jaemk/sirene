//! Drawing edits to timeline source (WYSIWY-27): adding, renaming, and deleting time periods and
//! their events. Periods are named by their place among the periods, and events by their place
//! among every period's events, in order.

use std::ops::Range;

use super::{Text, Timeline};
use crate::edits::{combine, indent, line_start, newline, normalize, span, statement_span};
use crate::flowchart::edit::TextEdit;
use crate::syntax::SyntaxKind::*;
use crate::syntax::{SyntaxNode, SyntaxToken};

/// A period's statements: its own and the `: EVENT` lines after it, with its event tokens.
struct Group {
    statements: Vec<SyntaxNode>,
    events: Vec<SyntaxToken>,
}

struct Doc {
    groups: Vec<Group>,
    timeline: Timeline,
}

impl Doc {
    fn new(src: &str) -> Doc {
        let parse = super::parse(src);
        let mut groups: Vec<Group> = Vec::new();
        for stmt in parse.syntax().children() {
            match stmt.kind() {
                PERIOD_STMT => groups.push(Group {
                    events: events(&stmt),
                    statements: vec![stmt],
                }),
                EVENTS_STMT => {
                    if let Some(g) = groups.last_mut() {
                        g.events.extend(events(&stmt));
                        g.statements.push(stmt);
                    }
                }
                _ => {}
            }
        }
        Doc {
            groups,
            timeline: parse.lower().0,
        }
    }

    /// The period and place within it of event `n`.
    fn event(&self, n: usize) -> Option<(usize, usize)> {
        let mut n = n;
        for (i, g) in self.groups.iter().enumerate() {
            if n < g.events.len() {
                return Some((i, n));
            }
            n -= g.events.len();
        }
        None
    }

    /// The bytes period `i` takes: its line and its `: EVENT` lines.
    fn group_span(&self, src: &str, i: usize) -> Range<usize> {
        let g = &self.groups[i];
        let start = statement_span(src, g.statements[0].text_range()).start;
        let end = statement_span(src, g.statements[g.statements.len() - 1].text_range()).end;
        start..end
    }
}

/// The event tokens of a statement, as lowering reads them: the token after each `:`.
fn events(stmt: &SyntaxNode) -> Vec<SyntaxToken> {
    let mut out = Vec::new();
    let mut tokens = (stmt.children_with_tokens())
        .filter_map(|e| e.into_token())
        .peekable();
    while let Some(t) = tokens.next() {
        if t.kind() != COLON {
            continue;
        }
        while tokens.peek().is_some_and(|n| n.kind() == WHITESPACE) {
            tokens.next();
        }
        if let Some(e) = tokens.peek().filter(|e| e.kind() == EVENT) {
            out.push(e.clone());
        }
    }
    out
}

/// Source text for `text`, lines joined with `<br>`: `None` when it is empty or would not read
/// back as written (a `:` before whitespace or at the end separates events; timelines have no
/// escapes).
fn text_source(text: &str) -> Option<String> {
    let text = normalize(text);
    let ok = !text.is_empty()
        && text
            .lines()
            .all(|l| !l.ends_with(':') && !l.contains(": ") && !l.contains(":\t"));
    ok.then(|| text.replace('\n', "<br>"))
}

/// The lines `text` reads back as.
fn lines(text: &str) -> Text {
    normalize(text).lines().map(str::to_owned).collect()
}

/// The source of period `i`: its line.
pub fn period_range(src: &str, i: usize) -> Option<Range<usize>> {
    let doc = Doc::new(src);
    Some(span(doc.groups.get(i)?.statements[0].text_range()))
}

/// The source of event `n`: its text.
pub fn event_range(src: &str, n: usize) -> Option<Range<usize>> {
    let doc = Doc::new(src);
    let (i, k) = doc.event(n)?;
    Some(span(doc.groups[i].events[k].text_range()))
}

/// The edit `edit`, when afterwards `check` holds of the timeline.
fn checked(src: &str, edit: TextEdit, check: impl Fn(&Timeline) -> bool) -> Option<TextEdit> {
    let after = super::parse(&edit.apply(src)).lower().0;
    check(&after).then_some(edit)
}

fn period(t: &Timeline, i: usize) -> Option<&super::Period> {
    t.periods().nth(i).map(|(_, p)| p)
}

fn event(t: &Timeline, n: usize) -> Option<&Text> {
    t.periods().flat_map(|(_, p)| &p.events).nth(n)
}

/// Shows period `i` as `text`. `None` when there is no such period, `text` can't be written, or
/// nothing changes.
pub fn set_period_label(src: &str, i: usize, text: &str) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let token = (doc.groups.get(i)?.statements[0].children_with_tokens())
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == PERIOD)?;
    let want = lines(text);
    if period(&doc.timeline, i).is_some_and(|p| p.label == want) {
        return None;
    }
    let edit = TextEdit {
        range: span(token.text_range()),
        text: text_source(text)?,
    };
    let count = doc.timeline.periods().count();
    checked(src, edit, |t| {
        t.periods().count() == count && period(t, i).is_some_and(|p| p.label == want)
    })
}

/// Sets the text of event `n`. `None` when there is no such event, `text` can't be written, or
/// nothing changes.
pub fn set_event_text(src: &str, n: usize, text: &str) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let (i, k) = doc.event(n)?;
    let want = lines(text);
    if event(&doc.timeline, n) == Some(&want) {
        return None;
    }
    let edit = TextEdit {
        range: span(doc.groups[i].events[k].text_range()),
        text: text_source(text)?,
    };
    let count = doc.timeline.periods().count();
    checked(src, edit, |t| {
        t.periods().count() == count && event(t, n) == Some(&want)
    })
}

/// Adds a period shown as `text` after period `after` and its events, or after the last
/// statement. Returns its index and the edit; `None` when `text` can't be written or there is no
/// such period.
pub fn add_period(src: &str, after: Option<usize>, text: &str) -> Option<(usize, TextEdit)> {
    let doc = Doc::new(src);
    let line = text_source(text)?;
    let nl = newline(src);
    let (i, at, pad) = match after {
        Some(a) => {
            let g = doc.groups.get(a)?;
            let start = usize::from(g.statements[0].text_range().start());
            (
                a + 1,
                doc.group_span(src, a).end,
                indent(src, start).to_owned(),
            )
        }
        None => {
            let pad = doc.groups.last().map_or("    ".to_owned(), |g| {
                indent(src, usize::from(g.statements[0].text_range().start())).to_owned()
            });
            (doc.groups.len(), src.len(), pad)
        }
    };
    let lead = if src[..at].ends_with('\n') || at == 0 {
        ""
    } else {
        nl
    };
    let edit = TextEdit {
        range: at..at,
        text: format!("{lead}{pad}{line}{nl}"),
    };
    let (count, want) = (doc.timeline.periods().count(), lines(text));
    let edit = checked(src, edit, |t| {
        t.periods().count() == count + 1 && period(t, i).is_some_and(|p| p.label == want)
    })?;
    Some((i, edit))
}

/// Adds an event `text` at the end of period `i`'s events. Returns its index among all events
/// and the edit; `None` when `text` can't be written or there is no such period.
pub fn add_event(src: &str, i: usize, text: &str) -> Option<(usize, TextEdit)> {
    let doc = Doc::new(src);
    let g = doc.groups.get(i)?;
    let last = &g.statements[g.statements.len() - 1];
    let end = usize::from(last.text_range().end());
    let end = src[..end].trim_end().len();
    let edit = TextEdit {
        range: end..end,
        text: format!(" : {}", text_source(text)?),
    };
    let n = doc.groups[..=i]
        .iter()
        .map(|g| g.events.len())
        .sum::<usize>();
    let want = lines(text);
    let edit = checked(src, edit, |t| event(t, n) == Some(&want))?;
    Some((n, edit))
}

/// Removes periods `periods`, each with its events, and events `events`. `None` when nothing is
/// removed.
pub fn delete(src: &str, periods: &[usize], events: &[usize]) -> Option<TextEdit> {
    let doc = Doc::new(src);
    let mut edits: Vec<(Range<usize>, String)> = periods
        .iter()
        .filter(|&&i| i < doc.groups.len())
        .map(|&i| (doc.group_span(src, i), String::new()))
        .collect();
    for &n in events {
        let Some((i, k)) = doc.event(n) else { continue };
        let token = &doc.groups[i].events[k];
        let stmt = token.parent()?;
        let mine = self::events(&stmt);
        // A `: EVENT` line holding only this event goes whole.
        if stmt.kind() == EVENTS_STMT && mine.len() == 1 {
            edits.push((statement_span(src, stmt.text_range()), String::new()));
            continue;
        }
        // From the space before its `:` to its end.
        let colon = (stmt.children_with_tokens())
            .filter_map(|e| e.into_token())
            .filter(|t| t.kind() == COLON && t.text_range().end() <= token.text_range().start())
            .last()?;
        let start = usize::from(colon.text_range().start());
        let start = src[..start].trim_end_matches([' ', '\t']).len();
        let start = start.max(line_start(src, start));
        edits.push((start..usize::from(token.text_range().end()), String::new()));
    }
    edits.sort_by_key(|(r, _)| r.start);
    combine(src, edits)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "timeline\n    title T\n    section S\n    2002 : LinkedIn\n    2004 : Facebook : Google\n         : Flickr\n    2005\n";

    fn labels(src: &str) -> Vec<(Text, Vec<Text>)> {
        let t = super::super::parse(src).lower().0;
        t.periods()
            .map(|(_, p)| (p.label.clone(), p.events.clone()))
            .collect()
    }

    #[test]
    fn ranges() {
        assert_eq!(
            &SRC[period_range(SRC, 1).unwrap()],
            "2004 : Facebook : Google"
        );
        assert_eq!(&SRC[event_range(SRC, 3).unwrap()], "Flickr");
        assert!(event_range(SRC, 4).is_none());
    }

    #[test]
    fn renames() {
        let out = set_period_label(SRC, 0, "2001\nearly").unwrap().apply(SRC);
        assert!(out.contains("    2001<br>early : LinkedIn\n"), "{out}");
        let out = set_event_text(SRC, 2, "Yahoo 10:30").unwrap().apply(SRC);
        assert!(out.contains("2004 : Facebook : Yahoo 10:30\n"), "{out}");
        assert!(set_event_text(SRC, 2, "a: b").is_none());
        assert!(set_event_text(SRC, 2, "a:").is_none());
        assert!(set_event_text(SRC, 2, "Google").is_none());
        assert!(set_period_label(SRC, 0, "section X").is_none());
        assert!(set_period_label(SRC, 0, "").is_none());
        assert!(set_period_label(SRC, 9, "x").is_none());
    }

    #[test]
    fn adds() {
        let (i, e) = add_period(SRC, Some(1), "2003").unwrap();
        let out = e.apply(SRC);
        assert_eq!(i, 2);
        assert!(
            out.contains("         : Flickr\n    2003\n    2005\n"),
            "{out}"
        );
        let (i, e) = add_period(SRC, None, "2006").unwrap();
        assert_eq!(i, 3);
        assert!(e.apply(SRC).ends_with("    2005\n    2006\n"));
        let (n, e) = add_event(SRC, 1, "Yahoo").unwrap();
        let out = e.apply(SRC);
        assert_eq!(n, 4);
        assert!(out.contains("         : Flickr : Yahoo\n"), "{out}");
        assert_eq!(labels(&out)[1].1.len(), 4);
        let (n, e) = add_event(SRC, 2, "x").unwrap();
        assert_eq!(n, 4);
        assert!(e.apply(SRC).ends_with("    2005 : x\n"));
        let (i, e) = add_period("timeline\n", None, "a").unwrap();
        assert_eq!(
            (i, e.apply("timeline\n")),
            (0, "timeline\n    a\n".to_owned())
        );
    }

    #[test]
    fn deletes() {
        let out = delete(SRC, &[1], &[]).unwrap().apply(SRC);
        assert!(out.ends_with("    2002 : LinkedIn\n    2005\n"), "{out}");
        let out = delete(SRC, &[], &[3]).unwrap().apply(SRC);
        assert!(
            out.ends_with("    2004 : Facebook : Google\n    2005\n"),
            "{out}"
        );
        let out = delete(SRC, &[], &[1]).unwrap().apply(SRC);
        assert!(out.contains("    2004 : Google\n"), "{out}");
        let out = delete(SRC, &[], &[0]).unwrap().apply(SRC);
        assert!(out.contains("    2002\n"), "{out}");
        assert_eq!(labels(&out)[0].1.len(), 0);
        assert!(delete(SRC, &[7], &[9]).is_none());
    }
}
