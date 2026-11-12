//! PlantUML Gantt charts (PGANTT-*), lowered to the gantt model (MGANTT-3): tasks placed by
//! durations, dates, and the ends of other tasks, with closed days skipped.

use std::collections::HashMap;

use rowan::TextRange;

use super::lines::{self, Common, Cursor, Line, keyword};
use crate::diagnostic::Diagnostic;
use crate::gantt::{self, Gantt, Interval, Section, Tags, Time, TodayMarker, Unit, time};
use crate::syntax::SyntaxNode;

pub(crate) const STATEMENT: &str =
    "expected a Gantt statement such as `[Task] lasts 3 days` or `Project starts 2026-01-05`";

/// When a task starts or ends (PGANTT-3).
#[derive(Debug, Clone, PartialEq)]
enum When {
    Date(Time),
    /// `D+N`: days after the project's start.
    Day(i64),
    /// The start or end of another task, plus days.
    Ref {
        target: String,
        end: bool,
        days: i64,
    },
}

#[derive(Debug, Default)]
struct Task {
    name: String,
    section: usize,
    start: Option<When>,
    end: Option<When>,
    /// In days, which skip closed days.
    duration: Option<i64>,
    tags: Tags,
    deleted: bool,
    range: TextRange,
}

struct Lower {
    g: Gantt,
    d: Vec<Diagnostic>,
    tasks: Vec<Task>,
    /// Task names and aliases to tasks.
    index: HashMap<String, usize>,
    project: Option<Time>,
    today: Option<When>,
    /// The section new tasks go in.
    section: usize,
}

/// A `[name]` at the start of `s`, and the rest.
fn bracket(s: &str) -> Option<(&str, &str)> {
    let body = s.trim_start().strip_prefix('[')?;
    let end = body.find(']')?;
    Some((body[..end].trim(), body[end + 1..].trim_start()))
}

/// A `YYYY-MM-DD` (or `YYYY/MM/DD`) date at the start of `s`, and the rest.
fn date(s: &str) -> Option<(Time, &str)> {
    let s = s.trim_start();
    let end = s.find(char::is_whitespace).unwrap_or(s.len());
    let t = time::parse_iso(&s[..end].replace('/', "-"))?;
    Some((t, s[end..].trim_start()))
}

/// `N day(s)` or `N week(s)` at the start of `s` as days, and the rest.
fn days(s: &str) -> Option<(i64, &str)> {
    let s = s.trim_start();
    let n_end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let n: i64 = s[..n_end].parse().ok()?;
    let rest = s[n_end..].trim_start();
    let unit = lines::word(rest);
    let per = match unit.to_ascii_lowercase().as_str() {
        "day" | "days" => 1,
        "week" | "weeks" => 7,
        _ => return None,
    };
    Some((n.min(100_000) * per, rest[unit.len()..].trim_start()))
}

/// A `when` after `starts`, `ends`, or `happens`: a date, `D+N`, `[X]'s end`, or
/// `N days after [X]'s end`.
fn when(s: &str) -> Option<When> {
    let mut s = s.trim();
    for filler in ["at", "on", "the"] {
        if let Some(r) = keyword(s, filler) {
            s = r;
        }
    }
    let reference = |s: &str, days: i64| {
        let (target, rest) = bracket(s)?;
        let rest = rest.strip_prefix("'s").unwrap_or(rest).trim();
        let end = match rest.to_ascii_lowercase().as_str() {
            "end" => true,
            "start" => false,
            _ => return None,
        };
        Some(When::Ref {
            target: target.to_owned(),
            end,
            days,
        })
    };
    if s.starts_with('[') {
        return reference(s, 0);
    }
    if let Some(n) = s.strip_prefix("D+").or_else(|| s.strip_prefix("d+")) {
        return n.trim().parse().ok().map(When::Day);
    }
    if let Some((n, rest)) = days(s) {
        let (sign, rest) = match lines::word(rest).to_ascii_lowercase().as_str() {
            "after" => (1, keyword(rest, "after")?),
            "before" => (-1, keyword(rest, "before")?),
            _ => return None,
        };
        return reference(rest, sign * n);
    }
    match date(s) {
        Some((t, "")) => Some(When::Date(t)),
        _ => None,
    }
}

pub(crate) fn lower(root: &SyntaxNode) -> (Gantt, Vec<Diagnostic>) {
    let mut l = Lower {
        g: Gantt {
            today_marker: TodayMarker::Off,
            ..Gantt::default()
        },
        d: Vec::new(),
        tasks: Vec::new(),
        index: HashMap::new(),
        project: None,
        today: None,
        section: 0,
    };
    l.g.sections.push(Section::default());
    let mut cursor = Cursor::new(lines::read(root));
    while let Some(line) = cursor.next() {
        l.statement(&line, &mut cursor);
    }
    l.finish()
}

impl Lower {
    fn error(&mut self, range: TextRange, message: impl Into<String>) {
        self.d.push(Diagnostic::new(range, message));
    }

    fn warn(&mut self, range: TextRange, message: impl Into<String>) {
        self.d.push(Diagnostic::warning(range, message));
    }

    /// The task named `name`, created in the current section if new.
    fn task(&mut self, name: &str, range: TextRange) -> usize {
        if let Some(&i) = self.index.get(name) {
            return i;
        }
        let i = self.tasks.len();
        self.tasks.push(Task {
            name: name.to_owned(),
            section: self.section,
            range,
            ..Task::default()
        });
        self.index.insert(name.to_owned(), i);
        i
    }

    fn statement(&mut self, line: &Line, cursor: &mut Cursor) {
        let t = line.text.as_str();
        match lines::common(line, cursor, &mut self.d) {
            Common::No => {}
            Common::Done => return,
            Common::Title(title) => {
                self.g.title = Some(title);
                return;
            }
            Common::Direction(_) => {
                return self.warn(line.range, "a Gantt chart runs left to right; ignoring it");
            }
        }
        if let Some(name) = t.strip_prefix("--").and_then(|r| r.strip_suffix("--")) {
            self.g.sections.push(Section {
                name: Some(name.trim().to_owned()),
                tasks: Vec::new(),
            });
            self.section = self.g.sections.len() - 1;
            return;
        }
        if t.starts_with('[') {
            if t.contains("->") {
                return self.arrows(line);
            }
            return self.task_line(line, t, None);
        }
        if let Some(rest) = keyword(t, "then") {
            let previous = self.tasks.len().checked_sub(1);
            return self.task_line(line, rest, previous);
        }
        let squeezed = line.squeezed();
        if let Some(rest) = keyword(t, "project").and_then(|r| keyword(r, "starts")) {
            return match when(rest) {
                Some(When::Date(d)) => self.project = Some(d),
                _ => self.error(line.range, "expected a date such as `2026-01-05`"),
            };
        }
        if let Some(rest) = keyword(t, "today").and_then(|r| keyword(r, "is")) {
            let rest = rest.trim();
            let today = match days(rest) {
                Some((n, r)) if lines::squeeze(r) == "after start" => Some(When::Day(n)),
                _ => when(rest),
            };
            match today {
                Some(w) => self.today = Some(w),
                None => self.error(line.range, "expected a date such as `2026-01-05`"),
            }
            return;
        }
        if let Some(scale) = ["printscale", "projectscale", "ganttscale"]
            .iter()
            .find_map(|w| keyword(t, w))
        {
            let unit = match lines::word(scale).to_ascii_lowercase().as_str() {
                "daily" => Some((Unit::Day, 1)),
                "weekly" => Some((Unit::Week, 1)),
                "monthly" => Some((Unit::Month, 1)),
                "quarterly" => Some((Unit::Month, 3)),
                "yearly" => Some((Unit::Month, 12)),
                _ => None,
            };
            match unit {
                Some((unit, step)) => self.g.tick_interval = Some(Interval { unit, step }),
                None => self.error(line.range, "expected `daily`, `weekly`, or `monthly`"),
            }
            return;
        }
        if let Some(head) = squeezed
            .strip_suffix(" are closed")
            .or_else(|| squeezed.strip_suffix(" is closed"))
        {
            return self.closed(line, head, true);
        }
        if let Some(head) = ["is open", "are open", "is opened", "are opened"]
            .iter()
            .find_map(|s| squeezed.strip_suffix(&format!(" {s}")))
        {
            return self.closed(line, head, false);
        }
        if keyword(t, "separator").is_some()
            || keyword(t, "language").is_some()
            || keyword(t, "saturday").is_some() && squeezed.contains("named")
        {
            return self.warn(line.range, format!("`{t}` is not drawn yet; ignoring it"));
        }
        if date(t).is_some() && squeezed.contains(" named ") {
            return self.warn(line.range, "named days are not drawn yet; ignoring it");
        }
        self.error(line.range, STATEMENT);
    }

    /// `saturday are closed`, `2026-01-05 is closed`, `2026-01-05 to 2026-01-09 are closed`,
    /// and `... is open` (PGANTT-4).
    fn closed(&mut self, line: &Line, head: &str, closed: bool) {
        let mut out = Vec::new();
        if let Some((from, rest)) = date(head) {
            let to = match keyword(rest, "to") {
                Some(r) => match date(r) {
                    Some((t, "")) => t,
                    _ => return self.error(line.range, "expected a date after `to`"),
                },
                None if rest.is_empty() => from,
                None => return self.error(line.range, STATEMENT),
            };
            let mut d = from;
            while d <= to && out.len() < 10_000 {
                out.push(time::format_dayjs(d, &self.g.date_format));
                d += time::DAY;
            }
        } else {
            for day in head
                .split([',', ' '])
                .filter(|s| !s.is_empty() && *s != "and")
            {
                let day = day.strip_suffix('s').unwrap_or(day);
                if time::weekday_of(day).is_none() {
                    return self.error(line.range, STATEMENT);
                }
                out.push(day.to_owned());
            }
        }
        let list = if closed {
            &mut self.g.excludes
        } else {
            &mut self.g.includes
        };
        list.extend(out);
    }

    /// `[A] -> [B] -> [C]`: each task starts at the end of the one before it.
    fn arrows(&mut self, line: &Line) {
        let mut previous: Option<usize> = None;
        for part in line.text.split("->") {
            let Some((name, "")) = bracket(part.trim_end_matches('-')) else {
                return self.error(line.range, "expected `[Task] -> [Task]`");
            };
            let i = self.task(name, line.range);
            if let Some(p) = previous {
                let target = self.tasks[p].name.clone();
                self.tasks[i].start = Some(When::Ref {
                    target,
                    end: true,
                    days: 0,
                });
            }
            previous = Some(i);
        }
    }

    /// `[Task] [as [T]] clause and clause ...` (PGANTT-2), after `then` when `previous` is set.
    fn task_line(&mut self, line: &Line, s: &str, previous: Option<usize>) {
        let Some((name, mut rest)) = bracket(s) else {
            return self.error(line.range, STATEMENT);
        };
        let i = self.task(name, line.range);
        if let Some(r) = keyword(rest, "as") {
            let Some((alias, r)) = bracket(r) else {
                return self.error(line.range, "expected `as [alias]`");
            };
            self.index.insert(alias.to_owned(), i);
            rest = r;
        }
        if let Some(p) = previous.filter(|&p| p != i) {
            let target = self.tasks[p].name.clone();
            self.tasks[i].start.get_or_insert(When::Ref {
                target,
                end: true,
                days: 0,
            });
        }
        let lower = rest.to_ascii_lowercase();
        let mut at = 0;
        let mut clauses = Vec::new();
        while let Some(k) = lower[at..].find(" and ") {
            clauses.push(&rest[at..at + k]);
            at += k + 5;
        }
        clauses.push(&rest[at..]);
        for clause in clauses.into_iter().map(str::trim).filter(|c| !c.is_empty()) {
            self.clause(line, i, clause);
        }
    }

    fn clause(&mut self, line: &Line, i: usize, c: &str) {
        let w = lines::word(c).to_ascii_lowercase();
        let rest = c[w.len()..].trim_start();
        let bad = |l: &mut Self| l.error(line.range_of(c), format!("cannot read `{c}`"));
        match w.as_str() {
            "lasts" | "requires" => match days(rest) {
                Some((n, "")) => self.tasks[i].duration = Some(n),
                _ => bad(self),
            },
            "starts" | "ends" | "happens" => {
                let Some(at) = when(rest) else {
                    return bad(self);
                };
                let task = &mut self.tasks[i];
                match w.as_str() {
                    "starts" => task.start = Some(at),
                    "ends" => task.end = Some(at),
                    _ => {
                        task.tags.milestone = true;
                        task.duration = Some(0);
                        task.start = Some(at);
                    }
                }
            }
            "is" => {
                let r = lines::squeeze(rest);
                if let Some(pct) = r
                    .strip_suffix("% completed")
                    .or_else(|| r.strip_suffix("% complete"))
                    .and_then(|p| p.parse::<u32>().ok())
                {
                    let tags = &mut self.tasks[i].tags;
                    tags.done = pct >= 100;
                    tags.active = (1..100).contains(&pct);
                } else if r == "deleted" {
                    self.tasks[i].deleted = true;
                } else if r.starts_with("colored") || r.starts_with("colour") {
                    self.warn(
                        line.range_of(c),
                        "task colors are not drawn yet; ignoring it",
                    );
                } else {
                    bad(self);
                }
            }
            "on" | "displays" | "links" | "pauses" | "occurs" => {
                self.warn(
                    line.range_of(c),
                    format!("`{w}` is not drawn yet; ignoring it"),
                );
            }
            _ => bad(self),
        }
    }

    /// Whether day `t` is closed.
    fn is_closed(&self, t: Time) -> bool {
        !self.g.excludes.is_empty() && self.g.is_excluded(t)
    }

    /// The first open day at or after `t`.
    fn open_from(&self, mut t: Time) -> Time {
        for _ in 0..10_000 {
            if !self.is_closed(t) {
                break;
            }
            t += time::DAY;
        }
        t
    }

    /// `n` open days after `start`: the end of the last one. Closed days are skipped.
    fn add_open(&self, start: Time, n: i64) -> Time {
        let (mut t, mut left) = (start, n);
        let mut guard = 0;
        while left > 0 && guard < 1_000_000 {
            if !self.is_closed(t) {
                left -= 1;
            }
            t += time::DAY;
            guard += 1;
        }
        t
    }

    /// `n` open days before `end`: the start of the first one.
    fn sub_open(&self, end: Time, n: i64) -> Time {
        let (mut t, mut left) = (end, n);
        let mut guard = 0;
        while left > 0 && guard < 1_000_000 {
            t -= time::DAY;
            if !self.is_closed(t) {
                left -= 1;
            }
            guard += 1;
        }
        t
    }

    /// The time `w` names, once what it refers to is placed.
    fn resolve(&self, w: &When, placed: &[Option<(Time, Time)>], project: Time) -> Option<Time> {
        match w {
            When::Date(t) => Some(*t),
            When::Day(n) => Some(project + n * time::DAY),
            When::Ref { target, end, days } => {
                let (s, e) = placed[*self.index.get(target)?]?;
                Some(if *end { e } else { s } + days * time::DAY)
            }
        }
    }

    /// Places the tasks (PGANTT-3) and builds the chart.
    fn finish(mut self) -> (Gantt, Vec<Diagnostic>) {
        let dated = self.tasks.iter().any(|t| {
            [&t.start, &t.end]
                .into_iter()
                .any(|w| matches!(w, Some(When::Date(_))))
        });
        let project = self.project.unwrap_or_else(|| {
            if !dated {
                // With no dates, the axis counts days from 1.
                self.g.axis_format = "%-j".to_owned();
            }
            self.tasks
                .iter()
                .flat_map(|t| [&t.start, &t.end])
                .filter_map(|w| match w {
                    Some(When::Date(t)) => Some(*t),
                    _ => None,
                })
                .min()
                .unwrap_or(0)
        });
        // An `end` date is the last day of the task, so the task ends after it.
        let end_of = |w: When| match w {
            When::Date(t) => When::Date(t + time::DAY),
            w => w,
        };
        for task in &mut self.tasks {
            task.end = task.end.take().map(end_of);
        }
        for (name, i) in self.index.clone() {
            if self.tasks[i].deleted {
                self.index.remove(&name);
            }
        }
        let n = self.tasks.len();
        let mut placed: Vec<Option<(Time, Time)>> = vec![None; n];
        for _ in 0..=n {
            let mut progress = false;
            for i in 0..n {
                if placed[i].is_some() || self.tasks[i].deleted {
                    continue;
                }
                let task = &self.tasks[i];
                let start = match &task.start {
                    Some(w) => match self.resolve(w, &placed, project) {
                        Some(t) => Some(t),
                        None => continue,
                    },
                    None => None,
                };
                let end = match &task.end {
                    Some(w) => match self.resolve(w, &placed, project) {
                        Some(t) => Some(t),
                        None => continue,
                    },
                    None => None,
                };
                let duration = task.duration;
                let span = match (start, end) {
                    (Some(s), Some(e)) => (s, e.max(s)),
                    (Some(s), None) => {
                        let s = if task.tags.milestone {
                            s
                        } else {
                            self.open_from(s)
                        };
                        (s, self.add_open(s, duration.unwrap_or(1)))
                    }
                    (None, Some(e)) => (self.sub_open(e, duration.unwrap_or(1)), e),
                    (None, None) => {
                        let s = self.open_from(project);
                        (s, self.add_open(s, duration.unwrap_or(1)))
                    }
                };
                placed[i] = Some(span);
                progress = true;
            }
            if !progress {
                break;
            }
        }
        for (i, task) in self.tasks.iter().enumerate() {
            if task.deleted {
                continue;
            }
            let (start, end) = match placed[i] {
                Some(span) => span,
                None => {
                    let missing = [&task.start, &task.end].into_iter().find_map(|w| match w {
                        Some(When::Ref { target, .. }) if !self.index.contains_key(target) => {
                            Some(format!("no task is named `[{target}]`"))
                        }
                        _ => None,
                    });
                    self.d.push(Diagnostic::new(
                        task.range,
                        missing.unwrap_or_else(|| {
                            format!("`[{}]` depends on itself and cannot be placed", task.name)
                        }),
                    ));
                    continue;
                }
            };
            self.g.sections[task.section].tasks.push(gantt::Task {
                name: task.name.clone(),
                id: None,
                tags: task.tags,
                start,
                end,
                drawn_end: end,
            });
        }
        // Day numbers need a tick per day at least: the automatic ticks of a short chart are
        // hours.
        if self.g.axis_format == "%-j"
            && self.g.tick_interval.is_none()
            && self.g.range().is_some_and(|(s, e)| e - s < 15 * time::DAY)
        {
            self.g.tick_interval = Some(Interval {
                unit: Unit::Day,
                step: 1,
            });
        }
        if self.g.sections[0].tasks.is_empty() && self.g.sections.len() > 1 {
            self.g.sections.remove(0);
        }
        if let Some(today) = self.today.take()
            && let Some(t) = self.resolve(&today, &[], project)
        {
            self.g.now = t;
            self.g.today_marker = TodayMarker::Default;
        }
        (self.g, self.d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram::Diagram;

    fn lower_src(body: &str) -> (Gantt, Vec<String>) {
        let src = format!("@startgantt\n{body}\n@endgantt\n");
        let (diagram, d) = super::super::parse(&src).lower();
        let Diagram::Gantt(g) = diagram else {
            panic!("not a gantt chart")
        };
        (g, d.into_iter().map(|d| d.message).collect())
    }

    fn day(s: &str) -> Time {
        time::parse_iso(s).unwrap()
    }

    /// Each task as (name, first day, last day).
    fn spans(g: &Gantt) -> Vec<(String, String, String)> {
        let f = |t: Time| time::format_dayjs(t, "YYYY-MM-DD");
        g.tasks()
            .map(|t| (t.name.clone(), f(t.start), f(t.end - time::DAY)))
            .collect()
    }

    #[test]
    fn whens() {
        assert_eq!(when("2026-01-05"), Some(When::Date(day("2026-01-05"))));
        assert_eq!(when("at 2026/01/05"), Some(When::Date(day("2026-01-05"))));
        assert_eq!(when("D+3"), Some(When::Day(3)));
        assert_eq!(
            when("at [A]'s end"),
            Some(When::Ref {
                target: "A".into(),
                end: true,
                days: 0
            })
        );
        assert_eq!(
            when("2 weeks after [A]'s start"),
            Some(When::Ref {
                target: "A".into(),
                end: false,
                days: 14
            })
        );
        assert_eq!(when("soon"), None);
    }

    #[test]
    fn placing() {
        let (g, d) = lower_src(
            "Project starts 2026-01-05\nsaturday are closed\nsunday are closed\n\
             [Design] as [D] lasts 3 days\nthen [Build] lasts 5 days and is 40% complete\n\
             -- Release --\n[Docs] starts 2026-01-07 and ends 2026-01-08\n\
             [Ship] happens at [Build]'s end\n[Review] lasts 1 day\n[D] -> [Review]\n\
             2026-01-19 is closed\n[Fix] starts 2 days after [Build]'s end and lasts 1 week",
        );
        assert!(d.is_empty(), "{d:?}");
        let s = |a: &str, b: &str, c: &str| (a.to_owned(), b.to_owned(), c.to_owned());
        assert_eq!(
            spans(&g),
            [
                s("Design", "2026-01-05", "2026-01-07"),
                // Five open days from Thursday skip the weekend.
                s("Build", "2026-01-08", "2026-01-14"),
                s("Docs", "2026-01-07", "2026-01-08"),
                // A milestone takes no days.
                s("Ship", "2026-01-15", "2026-01-14"),
                s("Review", "2026-01-08", "2026-01-08"),
                // Saturday the 17th and the closed Monday the 19th are skipped.
                s("Fix", "2026-01-20", "2026-01-28"),
            ]
        );
        assert_eq!(g.sections[1].name.as_deref(), Some("Release"));
        assert!(g.sections[0].tasks[1].tags.active);
        assert!(g.sections[1].tasks[1].tags.milestone);
    }

    #[test]
    fn undated_and_settings() {
        let (g, d) = lower_src(
            "title Plan\nprintscale weekly\n[A] lasts 2 days\n[B] starts D+1 and lasts 1 day\n\
             today is 1 days after start",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(g.title.as_deref(), Some("Plan"));
        assert_eq!(g.axis_format, "%-j");
        assert_eq!(
            g.tasks().map(|t| t.start).collect::<Vec<_>>(),
            [0, time::DAY]
        );
        assert_eq!(g.today_marker, TodayMarker::Default);
        assert_eq!(g.now, time::DAY);
        assert_eq!(g.tick_interval.unwrap().unit, Unit::Week);
    }

    #[test]
    fn errors() {
        let (g, d) = lower_src(
            "[A] lasts forever\n[B] starts at [Nope]'s end\n[C] starts at [D]'s end\n\
             [D] starts at [C]'s end\nwhat\n[E] is colored in red\n[F] lasts 1 day\n\
             [F] is deleted\nProject starts soon",
        );
        assert_eq!(
            d,
            [
                "cannot read `lasts forever`",
                "no task is named `[Nope]`",
                "`[C]` depends on itself and cannot be placed",
                "`[D]` depends on itself and cannot be placed",
                STATEMENT,
                "task colors are not drawn yet; ignoring it",
                "expected a date such as `2026-01-05`",
            ]
        );
        assert_eq!(
            g.tasks().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            ["A", "E"]
        );
    }
}
