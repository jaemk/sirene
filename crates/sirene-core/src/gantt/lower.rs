//! Collects a gantt chart's settings and sections, and dates its tasks (MGANTT-2, MGANTT-3).

use std::collections::HashMap;

use rowan::TextRange;

use super::parser::text_of;
use super::time::{self, Interval, Time};
use super::{Gantt, Section, Tags, Task, TodayMarker};
use crate::diagnostic::Diagnostic;
use crate::flowchart::text;
use crate::syntax::SyntaxKind::*;
use crate::syntax::{SyntaxNode, SyntaxToken};

/// When a task starts, as written.
enum Start {
    /// The end of the task before it in the source.
    Previous,
    Date(String),
    After(Vec<String>),
}

struct Raw {
    section: usize,
    name: String,
    id: Option<String>,
    tags: Tags,
    start: Start,
    end: String,
    /// The task before this one in the source.
    previous: Option<usize>,
    range: TextRange,
}

pub(crate) fn lower(root: &SyntaxNode, now: Time) -> (Gantt, Vec<Diagnostic>) {
    let mut g = Gantt {
        now,
        ..Gantt::default()
    };
    let mut diagnostics = Vec::new();
    let mut title = None;
    for el in root.children_with_tokens() {
        let Some(tok) = el.as_token() else { continue };
        match tok.kind() {
            FRONTMATTER => {
                title = text::frontmatter_title(tok.text());
                for (k, v) in frontmatter_config(tok.text()) {
                    apply_config(&mut g, &k, &v, tok, &mut diagnostics);
                }
            }
            DIRECTIVE => {
                for (k, v) in directive_config(tok.text()) {
                    apply_config(&mut g, &k, &v, tok, &mut diagnostics);
                }
            }
            _ => {}
        }
    }
    let mut raws: Vec<Raw> = Vec::new();
    for stmt in root.children() {
        match stmt.kind() {
            TITLE_STMT => title = text_of(&stmt, TEXT).filter(|s| !s.is_empty()),
            SECTION_STMT => g.sections.push(Section {
                name: text_of(&stmt, TEXT).filter(|s| !s.is_empty()),
                tasks: Vec::new(),
            }),
            SETTING_STMT => setting(&mut g, &stmt, &mut diagnostics),
            CLICK_STMT => diagnostics.push(Diagnostic::warning(
                stmt.text_range(),
                "`click` is not supported in gantt charts; ignoring it",
            )),
            TASK_STMT => {
                if g.sections.is_empty() {
                    g.sections.push(Section::default());
                }
                let previous = raws.len().checked_sub(1);
                if let Some(raw) = task(&stmt, g.sections.len() - 1, previous, &mut diagnostics) {
                    raws.push(raw);
                }
            }
            ACC_TITLE_STMT => g.acc_title = text_of(&stmt, ACC_TEXT),
            ACC_DESCR_STMT => g.acc_descr = text_of(&stmt, ACC_TEXT),
            _ => {}
        }
    }
    g.title = title;
    let dated = date_tasks(&g, &raws, &mut diagnostics);
    for (raw, dates) in raws.into_iter().zip(dated) {
        if let Some((start, end, drawn_end)) = dates {
            g.sections[raw.section].tasks.push(Task {
                name: raw.name,
                id: raw.id,
                tags: raw.tags,
                start,
                end,
                drawn_end,
            });
        }
    }
    (g, diagnostics)
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    for q in ['"', '\''] {
        if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
            return s[1..s.len() - 1].to_owned();
        }
    }
    s.to_owned()
}

/// The `key: value` lines under `config:` `gantt:` in a frontmatter block (MGANTT-2).
fn frontmatter_config(block: &str) -> Vec<(String, String)> {
    text::frontmatter_config(block, "gantt")
}

/// The `key: value` pairs of the `gantt` object in an `%%{init: ...}%%` directive (MGANTT-2).
fn directive_config(directive: &str) -> Vec<(String, String)> {
    let Some(at) = directive.find("gantt") else {
        return Vec::new();
    };
    let rest = &directive[at..];
    let (Some(open), Some(close)) = (rest.find('{'), rest.find('}')) else {
        return Vec::new();
    };
    if close < open {
        return Vec::new();
    }
    rest[open + 1..close]
        .split(',')
        .filter_map(|pair| {
            let (k, v) = pair.split_once(':')?;
            Some((unquote(k), unquote(v)))
        })
        .collect()
}

fn apply_config(
    g: &mut Gantt,
    key: &str,
    value: &str,
    tok: &SyntaxToken,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match key {
        "dateFormat" => g.date_format = value.to_owned(),
        "axisFormat" => g.axis_format = value.to_owned(),
        "tickInterval" => match Interval::parse(value) {
            Some(i) => g.tick_interval = Some(i),
            None => diagnostics.push(Diagnostic::new(tok.text_range(), bad_interval(value))),
        },
        "topAxis" => g.top_axis = value == "true",
        "displayMode" => g.compact = value == "compact",
        "weekday" => match time::weekday_of(value) {
            Some(d) => g.weekday = d,
            None => diagnostics.push(Diagnostic::new(tok.text_range(), bad_weekday(value))),
        },
        _ => {}
    }
}

fn bad_interval(v: &str) -> String {
    format!("`{v}` is not an interval; expected a number and a unit, such as `1day` or `2week`")
}

fn bad_weekday(v: &str) -> String {
    format!("`{v}` is not a weekday; expected one such as `monday`")
}

fn setting(g: &mut Gantt, stmt: &SyntaxNode, diagnostics: &mut Vec<Diagnostic>) {
    let Some(kw) = text_of(stmt, SETTING_KW) else {
        return;
    };
    let value = text_of(stmt, TEXT).unwrap_or_default();
    let error = |diagnostics: &mut Vec<Diagnostic>, message: String| {
        diagnostics.push(Diagnostic::new(stmt.text_range(), message));
    };
    let list = |v: &str| -> Vec<String> {
        v.to_lowercase()
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect()
    };
    match kw.to_ascii_lowercase().as_str() {
        "dateformat" => g.date_format = value,
        "axisformat" => g.axis_format = value,
        "tickinterval" => match Interval::parse(&value) {
            Some(i) => g.tick_interval = Some(i),
            None => error(diagnostics, bad_interval(&value)),
        },
        "excludes" => g.excludes = list(&value),
        "includes" => g.includes = list(&value),
        "todaymarker" => {
            g.today_marker = if value.eq_ignore_ascii_case("off") {
                TodayMarker::Off
            } else {
                TodayMarker::Style(value)
            }
        }
        "weekday" => match time::weekday_of(&value) {
            Some(d) => g.weekday = d,
            None => error(diagnostics, bad_weekday(&value)),
        },
        "weekend" => match value.to_ascii_lowercase().as_str() {
            "friday" => g.weekend = 5,
            "saturday" => g.weekend = 6,
            _ => error(
                diagnostics,
                format!("`{value}` cannot start the weekend; expected `friday` or `saturday`"),
            ),
        },
        "inclusiveenddates" => g.inclusive_end_dates = true,
        "topaxis" => g.top_axis = true,
        _ => {}
    }
}

/// A task statement as written: its tags, id, start, and end (MGANTT-1).
fn task(
    stmt: &SyntaxNode,
    section: usize,
    previous: Option<usize>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Raw> {
    let name = text_of(stmt, TASK_NAME)?;
    let data = text_of(stmt, TASK_DATA).unwrap_or_default();
    let range = stmt.text_range();
    let mut fields: Vec<&str> = data.split(',').map(str::trim).collect();
    let mut tags = Tags::default();
    loop {
        let tag = match fields.first() {
            Some(&"active") => &mut tags.active,
            Some(&"done") => &mut tags.done,
            Some(&"crit") => &mut tags.crit,
            Some(&"milestone") => &mut tags.milestone,
            Some(&"vert") => &mut tags.vert,
            _ => break,
        };
        *tag = true;
        fields.remove(0);
    }
    let start_of = |s: &str| match s.strip_prefix("after") {
        Some(ids) if ids.starts_with(char::is_whitespace) => {
            Start::After(ids.split_whitespace().map(str::to_owned).collect())
        }
        _ => Start::Date(s.to_owned()),
    };
    let (id, start, end) = match fields.as_slice() {
        [""] | [] => {
            diagnostics.push(Diagnostic::new(range, "expected the task's dates"));
            return None;
        }
        [end] => {
            if previous.is_none() {
                diagnostics.push(Diagnostic::new(
                    range,
                    format!("`{name}` has no start and no task before it"),
                ));
                return None;
            }
            (None, Start::Previous, *end)
        }
        [start, end] => (None, start_of(start), *end),
        [id, start, end] => (Some((*id).to_owned()), start_of(start), *end),
        _ => {
            diagnostics.push(Diagnostic::new(
                range,
                "expected at most an id, a start, and an end",
            ));
            return None;
        }
    };
    Some(Raw {
        section,
        name,
        id: id.filter(|i| !i.is_empty()),
        tags,
        start,
        end: end.to_owned(),
        previous,
        range,
    })
}

/// A task's start, end, and drawn end.
type Dates = (Time, Time, Time);

/// Each task's dates, or `None` when it cannot be dated (MGANTT-3).
fn date_tasks(g: &Gantt, raws: &[Raw], diagnostics: &mut Vec<Diagnostic>) -> Vec<Option<Dates>> {
    let mut ids: HashMap<&str, usize> = HashMap::new();
    let mut failed = vec![false; raws.len()];
    for (i, r) in raws.iter().enumerate() {
        if let Some(id) = r.id.as_deref() {
            if ids.contains_key(id) {
                diagnostics.push(Diagnostic::new(
                    r.range,
                    format!("`{id}` is already a task id"),
                ));
                failed[i] = true;
            } else {
                ids.insert(id, i);
            }
        }
    }
    let date = |s: &str| time::parse_dayjs(s, &g.date_format, g.now).or_else(|| time::parse_iso(s));
    let mut out: Vec<Option<Dates>> = vec![None; raws.len()];
    // Whether every task `names` refers to is known; reports the unknown ones.
    let known = |r: &Raw, names: &[String], diagnostics: &mut Vec<Diagnostic>| -> bool {
        let mut ok = true;
        for n in names {
            if !ids.contains_key(n.as_str()) {
                diagnostics.push(Diagnostic::new(
                    r.range,
                    format!("no task has the id `{n}`"),
                ));
                ok = false;
            }
        }
        ok
    };
    // Check references once, up front.
    for (i, r) in raws.iter().enumerate() {
        if failed[i] {
            continue;
        }
        if let Start::After(names) = &r.start
            && !known(r, names, diagnostics)
        {
            failed[i] = true;
        }
        if let Some(names) = until(&r.end)
            && !known(r, &names, diagnostics)
        {
            failed[i] = true;
        }
    }
    loop {
        let mut progress = false;
        for (i, r) in raws.iter().enumerate() {
            if failed[i] || out[i].is_some() {
                continue;
            }
            // Ok(Some) when known, Ok(None) when waiting on another task, Err when it cannot be.
            let lookup = |names: &[String]| -> Result<Option<Vec<Dates>>, ()> {
                let mut v = Vec::new();
                for n in names {
                    let j = ids[n.as_str()];
                    if failed[j] {
                        return Err(());
                    }
                    match out[j] {
                        Some(d) => v.push(d),
                        None => return Ok(None),
                    }
                }
                Ok(Some(v))
            };
            let start = match &r.start {
                Start::Previous => {
                    let j = r.previous.expect("checked when read");
                    if failed[j] {
                        failed[i] = true;
                        continue;
                    }
                    match out[j] {
                        Some(d) => d.1,
                        None => continue,
                    }
                }
                Start::After(names) => match lookup(names) {
                    Ok(Some(v)) => v.iter().map(|d| d.1).max().expect("ids given"),
                    Ok(None) => continue,
                    Err(()) => {
                        failed[i] = true;
                        continue;
                    }
                },
                Start::Date(s) => match date(s) {
                    Some(t) => t,
                    None => {
                        diagnostics.push(Diagnostic::new(
                            r.range,
                            format!("`{s}` is not a date in `{}`", g.date_format),
                        ));
                        failed[i] = true;
                        continue;
                    }
                },
            };
            let (end, manual) = if let Some(names) = until(&r.end) {
                match lookup(&names) {
                    Ok(Some(v)) => (v.iter().map(|d| d.0).min().expect("ids given"), false),
                    Ok(None) => continue,
                    Err(()) => {
                        failed[i] = true;
                        continue;
                    }
                }
            } else if let Some(t) = time::parse_dayjs(&r.end, &g.date_format, g.now) {
                let t = if g.inclusive_end_dates {
                    t + time::DAY
                } else {
                    t
                };
                (t, true)
            } else if let Some(t) = time::add_duration(start, &r.end) {
                (t, false)
            } else {
                diagnostics.push(Diagnostic::new(
                    r.range,
                    format!(
                        "`{}` is not an end date in `{}`, a duration such as `3d`, or `until ID`",
                        r.end, g.date_format
                    ),
                ));
                failed[i] = true;
                continue;
            };
            out[i] = Some(if manual || g.excludes.is_empty() {
                (start, end, end)
            } else {
                let (end, drawn) = skip_excluded(g, start, end);
                (start, end, drawn)
            });
            progress = true;
        }
        if !progress {
            break;
        }
    }
    for (i, r) in raws.iter().enumerate() {
        if out[i].is_none() && !failed[i] {
            diagnostics.push(Diagnostic::new(
                r.range,
                format!(
                    "`{}` cannot be dated: its `after` and `until` tasks wait on each other",
                    r.name
                ),
            ));
        }
    }
    out
}

/// The ids of an `until ID ID...` end.
fn until(end: &str) -> Option<Vec<String>> {
    let ids = end.strip_prefix("until")?;
    ids.starts_with(char::is_whitespace)
        .then(|| ids.split_whitespace().map(str::to_owned).collect())
}

/// Mermaid's `fixTaskDates`: each excluded day after the start moves the end a day later; the
/// bar ends at the last end that was not moved. Returns the end and the drawn end.
fn skip_excluded(g: &Gantt, start: Time, mut end: Time) -> (Time, Time) {
    let mut day = start + time::DAY;
    let mut excluded = false;
    let mut drawn = end;
    while day <= end {
        if !excluded {
            drawn = end;
        }
        excluded = g.is_excluded(day);
        if excluded {
            end += time::DAY;
        }
        day += time::DAY;
    }
    (end, drawn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configs() {
        let block = "---\ntitle: T\nconfig:\n  theme: dark\n  gantt:\n    axisFormat: \"%m/%d\"\n    topAxis: true\n  other:\n    weekday: friday\n---";
        assert_eq!(
            frontmatter_config(block),
            [
                ("axisFormat".to_owned(), "%m/%d".to_owned()),
                ("topAxis".to_owned(), "true".to_owned())
            ]
        );
        assert!(frontmatter_config("---\ntitle: x\n---").is_empty());
        let d = "%%{init: {'gantt': {'tickInterval': '1week', \"displayMode\": compact}}}%%";
        assert_eq!(
            directive_config(d),
            [
                ("tickInterval".to_owned(), "1week".to_owned()),
                ("displayMode".to_owned(), "compact".to_owned())
            ]
        );
        assert!(directive_config("%%{init: {}}%%").is_empty());
        assert!(directive_config("%%{init: {'theme': 'x'}} gantt %%").is_empty());
    }
}
