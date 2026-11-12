//! PlantUML Gantt charts (PGANTT-*), lowered to the gantt model (MGANTT-3): tasks placed by
//! durations, dates, and the ends of other tasks, with closed days skipped.

use std::cell::Cell;
use std::collections::{HashMap, HashSet, VecDeque};

use rowan::TextRange;

use super::lines::{self, Common, Cursor, Line, keyword};
use crate::diagnostic::Diagnostic;
use crate::gantt::{self, Gantt, Interval, Section, Tags, Time, TodayMarker, Unit, time};
use crate::syntax::SyntaxNode;

pub(crate) const STATEMENT: &str =
    "expected a Gantt statement such as `[Task] lasts 3 days` or `Project starts 2026-01-05`";

/// The most days a duration, `D+N`, or offset counts.
const MAX_DAYS: i64 = 100_000;
/// The most days `... is closed` and `... is open` lines mark, together.
const MAX_MARKED_DAYS: usize = 10_000;
/// The most days placing every task may step through to skip closed days.
const STEP_BUDGET: i64 = 2_000_000;

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
    /// Where `start` and `end` were written.
    start_range: TextRange,
    end_range: TextRange,
}

struct Lower {
    g: Gantt,
    d: Vec<Diagnostic>,
    tasks: Vec<Task>,
    /// Task names and aliases to tasks.
    index: HashMap<String, usize>,
    /// Names and aliases of deleted tasks.
    deleted: HashSet<String>,
    project: Option<Time>,
    today: Option<When>,
    /// The section new tasks go in.
    section: usize,
    /// Closed weekdays, 0 for Sunday.
    weekdays: [bool; 7],
    /// Days marked by `... is closed` and `... is open` so far.
    marked: usize,
    /// Days left to step through when skipping closed days.
    budget: Cell<i64>,
}

/// A `[name]` at the start of `s`, and the rest.
fn bracket(s: &str) -> Option<(&str, &str)> {
    let body = s.trim_start().strip_prefix('[')?;
    let end = body.find(']')?;
    Some((body[..end].trim(), body[end + 1..].trim_start()))
}

/// The clauses of a task line, split on ` and ` outside `[...]`.
fn clauses(s: &str) -> Vec<&str> {
    let bytes = s.as_bytes();
    let (mut out, mut from, mut depth, mut i) = (Vec::new(), 0, 0usize, 0);
    while i < bytes.len() {
        match bytes[i] {
            b'[' => depth += 1,
            b']' => depth = depth.saturating_sub(1),
            b' ' if depth == 0
                && bytes.len() - i >= 5
                && bytes[i..i + 5].eq_ignore_ascii_case(b" and ") =>
            {
                out.push(&s[from..i]);
                i += 5;
                from = i;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    out.push(&s[from..]);
    out
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
    let n = count(&s[..n_end])?;
    let rest = s[n_end..].trim_start();
    let unit = lines::word(rest);
    let per = match unit.to_ascii_lowercase().as_str() {
        "day" | "days" => 1,
        "week" | "weeks" => 7,
        _ => return None,
    };
    Some(((n * per).min(MAX_DAYS), rest[unit.len()..].trim_start()))
}

/// Digits as a count of days, no more than [`MAX_DAYS`].
fn count(s: &str) -> Option<i64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(s.parse::<i64>().unwrap_or(MAX_DAYS).min(MAX_DAYS))
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
        return count(n.trim()).map(When::Day);
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
        deleted: HashSet::new(),
        project: None,
        today: None,
        section: 0,
        weekdays: [false; 7],
        marked: 0,
        budget: Cell::new(STEP_BUDGET),
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
                None => self.error(
                    line.range,
                    "expected `daily`, `weekly`, `monthly`, `quarterly`, or `yearly`",
                ),
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
        if let Some((from, rest)) = date(head) {
            let to = match keyword(rest, "to") {
                Some(r) => match date(r) {
                    Some((t, "")) => t,
                    _ => return self.error(line.range, "expected a date after `to`"),
                },
                None if rest.is_empty() => from,
                None => return self.error(line.range, STATEMENT),
            };
            let (from, to) = (from.div_euclid(time::DAY), to.div_euclid(time::DAY));
            for n in from..=to {
                if self.marked >= MAX_MARKED_DAYS {
                    return self.warn(
                        line.range,
                        format!(
                            "more than {MAX_MARKED_DAYS} days are closed or opened; ignoring the rest"
                        ),
                    );
                }
                self.marked += 1;
                let (add, remove) = if closed {
                    (&mut self.g.closed_days, &mut self.g.open_days)
                } else {
                    (&mut self.g.open_days, &mut self.g.closed_days)
                };
                remove.remove(&n);
                add.insert(n);
            }
            return;
        }
        let mut weekdays = self.weekdays;
        for day in head
            .split([',', ' '])
            .filter(|s| !s.is_empty() && *s != "and")
        {
            let day = day.strip_suffix('s').unwrap_or(day);
            let Some(w) = time::weekday_of(day) else {
                return self.error(line.range, STATEMENT);
            };
            weekdays[w as usize] = closed;
        }
        if weekdays.iter().all(|&c| c) {
            return self.error(
                line.range,
                "closing every weekday leaves no day to work on; ignoring it",
            );
        }
        self.weekdays = weekdays;
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
                self.tasks[i].start_range = line.range;
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
        if let Some(p) = previous.filter(|&p| p != i)
            && self.tasks[i].start.is_none()
        {
            let target = self.tasks[p].name.clone();
            self.tasks[i].start = Some(When::Ref {
                target,
                end: true,
                days: 0,
            });
            self.tasks[i].start_range = line.range;
        }
        for clause in clauses(rest)
            .into_iter()
            .map(str::trim)
            .filter(|c| !c.is_empty())
        {
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
                let range = line.range_of(c);
                let task = &mut self.tasks[i];
                match w.as_str() {
                    "starts" => (task.start, task.start_range) = (Some(at), range),
                    "ends" => (task.end, task.end_range) = (Some(at), range),
                    _ => {
                        task.tags.milestone = true;
                        task.duration = Some(0);
                        (task.start, task.start_range) = (Some(at), range);
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
        let n = t.div_euclid(time::DAY);
        if self.g.open_days.contains(&n) {
            return false;
        }
        self.weekdays[time::weekday(t) as usize] || self.g.closed_days.contains(&n)
    }

    /// Takes a step from the budget; false once it is spent.
    fn step(&self) -> bool {
        let left = self.budget.get();
        self.budget.set(left - 1);
        left > 0
    }

    /// The first open day at or after `t`.
    fn open_from(&self, mut t: Time) -> Time {
        while self.is_closed(t) && self.step() {
            t = t.saturating_add(time::DAY);
        }
        t
    }

    /// `n` open days after `start`: the end of the last one. Closed days are skipped.
    fn add_open(&self, start: Time, n: i64) -> Time {
        let (mut t, mut left) = (start, n);
        while left > 0 {
            if !self.step() {
                // Out of budget: the rest as plain days.
                return t.saturating_add(left.saturating_mul(time::DAY));
            }
            if !self.is_closed(t) {
                left -= 1;
            }
            t = t.saturating_add(time::DAY);
        }
        t
    }

    /// `n` open days before `end`: the start of the first one.
    fn sub_open(&self, end: Time, n: i64) -> Time {
        let (mut t, mut left) = (end, n);
        while left > 0 {
            if !self.step() {
                return t.saturating_sub(left.saturating_mul(time::DAY));
            }
            t = t.saturating_sub(time::DAY);
            if !self.is_closed(t) {
                left -= 1;
            }
        }
        t
    }

    /// The time `w` names, once what it refers to is placed.
    fn resolve(&self, w: &When, placed: &[Option<(Time, Time)>], project: Time) -> Option<Time> {
        match w {
            When::Date(t) => Some(*t),
            When::Day(n) => Some(project.saturating_add(n.saturating_mul(time::DAY))),
            When::Ref { target, end, days } => {
                let (s, e) = (*placed.get(*self.index.get(target)?)?)?;
                Some(if *end { e } else { s }.saturating_add(days.saturating_mul(time::DAY)))
            }
        }
    }

    /// The tasks `i` refers to, with where each reference was written.
    fn deps(&self, i: usize) -> Vec<(usize, TextRange)> {
        let task = &self.tasks[i];
        [(&task.start, task.start_range), (&task.end, task.end_range)]
            .into_iter()
            .filter_map(|(w, range)| match w {
                Some(When::Ref { target, .. }) => Some((*self.index.get(target)?, range)),
                _ => None,
            })
            .collect()
    }

    /// The first reference of task `i` to a missing or deleted task, as an error.
    fn missing(&self, i: usize) -> Option<(TextRange, String)> {
        let task = &self.tasks[i];
        [(&task.start, task.start_range), (&task.end, task.end_range)]
            .into_iter()
            .find_map(|(w, range)| match w {
                Some(When::Ref { target, .. }) if !self.index.contains_key(target) => Some((
                    range,
                    if self.deleted.contains(target) {
                        format!("`[{target}]` is deleted")
                    } else {
                        format!("no task is named `[{target}]`")
                    },
                )),
                _ => None,
            })
    }

    /// Places task `i` once what it refers to is placed: its span, and an error if it ends
    /// before it starts.
    fn place(
        &self,
        i: usize,
        placed: &[Option<(Time, Time)>],
        project: Time,
    ) -> Option<((Time, Time), Option<Diagnostic>)> {
        let task = &self.tasks[i];
        let start = match &task.start {
            Some(w) => Some(self.resolve(w, placed, project)?),
            None => None,
        };
        let end = match &task.end {
            Some(w) => Some(self.resolve(w, placed, project)?),
            None => None,
        };
        let duration = task.duration;
        let mut error = None;
        let span = match (start, end) {
            (Some(s), Some(e)) => {
                // A date or `D+N` end names the last day, a day before `e`.
                let last_day = matches!(task.end, Some(When::Date(_) | When::Day(_)));
                if e < s || last_day && e == s && !task.tags.milestone {
                    error = Some(Diagnostic::new(
                        task.end_range,
                        format!("`[{}]` ends before it starts", task.name),
                    ));
                }
                (s, e.max(s))
            }
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
        Some((span, error))
    }

    /// Which unplaced tasks are on a dependency cycle (Tarjan's strongly connected components,
    /// iterative).
    fn cycles(&self, unplaced: &[bool]) -> Vec<bool> {
        let n = self.tasks.len();
        let deps: Vec<Vec<usize>> = (0..n)
            .map(|i| {
                if !unplaced[i] {
                    return Vec::new();
                }
                self.deps(i)
                    .into_iter()
                    .map(|(j, _)| j)
                    .filter(|&j| unplaced[j])
                    .collect()
            })
            .collect();
        let mut on_cycle = vec![false; n];
        let mut order = vec![usize::MAX; n];
        let mut low = vec![0; n];
        let mut on_stack = vec![false; n];
        let mut stack = Vec::new();
        let mut next = 0;
        for root in (0..n).filter(|&i| unplaced[i]) {
            if order[root] != usize::MAX {
                continue;
            }
            // (node, next dependency to visit)
            let mut work = vec![(root, 0)];
            while let Some(top) = work.last_mut() {
                let (v, k) = *top;
                if k == 0 && order[v] == usize::MAX {
                    order[v] = next;
                    low[v] = next;
                    next += 1;
                    stack.push(v);
                    on_stack[v] = true;
                }
                if let Some(&w) = deps[v].get(k) {
                    top.1 += 1;
                    if order[w] == usize::MAX {
                        work.push((w, 0));
                    } else if on_stack[w] {
                        low[v] = low[v].min(order[w]);
                    }
                    continue;
                }
                work.pop();
                if let Some(&(parent, _)) = work.last() {
                    low[parent] = low[parent].min(low[v]);
                }
                if low[v] == order[v] {
                    let mut component = Vec::new();
                    while let Some(w) = stack.pop() {
                        on_stack[w] = false;
                        component.push(w);
                        if w == v {
                            break;
                        }
                    }
                    let cyclic = component.len() > 1 || deps[v].contains(&v);
                    for w in component {
                        on_cycle[w] = cyclic;
                    }
                }
            }
        }
        on_cycle
    }

    /// Reports each task left unplaced, at the reference that held it back.
    fn unplaced(&mut self, placed: &[Option<(Time, Time)>]) {
        let n = self.tasks.len();
        let unplaced: Vec<bool> = (0..n)
            .map(|i| !self.tasks[i].deleted && placed[i].is_none())
            .collect();
        let on_cycle = self.cycles(&unplaced);
        // The first unplaced task each one refers to.
        let blocker = |i: usize| {
            self.deps(i)
                .into_iter()
                .find(|&(j, _)| unplaced[j])
                .unwrap_or((i, self.tasks[i].range))
        };
        // Where each chain of unplaced tasks leads: a task on a cycle or with a bad reference.
        let mut root: Vec<Option<usize>> = vec![None; n];
        let mut out = Vec::new();
        for i in (0..n).filter(|&i| unplaced[i]) {
            let mut path = Vec::new();
            let mut k = i;
            let r = loop {
                if let Some(r) = root[k] {
                    break r;
                }
                let next = blocker(k).0;
                if on_cycle[k] || next == k || self.missing(k).is_some() {
                    break k;
                }
                path.push(k);
                k = next;
            };
            root[k] = Some(r);
            for p in path {
                root[p] = Some(r);
            }
            let name = &self.tasks[i].name;
            let (range, message) = if let Some(missing) = self.missing(i) {
                missing
            } else if on_cycle[i] {
                (
                    blocker(i).1,
                    format!("`[{name}]` depends on itself and cannot be placed"),
                )
            } else {
                let because = if on_cycle[r] {
                    "depends on itself"
                } else {
                    "cannot be placed"
                };
                let root_name = &self.tasks[r].name;
                (
                    blocker(i).1,
                    format!("`[{name}]` depends on `[{root_name}]`, which {because}"),
                )
            };
            out.push(Diagnostic::new(range, message));
        }
        self.d.extend(out);
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
            When::Date(t) => When::Date(t.saturating_add(time::DAY)),
            When::Day(n) => When::Day(n + 1),
            w => w,
        };
        for task in &mut self.tasks {
            task.end = task.end.take().map(end_of);
        }
        for (name, i) in self.index.clone() {
            if self.tasks[i].deleted {
                self.index.remove(&name);
                self.deleted.insert(name);
            }
        }
        self.g.excludes = (0..7)
            .filter(|&w| self.weekdays[w])
            .map(|w| time::weekday_name(w as i64).to_lowercase())
            .collect();
        let n = self.tasks.len();
        // Kahn's algorithm: each task once all it refers to is placed.
        let mut waiting = vec![0usize; n];
        let mut dependents = vec![Vec::new(); n];
        for i in (0..n).filter(|&i| !self.tasks[i].deleted) {
            for (j, _) in self.deps(i) {
                waiting[i] += 1;
                dependents[j].push(i);
            }
        }
        let mut queue: VecDeque<usize> = (0..n)
            .filter(|&i| !self.tasks[i].deleted && waiting[i] == 0)
            .collect();
        let mut placed: Vec<Option<(Time, Time)>> = vec![None; n];
        let mut spent = false;
        while let Some(i) = queue.pop_front() {
            if let Some((span, error)) = self.place(i, &placed, project) {
                placed[i] = Some(span);
                self.d.extend(error);
            }
            if !spent && self.budget.get() < 0 {
                spent = true;
                let name = &self.tasks[i].name;
                self.d.push(Diagnostic::warning(
                    self.tasks[i].range,
                    format!(
                        "too many closed days to skip; `[{name}]` and later tasks ignore some of them"
                    ),
                ));
            }
            for &k in &dependents[i] {
                waiting[k] -= 1;
                if waiting[k] == 0 {
                    queue.push_back(k);
                }
            }
        }
        self.unplaced(&placed);
        for (i, task) in self.tasks.iter().enumerate() {
            if task.deleted {
                continue;
            }
            let Some((start, end)) = placed[i] else {
                continue;
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
            && let Some(t) = self.resolve(&today, &placed, project)
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
    fn today_at_a_task() {
        let (g, d) = lower_src("[A] lasts 2 days\ntoday is [A]'s end");
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(g.today_marker, TodayMarker::Default);
        assert_eq!(g.now, 2 * time::DAY);
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

    /// Diagnostics as (message, the source text they point at).
    fn lower_ranged(body: &str) -> (Gantt, Vec<(String, String)>) {
        let src = format!("@startgantt\n{body}\n@endgantt\n");
        let (diagram, d) = super::super::parse(&src).lower();
        let Diagram::Gantt(g) = diagram else {
            panic!("not a gantt chart")
        };
        let at = |r: TextRange| src[usize::from(r.start())..usize::from(r.end())].to_owned();
        (g, d.into_iter().map(|d| (d.message, at(d.range))).collect())
    }

    fn s(a: &str, b: &str, c: &str) -> (String, String, String) {
        (a.to_owned(), b.to_owned(), c.to_owned())
    }

    #[test]
    fn ends_on_a_day_number() {
        let (g, d) = lower_src(
            "Project starts 2026-01-05\n[A] starts D+0 and ends D+2\n\
             [B] starts 2026-01-05 and ends 2026-01-07",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(
            spans(&g),
            [
                s("A", "2026-01-05", "2026-01-07"),
                s("B", "2026-01-05", "2026-01-07")
            ]
        );
    }

    #[test]
    fn reopened_weekdays() {
        // Friday the 9th: Saturday is reopened, Sunday stays closed.
        let (g, d) = lower_src(
            "Project starts 2026-01-09\nsaturday are closed\nsunday are closed\n\
             saturday are open\n[A] lasts 3 days",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(spans(&g), [s("A", "2026-01-09", "2026-01-12")]);
        assert_eq!(g.excludes, ["sunday"]);
        assert!(!g.is_excluded(day("2026-01-10")));
        assert!(g.is_excluded(day("2026-01-11")));
        // A reopened date beats its closed weekday, whatever the order.
        let (g, d) = lower_src(
            "Project starts 2026-01-09\n2026-01-11 is open\nsaturday are closed\n\
             sunday are closed\n[A] lasts 2 days",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(spans(&g), [s("A", "2026-01-09", "2026-01-11")]);
        assert!(!g.is_excluded(day("2026-01-11")));
    }

    #[test]
    fn huge_day_numbers() {
        let (g, d) = lower_src(
            "Project starts 2026-01-05\nsunday are closed\n\
             [A] starts D+9223372036854775807\n[B] starts D+100000000000\n\
             [C] starts 2026-01-05 and ends D+9223372036854775807\n\
             [E] lasts 99999999999999999999 days\ntoday is D+9223372036854775807",
        );
        assert!(d.is_empty(), "{d:?}");
        let far = day("2026-01-05") + MAX_DAYS * time::DAY;
        let starts: Vec<Time> = g.tasks().map(|t| t.start).collect();
        assert_eq!(starts[..2], [far, far]);
        assert_eq!(g.tasks().nth(2).unwrap().end, far + time::DAY);
        assert_eq!(g.now, far);
        // Drawing looks at a bounded number of days.
        assert!(g.excluded_days().len() <= gantt::MAX_EXCLUDED_SCAN);
    }

    #[test]
    fn every_weekday_closed() {
        let (g, d) = lower_src(
            "Project starts 2026-01-05\nsaturday are closed\nsunday are closed\n\
             monday, tuesday, wednesday, thursday and friday are closed\n[A] lasts 2 days",
        );
        assert_eq!(
            d,
            ["closing every weekday leaves no day to work on; ignoring it"]
        );
        assert_eq!(spans(&g), [s("A", "2026-01-05", "2026-01-06")]);
        assert_eq!(g.excludes, ["sunday", "saturday"]);
    }

    #[test]
    fn closed_day_budget() {
        let mut body =
            "Project starts 2026-01-01\n2026-01-01 to 2027-12-31 are closed\n".to_owned();
        for i in 0..3000 {
            body.push_str(&format!("[T{i}] lasts 1 day\n"));
        }
        let (g, d) = lower_src(&body);
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].starts_with("too many closed days to skip"), "{d:?}");
        let tasks: Vec<_> = g.tasks().collect();
        assert_eq!(tasks.len(), 3000);
        assert_eq!(tasks[0].start, day("2028-01-01"));
        // Past the budget, closed days are not skipped.
        assert_eq!(tasks[2999].start, day("2026-01-01"));
    }

    #[test]
    fn closed_day_cap() {
        let (g, d) =
            lower_src("2000-01-01 to 2015-12-31 are closed\n2016-01-01 to 2030-12-31 are opened");
        assert_eq!(
            d,
            [format!(
                "more than {MAX_MARKED_DAYS} days are closed or opened; ignoring the rest"
            )]
        );
        assert_eq!(g.closed_days.len(), 5844);
        assert_eq!(g.open_days.len(), MAX_MARKED_DAYS - 5844);
        let (g, _) = lower_src("[A] lasts 1000000 weeks");
        let a = g.tasks().next().unwrap();
        assert_eq!(a.end - a.start, MAX_DAYS * time::DAY);
    }

    #[test]
    fn reverse_declared_chain() {
        let mut body = String::new();
        for i in 0..2000 {
            body.push_str(&format!("[T{i}] starts at [T{}]'s end\n", i + 1));
        }
        body.push_str("[T2000] lasts 1 day");
        let (g, d) = lower_src(&body);
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(g.tasks().next().unwrap().start, 2000 * time::DAY);
    }

    #[test]
    fn and_inside_a_name() {
        let (g, d) = lower_src(
            "[Design and build] lasts 2 days\n\
             [Test] starts at [Design and build]'s end and lasts 1 day",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(
            g.tasks().map(|t| t.start).collect::<Vec<_>>(),
            [0, 2 * time::DAY]
        );
        assert_eq!(
            clauses("a AND b and [x and y] and c"),
            ["a", "b", "[x and y]", "c"]
        );
    }

    #[test]
    fn unplaceable_dependencies() {
        let (_, d) = lower_ranged(
            "[A] starts at [B]'s end\n[B] lasts 1 day\n[B] starts at [C]'s end\n\
             [C] starts at [C]'s end\n[P] starts at [Q]'s end\n[Q] starts at [Nope]'s end\n\
             [M] lasts 1 day\n[M] starts at [Gone]'s end and lasts 2 days",
        );
        let p = |m: &str, at: &str| (m.to_owned(), at.to_owned());
        assert_eq!(
            d,
            [
                p(
                    "`[A]` depends on `[C]`, which depends on itself",
                    "starts at [B]'s end"
                ),
                p(
                    "`[B]` depends on `[C]`, which depends on itself",
                    "starts at [C]'s end"
                ),
                p(
                    "`[C]` depends on itself and cannot be placed",
                    "starts at [C]'s end"
                ),
                p(
                    "`[P]` depends on `[Q]`, which cannot be placed",
                    "starts at [Q]'s end"
                ),
                p("no task is named `[Nope]`", "starts at [Nope]'s end"),
                p("no task is named `[Gone]`", "starts at [Gone]'s end"),
            ]
        );
    }

    #[test]
    fn deleted_reference() {
        let (g, d) = lower_src("[F] lasts 1 day\n[F] is deleted\n[G] starts at [F]'s end");
        assert_eq!(d, ["`[F]` is deleted"]);
        assert_eq!(g.tasks().count(), 0);
    }

    #[test]
    fn end_before_start() {
        let (g, d) = lower_ranged(
            "[A] starts 2026-01-10 and ends 2026-01-08\n\
             [B] starts 2026-01-10 and ends 2026-01-09\n\
             [C] starts 2026-01-10 and ends 2026-01-10\n\
             [D] starts at [C]'s end and ends at [C]'s start",
        );
        let p = |m: &str, at: &str| (m.to_owned(), at.to_owned());
        assert_eq!(
            d,
            [
                p("`[A]` ends before it starts", "ends 2026-01-08"),
                p("`[B]` ends before it starts", "ends 2026-01-09"),
                p("`[D]` ends before it starts", "ends at [C]'s start"),
            ]
        );
        assert_eq!(g.tasks().count(), 4);
    }

    #[test]
    fn end_only_task_skips_closed_days() {
        // Monday the 12th ends the task; two open days back is Friday the 9th.
        let (g, d) = lower_src(
            "Project starts 2026-01-05\nsaturday are closed\nsunday are closed\n\
             [A] ends 2026-01-12 and lasts 2 days",
        );
        assert!(d.is_empty(), "{d:?}");
        assert_eq!(spans(&g), [s("A", "2026-01-09", "2026-01-12")]);
    }

    #[test]
    fn end_only_task_budget() {
        let mut body =
            "Project starts 2028-01-01\n2026-01-01 to 2027-12-31 are closed\n".to_owned();
        for i in 0..3000 {
            body.push_str(&format!("[T{i}] ends 2027-12-31 and lasts 1 day\n"));
        }
        let (g, d) = lower_src(&body);
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].starts_with("too many closed days to skip"), "{d:?}");
        let tasks: Vec<_> = g.tasks().collect();
        assert_eq!(tasks.len(), 3000);
        // Walks back over the closed run to the last open day before it.
        assert_eq!(tasks[0].start, day("2025-12-31"));
        // Past the budget, the rest are plain days.
        assert_eq!(tasks[2999].start, day("2027-12-31"));
        assert_eq!(tasks[2999].end, day("2028-01-01"));
    }

    #[test]
    fn task_between_two_cycles() {
        let (_, d) = lower_ranged(
            "[P] starts at [Q]'s end\n[Q] starts at [P]'s end\n\
             [M] starts at [Q]'s end\n\
             [R] starts at [M]'s end and ends at [S]'s end\n[S] starts at [R]'s end",
        );
        let m: Vec<&str> = d.iter().map(|(m, _)| m.as_str()).collect();
        assert_eq!(
            m,
            [
                "`[P]` depends on itself and cannot be placed",
                "`[Q]` depends on itself and cannot be placed",
                "`[M]` depends on `[Q]`, which depends on itself",
                "`[R]` depends on itself and cannot be placed",
                "`[S]` depends on itself and cannot be placed",
            ]
        );
        // R and S cycle, though R also hangs off M: M is not blamed as a cycle member.
        assert_eq!(d[2].1, "starts at [Q]'s end");
    }

    #[test]
    fn long_cycle_with_dependents() {
        let (g, d) = lower_ranged(
            "[A] starts at [C]'s end\n[B] starts at [A]'s end\n[C] starts at [B]'s end\n\
             [D] starts at [A]'s end\n[E] starts at [D]'s end\n[F] lasts 1 day",
        );
        let m: Vec<&str> = d.iter().map(|(m, _)| m.as_str()).collect();
        assert_eq!(
            m,
            [
                "`[A]` depends on itself and cannot be placed",
                "`[B]` depends on itself and cannot be placed",
                "`[C]` depends on itself and cannot be placed",
                "`[D]` depends on `[A]`, which depends on itself",
                "`[E]` depends on `[A]`, which depends on itself",
            ]
        );
        assert_eq!(
            g.tasks().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            ["F"]
        );
    }

    #[test]
    fn missing_target_beats_cycle() {
        let (_, d) = lower_ranged(
            "[C] starts at [C]'s end\n\
             [T] starts at [C]'s end and ends at [Nope]'s end\n\
             [U] starts at [T]'s end",
        );
        let m: Vec<&str> = d.iter().map(|(m, _)| m.as_str()).collect();
        assert_eq!(
            m,
            [
                "`[C]` depends on itself and cannot be placed",
                "no task is named `[Nope]`",
                "`[U]` depends on `[T]`, which cannot be placed",
            ]
        );
        assert_eq!(d[1].1, "ends at [Nope]'s end");
    }

    #[test]
    fn then_after_a_deleted_task() {
        let (g, d) =
            lower_ranged("[F] lasts 1 day\n[F] is deleted\nthen [G] lasts 1 day\n[H] lasts 1 day");
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].0, "`[F]` is deleted");
        assert_eq!(
            g.tasks().map(|t| t.name.as_str()).collect::<Vec<_>>(),
            ["H"]
        );
    }

    #[test]
    fn reversed_closed_range_marks_nothing() {
        let (g, d) = lower_src(
            "Project starts 2026-01-05\n2026-01-09 to 2026-01-06 are closed\n[A] lasts 5 days",
        );
        assert!(d.is_empty(), "{d:?}");
        assert!(g.closed_days.is_empty());
        assert_eq!(spans(&g), [s("A", "2026-01-05", "2026-01-09")]);
    }

    #[test]
    fn open_then_closed_same_day() {
        let (g, _) = lower_src(
            "2026-01-05 is open\n2026-01-05 is closed\n2026-01-06 is closed\n2026-01-06 is open",
        );
        let n = day("2026-01-05").div_euclid(time::DAY);
        assert!(g.closed_days.contains(&n) && !g.open_days.contains(&n));
        assert!(g.open_days.contains(&(n + 1)) && !g.closed_days.contains(&(n + 1)));
    }

    #[test]
    fn printscale_words() {
        let (_, d) = lower_src("printscale hourly");
        assert_eq!(
            d,
            ["expected `daily`, `weekly`, `monthly`, `quarterly`, or `yearly`"]
        );
    }
}
