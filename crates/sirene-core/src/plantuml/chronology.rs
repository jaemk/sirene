//! PlantUML chronology diagrams (PCHRON-*): `[Event] happens on DATE` lines, drawn as a timeline
//! (TIMELI-2) in date order, one period per moment.

use std::collections::BTreeMap;

use super::lines::{self, Common, Cursor};
use crate::diagnostic::Diagnostic;
use crate::syntax::SyntaxNode;
use crate::timeline::{Period, Section, Timeline};

pub(crate) const STATEMENT: &str = "expected an event such as `[Launch] happens on 2024-01-15`";

/// `YYYY-MM-DD`, optionally followed by `HH:MM` or `HH:MM:SS`, as a sortable tuple.
fn moment(s: &str) -> Option<[u32; 6]> {
    let mut parts = s.split_whitespace();
    let date = parts.next()?;
    let time = parts.next();
    if parts.next().is_some() {
        return None;
    }
    let num = |p: &str, len: usize| {
        (p.len() == len && p.bytes().all(|b| b.is_ascii_digit()))
            .then(|| p.parse::<u32>().ok())
            .flatten()
    };
    let mut d = date.split('-');
    let (y, mo, da) = (num(d.next()?, 4)?, num(d.next()?, 2)?, num(d.next()?, 2)?);
    if d.next().is_some()
        || !(1..=12).contains(&mo)
        || da < 1
        || i64::from(da) > crate::gantt::time::days_in_month(i64::from(y), i64::from(mo))
    {
        return None;
    }
    let (mut h, mut mi, mut se) = (0, 0, 0);
    if let Some(t) = time {
        let mut t = t.split(':');
        h = num(t.next()?, 2)?;
        mi = num(t.next()?, 2)?;
        if let Some(s) = t.next() {
            se = num(s, 2)?;
        }
        if t.next().is_some() || h > 23 || mi > 59 || se > 59 {
            return None;
        }
    }
    Some([y, mo, da, h, mi, se])
}

pub(crate) fn lower(root: &SyntaxNode) -> (Timeline, Vec<Diagnostic>) {
    let mut d = Vec::new();
    let mut timeline = Timeline::default();
    // moment -> (the date as first written, event names), in date order
    let mut moments: BTreeMap<[u32; 6], (String, Vec<String>)> = BTreeMap::new();
    let mut cursor = Cursor::new(lines::read(root));
    while let Some(line) = cursor.next() {
        match lines::common(&line, &mut cursor, &mut d) {
            Common::No => {}
            Common::Title(t) => {
                timeline.title = Some(t);
                continue;
            }
            Common::Done | Common::Direction(_) => continue,
        }
        let Some(rest) = line.text.strip_prefix('[') else {
            d.push(Diagnostic::new(line.range, STATEMENT));
            continue;
        };
        let Some(close) = rest.find(']') else {
            d.push(Diagnostic::new(line.range, "`[` has no closing `]`"));
            continue;
        };
        let name = lines::text(rest[..close].trim());
        let after = rest[close + 1..].trim();
        let Some(date) = lines::keyword(after, "happens")
            .and_then(|r| lines::keyword(r, "on").or_else(|| lines::keyword(r, "at")))
        else {
            d.push(Diagnostic::new(line.range, STATEMENT));
            continue;
        };
        let date = date.trim();
        let Some(at) = moment(date) else {
            d.push(Diagnostic::new(
                line.range_of(date),
                format!(
                    "expected a date such as `2024-01-15` or `2024-01-15 13:08:12`, not `{date}`"
                ),
            ));
            continue;
        };
        moments
            .entry(at)
            .or_insert_with(|| (date.to_owned(), Vec::new()))
            .1
            .push(name);
    }
    timeline.sections.push(Section {
        name: None,
        periods: moments
            .into_values()
            .map(|(date, names)| Period {
                label: vec![date],
                events: names
                    .into_iter()
                    .map(|n| n.split('\n').map(str::to_owned).collect())
                    .collect(),
            })
            .collect(),
    });
    (timeline, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moments() {
        assert_eq!(moment("2024-01-15"), Some([2024, 1, 15, 0, 0, 0]));
        assert_eq!(moment("2024-01-15 01:08:12"), Some([2024, 1, 15, 1, 8, 12]));
        assert_eq!(moment("2024-01-15 01:08"), Some([2024, 1, 15, 1, 8, 0]));
        assert_eq!(moment("2024-13-01"), None);
        assert_eq!(moment("2024-02-29"), Some([2024, 2, 29, 0, 0, 0]));
        assert_eq!(moment("2023-02-29"), None);
        assert_eq!(moment("2024-04-31"), None);
        assert_eq!(moment("2024-01-00"), None);
        assert_eq!(moment("2024-1-01"), None);
        assert_eq!(moment("2024-01-15 25:00"), None);
        assert_eq!(moment("soon"), None);
    }
}
