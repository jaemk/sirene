//! Dates on a calendar with no time zone, read and written in dayjs and d3 formats (MGANTT-2),
//! and the axis ticks d3 would choose (MGANTT-4).

/// Milliseconds since 1970-01-01 00:00, with no time zone.
pub type Time = i64;

pub const SECOND: i64 = 1000;
pub const MINUTE: i64 = 60 * SECOND;
pub const HOUR: i64 = 60 * MINUTE;
pub const DAY: i64 = 24 * HOUR;
pub const WEEK: i64 = 7 * DAY;

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (
        if m <= 2 {
            yoe + era * 400 + 1
        } else {
            yoe + era * 400
        },
        m,
        d,
    )
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        2 if is_leap(y) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// A time split into calendar fields; `month` and `day` count from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parts {
    pub year: i64,
    pub month: i64,
    pub day: i64,
    pub hour: i64,
    pub minute: i64,
    pub second: i64,
    pub ms: i64,
}

impl Parts {
    pub fn of(t: Time) -> Parts {
        let days = t.div_euclid(DAY);
        let rest = t.rem_euclid(DAY);
        let (year, month, day) = civil_from_days(days);
        Parts {
            year,
            month,
            day,
            hour: rest / HOUR,
            minute: rest % HOUR / MINUTE,
            second: rest % MINUTE / SECOND,
            ms: rest % SECOND,
        }
    }

    /// The time of these fields; out-of-range days and times carry into the next field.
    pub fn time(&self) -> Time {
        days_from_civil(self.year, self.month, 1) * DAY
            + (self.day - 1) * DAY
            + self.hour * HOUR
            + self.minute * MINUTE
            + self.second * SECOND
            + self.ms
    }
}

/// The weekday of `t`, 0 for Sunday.
pub fn weekday(t: Time) -> i64 {
    (t.div_euclid(DAY) + 4).rem_euclid(7)
}

/// Midnight at the start of `t`'s day.
pub fn day_start(t: Time) -> Time {
    t.div_euclid(DAY) * DAY
}

/// `t` plus `n` months, the day kept but no later than the month's last (dayjs `add`).
pub fn add_months(t: Time, n: i64) -> Time {
    let p = Parts::of(t);
    let m0 = p.year * 12 + p.month - 1 + n;
    let (year, month) = (m0.div_euclid(12), m0.rem_euclid(12) + 1);
    Parts {
        year,
        month,
        day: p.day.min(days_in_month(year, month)),
        ..p
    }
    .time()
}

/// The lowercase English name of a weekday, 0 for Sunday.
pub fn weekday_name(n: i64) -> &'static str {
    WEEKDAYS[n.rem_euclid(7) as usize]
}

/// The weekday named `s` (`monday`, any case), 0 for Sunday.
pub fn weekday_of(s: &str) -> Option<i64> {
    WEEKDAYS
        .iter()
        .position(|d| d.eq_ignore_ascii_case(s))
        .map(|i| i as i64)
}

// dayjs formats.

/// One piece of a dayjs format: a token such as `YYYY`, or literal text.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece<'a> {
    Token(&'a str),
    Literal(&'a str),
}

const TOKENS: &[&str] = &[
    "YYYY", "YY", "MMMM", "MMM", "MM", "M", "Do", "DD", "D", "dddd", "ddd", "dd", "d", "HH", "H",
    "hh", "h", "kk", "k", "mm", "m", "ss", "s", "SSS", "SS", "S", "ZZ", "Z", "X", "x", "A", "a",
    "Q",
];

fn pieces(format: &str) -> Vec<Piece<'_>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < format.len() {
        let r = &format[i..];
        if let Some(body) = r.strip_prefix('[')
            && let Some(end) = body.find(']')
        {
            out.push(Piece::Literal(&body[..end]));
            i += end + 2;
        } else if let Some(t) = TOKENS.iter().find(|t| r.starts_with(**t)) {
            out.push(Piece::Token(t));
            i += t.len();
        } else {
            let n = r.chars().next().map_or(1, char::len_utf8);
            out.push(Piece::Literal(&r[..n]));
            i += n;
        }
    }
    out
}

fn ordinal(n: i64) -> &'static str {
    match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    }
}

fn hour12(h: i64) -> i64 {
    match h % 12 {
        0 => 12,
        n => n,
    }
}

/// `t` written in the dayjs `format` (MGANTT-2).
pub fn format_dayjs(t: Time, format: &str) -> String {
    let p = Parts::of(t);
    let mut out = String::new();
    for piece in pieces(format) {
        let s = match piece {
            Piece::Literal(l) => l.to_owned(),
            Piece::Token(tok) => match tok {
                "YYYY" => format!("{:04}", p.year),
                "YY" => format!("{:02}", p.year.rem_euclid(100)),
                "MMMM" => MONTHS[p.month as usize - 1].to_owned(),
                "MMM" => MONTHS[p.month as usize - 1][..3].to_owned(),
                "MM" => format!("{:02}", p.month),
                "M" => p.month.to_string(),
                "Do" => format!("{}{}", p.day, ordinal(p.day)),
                "DD" => format!("{:02}", p.day),
                "D" => p.day.to_string(),
                "dddd" => weekday_name(weekday(t)).to_owned(),
                "ddd" => weekday_name(weekday(t))[..3].to_owned(),
                "dd" => weekday_name(weekday(t))[..2].to_owned(),
                "d" => weekday(t).to_string(),
                "HH" => format!("{:02}", p.hour),
                "H" => p.hour.to_string(),
                "hh" => format!("{:02}", hour12(p.hour)),
                "h" => hour12(p.hour).to_string(),
                "kk" => format!("{:02}", if p.hour == 0 { 24 } else { p.hour }),
                "k" => (if p.hour == 0 { 24 } else { p.hour }).to_string(),
                "mm" => format!("{:02}", p.minute),
                "m" => p.minute.to_string(),
                "ss" => format!("{:02}", p.second),
                "s" => p.second.to_string(),
                "SSS" => format!("{:03}", p.ms),
                "SS" => format!("{:02}", p.ms / 10),
                "S" => (p.ms / 100).to_string(),
                "ZZ" => "+0000".to_owned(),
                "Z" => "+00:00".to_owned(),
                "X" => t.div_euclid(SECOND).to_string(),
                "x" => t.to_string(),
                "A" => (if p.hour < 12 { "AM" } else { "PM" }).to_owned(),
                "a" => (if p.hour < 12 { "am" } else { "pm" }).to_owned(),
                "Q" => ((p.month - 1) / 3 + 1).to_string(),
                _ => tok.to_owned(),
            },
        };
        out.push_str(&s);
    }
    out
}

/// The number of ASCII digits at the start of `s`, at most `max`.
fn digits(s: &str, max: usize) -> usize {
    s.bytes().take(max).take_while(u8::is_ascii_digit).count()
}

/// `s` read with the dayjs `format`, strictly: it must match exactly and be a real date. Fields
/// the format lacks come from `now` as dayjs fills them.
pub fn parse_dayjs(s: &str, format: &str, now: Time) -> Option<Time> {
    let mut rest = s;
    let (mut year, mut month, mut day) = (None, None, None);
    let (mut hour, mut minute, mut second, mut ms) = (0, 0, 0, 0);
    let mut pm = None;
    let mut offset = 0;
    let mut stamp = None;
    // Takes between `min` and `max` digits.
    let number = |rest: &mut &str, min: usize, max: usize| -> Option<i64> {
        let n = digits(rest, max);
        if n < min {
            return None;
        }
        let v = rest[..n].parse().ok()?;
        *rest = &rest[n..];
        Some(v)
    };
    for piece in pieces(format) {
        match piece {
            Piece::Literal(l) => rest = rest.strip_prefix(l)?,
            Piece::Token(tok) => match tok {
                "YYYY" => year = Some(number(&mut rest, 4, 4)?),
                "YY" => {
                    let y = number(&mut rest, 2, 2)?;
                    year = Some(y + if y > 68 { 1900 } else { 2000 });
                }
                "MMMM" | "MMM" => {
                    let i = MONTHS.iter().position(|m| {
                        let name = if tok == "MMM" { &m[..3] } else { m };
                        rest.starts_with(name)
                    })?;
                    rest = &rest[if tok == "MMM" { 3 } else { MONTHS[i].len() }..];
                    month = Some(i as i64 + 1);
                }
                "MM" => month = Some(number(&mut rest, 2, 2)?),
                "M" => month = Some(number(&mut rest, 1, 2)?),
                "Do" => {
                    day = Some(number(&mut rest, 1, 2)?);
                    let n = rest.bytes().take_while(u8::is_ascii_alphabetic).count();
                    rest = &rest[n..];
                }
                "DD" => day = Some(number(&mut rest, 2, 2)?),
                "D" => day = Some(number(&mut rest, 1, 2)?),
                "HH" | "hh" | "kk" => hour = number(&mut rest, 2, 2)?,
                "H" | "h" | "k" => hour = number(&mut rest, 1, 2)?,
                "mm" => minute = number(&mut rest, 2, 2)?,
                "m" => minute = number(&mut rest, 1, 2)?,
                "ss" => second = number(&mut rest, 2, 2)?,
                "s" => second = number(&mut rest, 1, 2)?,
                "SSS" => ms = number(&mut rest, 3, 3)?,
                "SS" => ms = number(&mut rest, 2, 2)? * 10,
                "S" => ms = number(&mut rest, 1, 1)? * 100,
                "A" | "a" => {
                    let lower = rest.get(..2)?.to_ascii_lowercase();
                    pm = Some(match lower.as_str() {
                        "am" => false,
                        "pm" => true,
                        _ => return None,
                    });
                    rest = &rest[2..];
                }
                "ZZ" | "Z" => {
                    if let Some(r) = rest.strip_prefix('Z') {
                        rest = r;
                    } else {
                        let sign = match rest.as_bytes().first()? {
                            b'+' => 1,
                            b'-' => -1,
                            _ => return None,
                        };
                        rest = &rest[1..];
                        let h = number(&mut rest, 2, 2)?;
                        if let Some(r) = rest.strip_prefix(':') {
                            rest = r;
                        }
                        let m = number(&mut rest, 2, 2)?;
                        offset = sign * (h * HOUR + m * MINUTE);
                    }
                }
                "X" | "x" => {
                    let n = rest
                        .bytes()
                        .take_while(|b| b.is_ascii_digit() || *b == b'.' || *b == b'-')
                        .count();
                    let v: f64 = rest[..n].parse().ok()?;
                    rest = &rest[n..];
                    stamp = Some(if tok == "X" {
                        (v * 1000.0).round() as i64
                    } else {
                        v as i64
                    });
                }
                // Weekdays and quarters are written, not read.
                _ => return None,
            },
        }
    }
    if !rest.is_empty() {
        return None;
    }
    if let Some(t) = stamp {
        return Some(t);
    }
    match pm {
        Some(true) if hour < 12 => hour += 12,
        Some(false) if hour == 12 => hour = 0,
        _ => {}
    }
    let today = Parts::of(now);
    let y = year.unwrap_or(today.year);
    let d = day.unwrap_or(if year.is_none() && month.is_none() {
        today.day
    } else {
        1
    });
    let m = match (year, month) {
        (_, Some(m)) => m,
        (Some(_), None) => 1,
        (None, None) => today.month,
    };
    if !(1..=12).contains(&m) || !(1..=days_in_month(y, m)).contains(&d) {
        return None;
    }
    if hour > 24 || minute > 59 || second > 59 {
        return None;
    }
    let t = Parts {
        year: y,
        month: m,
        day: d,
        hour,
        minute,
        second,
        ms,
    }
    .time()
        - offset;
    // Strict, as dayjs: the time written back in the format is the text read, so `M` does not
    // accept `01`. An offset is written as UTC, so it is left out of the check.
    (offset != 0 || format.contains('Z') || format_dayjs(t, format) == s).then_some(t)
}

/// `s` as an ISO 8601 date or date and time, the dates `Date` reads when the format does not
/// match.
pub fn parse_iso(s: &str) -> Option<Time> {
    let s = s.strip_suffix('Z').unwrap_or(s);
    let (date, time) = match s.find(['T', ' ']) {
        Some(i) => (&s[..i], Some(&s[i + 1..])),
        None => (s, None),
    };
    let mut parts = date.split('-');
    let field = |p: Option<&str>, n: usize| -> Option<i64> {
        let p = p?;
        (p.len() == n && digits(p, n) == n).then(|| p.parse().ok())?
    };
    let year = field(parts.next(), 4)?;
    let month = match parts.next() {
        Some(p) => field(Some(p), 2)?,
        None => 1,
    };
    let day = match parts.next() {
        Some(p) => field(Some(p), 2)?,
        None => 1,
    };
    if parts.next().is_some()
        || !(1..=12).contains(&month)
        || !(1..=days_in_month(year, month)).contains(&day)
    {
        return None;
    }
    let (mut hour, mut minute, mut second, mut ms) = (0, 0, 0, 0);
    if let Some(time) = time {
        let (hms, frac) = match time.split_once('.') {
            Some((a, b)) => (a, Some(b)),
            None => (time, None),
        };
        let mut f = hms.split(':');
        hour = field(f.next(), 2)?;
        minute = field(f.next(), 2)?;
        if let Some(sec) = f.next() {
            second = field(Some(sec), 2)?;
        }
        if f.next().is_some() || hour > 24 || minute > 59 || second > 59 {
            return None;
        }
        if let Some(frac) = frac {
            if frac.is_empty() || frac.len() > 3 || digits(frac, 3) != frac.len() {
                return None;
            }
            ms = frac.parse::<i64>().ok()? * 10_i64.pow(3 - frac.len() as u32);
        }
    }
    Some(
        Parts {
            year,
            month,
            day,
            hour,
            minute,
            second,
            ms,
        }
        .time(),
    )
}

// d3 formats.

/// The ISO 8601 week-numbering year and week of `t`.
fn iso_week(t: Time) -> (i64, i64) {
    let days = t.div_euclid(DAY);
    // Monday 0 .. Sunday 6.
    let wd = (weekday(t) + 6) % 7;
    let thursday = days - wd + 3;
    let (y, _, _) = civil_from_days(thursday);
    let jan1 = days_from_civil(y, 1, 1);
    (y, (thursday - jan1) / 7 + 1)
}

/// `t` written in the d3 time `format`, in the en-US locale (MGANTT-2).
pub fn format_d3(t: Time, format: &str) -> String {
    let p = Parts::of(t);
    let yday = t.div_euclid(DAY) - days_from_civil(p.year, 1, 1);
    let wday = weekday(t);
    let mut out = String::new();
    let mut chars = format.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let mut pad = None;
        if let Some(&f) = chars.peek()
            && matches!(f, '-' | '_' | '0')
        {
            pad = Some(f);
            chars.next();
        }
        let Some(d) = chars.next() else {
            out.push('%');
            break;
        };
        // A number padded to `width` with the flag or the directive's default fill.
        let num = |v: i64, width: usize, fill: char| -> String {
            match pad.unwrap_or(fill) {
                '-' => v.to_string(),
                '_' => format!("{v:>width$}"),
                _ => format!("{v:0width$}"),
            }
        };
        let s = match d {
            'a' => weekday_name(wday)[..3].to_owned(),
            'A' => weekday_name(wday).to_owned(),
            'b' => MONTHS[p.month as usize - 1][..3].to_owned(),
            'B' => MONTHS[p.month as usize - 1].to_owned(),
            'c' => format_d3(t, "%x, %X"),
            'd' => num(p.day, 2, '0'),
            'e' => num(p.day, 2, '_'),
            'f' => num(p.ms * 1000, 6, '0'),
            'g' => num(iso_week(t).0.rem_euclid(100), 2, '0'),
            'G' => num(iso_week(t).0, 4, '0'),
            'H' => num(p.hour, 2, '0'),
            'I' => num(hour12(p.hour), 2, '0'),
            'j' => num(yday + 1, 3, '0'),
            'L' => num(p.ms, 3, '0'),
            'm' => num(p.month, 2, '0'),
            'M' => num(p.minute, 2, '0'),
            'p' => (if p.hour < 12 { "AM" } else { "PM" }).to_owned(),
            'q' => ((p.month - 1) / 3 + 1).to_string(),
            'Q' => t.to_string(),
            's' => t.div_euclid(SECOND).to_string(),
            'S' => num(p.second, 2, '0'),
            'u' => (if wday == 0 { 7 } else { wday }).to_string(),
            'U' => num((yday + 7 - wday) / 7, 2, '0'),
            'V' => num(iso_week(t).1, 2, '0'),
            'w' => wday.to_string(),
            'W' => num((yday + 7 - (wday + 6) % 7) / 7, 2, '0'),
            'x' => format_d3(t, "%-m/%-d/%Y"),
            'X' => format_d3(t, "%-I:%M:%S %p"),
            'y' => num(p.year.rem_euclid(100), 2, '0'),
            'Y' => num(p.year, 4, '0'),
            'Z' => "+0000".to_owned(),
            '%' => "%".to_owned(),
            other => format!("%{other}"),
        };
        out.push_str(&s);
    }
    out
}

// Ticks.

/// A unit of a tick interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Millisecond,
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

/// Every `step` units, on whole units (MGANTT-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    pub unit: Unit,
    pub step: i64,
}

impl Interval {
    /// `tickInterval` text such as `1day` or `2week`.
    pub fn parse(s: &str) -> Option<Interval> {
        let n = digits(s, 18);
        let step: i64 = s[..n].parse().ok().filter(|&v| v > 0)?;
        if s.starts_with('0') {
            return None;
        }
        let unit = match &s[n..] {
            "millisecond" => Unit::Millisecond,
            "second" => Unit::Second,
            "minute" => Unit::Minute,
            "hour" => Unit::Hour,
            "day" => Unit::Day,
            "week" => Unit::Week,
            "month" => Unit::Month,
            _ => return None,
        };
        Some(Interval { unit, step })
    }
}

/// The start of the unit holding `t`; weeks start on `weekday` (0 for Sunday).
fn floor(t: Time, unit: Unit, weekday_start: i64) -> Time {
    let p = Parts::of(t);
    match unit {
        Unit::Millisecond => t,
        Unit::Second => t.div_euclid(SECOND) * SECOND,
        Unit::Minute => t.div_euclid(MINUTE) * MINUTE,
        Unit::Hour => t.div_euclid(HOUR) * HOUR,
        Unit::Day => day_start(t),
        Unit::Week => day_start(t) - (weekday(t) - weekday_start).rem_euclid(7) * DAY,
        Unit::Month => Parts {
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
            ms: 0,
            ..p
        }
        .time(),
        Unit::Year => Parts {
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
            ms: 0,
            ..p
        }
        .time(),
    }
}

fn next(t: Time, unit: Unit, n: i64) -> Time {
    match unit {
        Unit::Millisecond => t + n,
        Unit::Second => t + n * SECOND,
        Unit::Minute => t + n * MINUTE,
        Unit::Hour => t + n * HOUR,
        Unit::Day => t + n * DAY,
        Unit::Week => t + n * WEEK,
        Unit::Month => add_months(t, n),
        Unit::Year => add_months(t, 12 * n),
    }
}

/// Whether a unit start `t` is one d3's `every(step)` keeps: the unit's field divides by the
/// step, or for weeks the count of weeks since 1970.
fn kept(t: Time, i: Interval, weekday_start: i64) -> bool {
    if i.step <= 1 {
        return true;
    }
    let p = Parts::of(t);
    let field = match i.unit {
        Unit::Millisecond => t,
        Unit::Second => p.second,
        Unit::Minute => p.minute,
        Unit::Hour => p.hour,
        Unit::Day => p.day - 1,
        Unit::Month => p.month - 1,
        Unit::Year => p.year,
        Unit::Week => {
            let first = next(floor(0, Unit::Week, weekday_start), Unit::Week, 1);
            (t - first).div_euclid(WEEK) + 1
        }
    };
    field.rem_euclid(i.step) == 0
}

/// d3's `tickStep`: a round step near `(stop - start) / count`.
fn tick_step(start: f64, stop: f64, count: f64) -> f64 {
    let step = (stop - start) / count;
    let power = step.log10().floor();
    let error = step / 10f64.powf(power);
    let factor = if error >= 50f64.sqrt() {
        10.0
    } else if error >= 10f64.sqrt() {
        5.0
    } else if error >= 2f64.sqrt() {
        2.0
    } else {
        1.0
    };
    factor * 10f64.powf(power)
}

/// The interval d3 picks for about ten ticks from `start` to `stop`.
pub fn auto_interval(start: Time, stop: Time) -> Interval {
    use Unit::*;
    const YEAR: i64 = 365 * DAY;
    const MONTH: i64 = 30 * DAY;
    let table: [(Unit, i64, i64); 18] = [
        (Second, 1, SECOND),
        (Second, 5, 5 * SECOND),
        (Second, 15, 15 * SECOND),
        (Second, 30, 30 * SECOND),
        (Minute, 1, MINUTE),
        (Minute, 5, 5 * MINUTE),
        (Minute, 15, 15 * MINUTE),
        (Minute, 30, 30 * MINUTE),
        (Hour, 1, HOUR),
        (Hour, 3, 3 * HOUR),
        (Hour, 6, 6 * HOUR),
        (Hour, 12, 12 * HOUR),
        (Day, 1, DAY),
        (Day, 2, 2 * DAY),
        (Week, 1, WEEK),
        (Month, 1, MONTH),
        (Month, 3, 3 * MONTH),
        (Year, 1, YEAR),
    ];
    let target = (stop - start).abs() as f64 / 10.0;
    let i = table.partition_point(|&(_, _, d)| (d as f64) <= target);
    if i == table.len() {
        let step = tick_step(start as f64 / YEAR as f64, stop as f64 / YEAR as f64, 10.0);
        return Interval {
            unit: Year,
            step: (step.round() as i64).max(1),
        };
    }
    if i == 0 {
        let step = tick_step(start as f64, stop as f64, 10.0);
        return Interval {
            unit: Millisecond,
            step: (step.round() as i64).max(1),
        };
    }
    let (lo, hi) = (table[i - 1], table[i]);
    let (unit, step, _) = if target / (lo.2 as f64) < (hi.2 as f64) / target {
        lo
    } else {
        hi
    };
    Interval { unit, step }
}

/// The ticks from `start` to `stop`, both included, every `interval`.
pub fn ticks(start: Time, stop: Time, interval: Interval, weekday_start: i64) -> Vec<Time> {
    let mut out = Vec::new();
    let mut t = floor(start, interval.unit, weekday_start);
    if interval.unit == Unit::Millisecond {
        t = start.div_euclid(interval.step) * interval.step;
        if t < start {
            t += interval.step;
        }
        while t <= stop && out.len() < 10_000 {
            out.push(t);
            t += interval.step;
        }
        return out;
    }
    if interval.unit == Unit::Year && interval.step > 1 {
        let p = Parts::of(t);
        let y = p.year.div_euclid(interval.step) * interval.step;
        t = Parts {
            year: y,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
            ms: 0,
        }
        .time();
    }
    if t < start {
        let n = if interval.unit == Unit::Year {
            interval.step
        } else {
            1
        };
        t = next(t, interval.unit, n);
    }
    // Years every N step N at a time; the others step one unit and keep every Nth.
    let (stride, filter) = match interval.unit {
        Unit::Year => (interval.step, false),
        _ => (1, true),
    };
    let mut k = 0;
    let base = t;
    while out.len() < 10_000 {
        let at = next(base, interval.unit, k * stride);
        if at > stop {
            break;
        }
        if at >= start && (!filter || kept(at, interval, weekday_start)) {
            out.push(at);
        }
        k += 1;
    }
    out
}

/// A duration such as `3d`, `1.5h`, or `500ms`, added to `t` as dayjs adds it: days and weeks
/// rounded to whole days, months and years truncated.
pub fn add_duration(t: Time, s: &str) -> Option<Time> {
    let n = s
        .bytes()
        .take_while(|b| b.is_ascii_digit() || *b == b'.')
        .count();
    let (num, unit) = s.split_at(n);
    if num.is_empty() || num.starts_with('.') || num.ends_with('.') || num.matches('.').count() > 1
    {
        return None;
    }
    let v: f64 = num.parse().ok()?;
    Some(match unit {
        "ms" => t + v.round() as i64,
        "s" => t + (v * SECOND as f64).round() as i64,
        "m" => t + (v * MINUTE as f64).round() as i64,
        "h" => t + (v * HOUR as f64).round() as i64,
        "d" => t + v.round() as i64 * DAY,
        "w" => t + (v * 7.0).round() as i64 * DAY,
        "M" => add_months(t, v.trunc() as i64),
        "y" => add_months(t, v.trunc() as i64 * 12),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i64, m: i64, d: i64) -> Time {
        days_from_civil(y, m, d) * DAY
    }

    #[test]
    fn calendar() {
        assert_eq!(date(1970, 1, 1), 0);
        for days in [-800_000, -1, 0, 59, 11_016, 2_000_000] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
        assert_eq!(weekday(date(2014, 1, 6)), 1);
        assert_eq!(add_months(date(2014, 1, 31), 1), date(2014, 2, 28));
        assert_eq!(add_months(date(2016, 1, 31), 1), date(2016, 2, 29));
        assert_eq!(add_months(date(2014, 3, 15), -14), date(2013, 1, 15));
        assert_eq!(weekday_of("Monday"), Some(1));
        assert_eq!(weekday_of("funday"), None);
    }

    #[test]
    fn dayjs() {
        let now = date(2020, 6, 15);
        let at = |s, f| parse_dayjs(s, f, now);
        assert_eq!(at("2014-01-06", "YYYY-MM-DD"), Some(date(2014, 1, 6)));
        assert_eq!(at("2014-1-6", "YYYY-MM-DD"), None);
        assert_eq!(at("2014-02-30", "YYYY-MM-DD"), None);
        assert_eq!(at("6/1/14", "D/M/YY"), Some(date(2014, 1, 6)));
        assert_eq!(at("01/1/14", "D/M/YY"), None);
        assert_eq!(at("6th Jan 2014", "Do MMM YYYY"), Some(date(2014, 1, 6)));
        assert_eq!(
            at("January 6, 2014 3:05 pm", "MMMM D, YYYY h:mm a"),
            Some(date(2014, 1, 6) + 15 * HOUR + 5 * MINUTE)
        );
        assert_eq!(at("12:00 AM", "hh:mm A"), Some(now));
        assert_eq!(at("09:30", "HH:mm"), Some(now + 9 * HOUR + 30 * MINUTE));
        assert_eq!(
            at("2014-01-06 10:00:00.250", "YYYY-MM-DD HH:mm:ss.SSS"),
            Some(date(2014, 1, 6) + 10 * HOUR + 250)
        );
        assert_eq!(at("1389000000", "X"), Some(1_389_000_000_000));
        assert_eq!(at("1389000000123", "x"), Some(1_389_000_000_123));
        assert_eq!(
            at("2014-01-06T10:00+02:00", "YYYY-MM-DDTHH:mmZ"),
            Some(date(2014, 1, 6) + 8 * HOUR)
        );
        assert_eq!(at("Y2014", "[Y]YYYY"), Some(date(2014, 1, 1)));
        assert_eq!(at("2014", "YYYY"), Some(date(2014, 1, 1)));
        assert_eq!(at("Mon 2014", "ddd YYYY"), None);
        assert_eq!(at("2014-01-06 extra", "YYYY-MM-DD"), None);
        let t = date(2014, 1, 6) + 15 * HOUR + 5 * MINUTE + 7 * SECOND + 89;
        assert_eq!(
            format_dayjs(
                t,
                "dddd ddd dd d Do MMMM MMM Q YY h hh k kk A a s ss S SS SSS Z ZZ"
            ),
            "Monday Mon Mo 1 6th January Jan 1 14 3 03 15 15 PM pm 7 07 0 08 089 +00:00 +0000"
        );
        assert_eq!(format_dayjs(0, "X x [at] H"), "0 0 at 0");
        for (n, s) in [
            (1, "1st"),
            (2, "2nd"),
            (3, "3rd"),
            (4, "4th"),
            (11, "11th"),
            (22, "22nd"),
        ] {
            assert_eq!(format!("{n}{}", ordinal(n)), s);
        }
    }

    #[test]
    fn iso() {
        assert_eq!(parse_iso("2014-01-06"), Some(date(2014, 1, 6)));
        assert_eq!(parse_iso("2014-01"), Some(date(2014, 1, 1)));
        assert_eq!(
            parse_iso("2014-01-06T09:30:15.5Z"),
            Some(date(2014, 1, 6) + 9 * HOUR + 30 * MINUTE + 15 * SECOND + 500)
        );
        assert_eq!(
            parse_iso("2014-01-06 09:30"),
            Some(date(2014, 1, 6) + 9 * HOUR + 30 * MINUTE)
        );
        for bad in [
            "2014-13-01",
            "14-01-06",
            "2014-01-06T9:30",
            "x",
            "2014-01-06T09:30.",
            "",
        ] {
            assert_eq!(parse_iso(bad), None, "{bad}");
        }
    }

    #[test]
    fn d3() {
        let t = date(2014, 1, 6) + 15 * HOUR + 5 * MINUTE + 7 * SECOND + 89;
        assert_eq!(format_d3(t, "%Y-%m-%d"), "2014-01-06");
        assert_eq!(
            format_d3(
                t,
                "%a %A %b %B %d %e %-d %_m %H %I %j %L %f %M %p %q %S %u %w %y %Z %%"
            ),
            "Mon Monday Jan January 06  6 6  1 15 03 006 089 089000 05 PM 1 07 1 1 14 +0000 %"
        );
        assert_eq!(format_d3(t, "%c"), "1/6/2014, 3:05:07 PM");
        assert_eq!(format_d3(t, "%s %Q"), "1389020707 1389020707089");
        // 2014-01-06 is in ISO week 2; week numbers counted from the first Sunday and Monday.
        assert_eq!(format_d3(t, "%G %g %V %U %W"), "2014 14 02 01 01");
        assert_eq!(format_d3(date(2016, 1, 1), "%G-W%V"), "2015-W53");
        assert_eq!(format_d3(date(2014, 1, 5), "%u %U"), "7 01");
        assert_eq!(format_d3(t, "%k %"), "%k %");
    }

    #[test]
    fn intervals() {
        let i = |s| Interval::parse(s);
        assert_eq!(
            i("2week"),
            Some(Interval {
                unit: Unit::Week,
                step: 2
            })
        );
        assert_eq!(i("1millisecond").unwrap().unit, Unit::Millisecond);
        for bad in ["0day", "day", "1year", "01day", "1 day"] {
            assert_eq!(i(bad), None, "{bad}");
        }
        let auto = |span| auto_interval(0, span);
        assert_eq!(
            auto(30 * DAY),
            Interval {
                unit: Unit::Day,
                step: 2
            }
        );
        assert_eq!(
            auto(60 * DAY),
            Interval {
                unit: Unit::Week,
                step: 1
            }
        );
        assert_eq!(
            auto(2 * HOUR),
            Interval {
                unit: Unit::Minute,
                step: 15
            }
        );
        assert_eq!(auto(30 * 365 * DAY).unit, Unit::Year);
        assert_eq!(auto(30 * 365 * DAY).step, 2);
        assert_eq!(
            auto(50),
            Interval {
                unit: Unit::Millisecond,
                step: 5
            }
        );
    }

    #[test]
    fn tick_times() {
        let start = date(2014, 1, 6) + HOUR;
        let days = ticks(
            start,
            date(2014, 1, 12),
            Interval {
                unit: Unit::Day,
                step: 2,
            },
            0,
        );
        assert_eq!(
            days,
            [date(2014, 1, 7), date(2014, 1, 9), date(2014, 1, 11)]
        );
        let weeks = ticks(
            date(2014, 1, 1),
            date(2014, 1, 31),
            Interval {
                unit: Unit::Week,
                step: 1,
            },
            1,
        );
        assert_eq!(weeks[0], date(2014, 1, 6));
        assert_eq!(weeks.len(), 4);
        let two = ticks(
            date(2014, 1, 1),
            date(2014, 3, 1),
            Interval {
                unit: Unit::Week,
                step: 2,
            },
            0,
        );
        assert!(two.windows(2).all(|w| w[1] - w[0] == 2 * WEEK));
        let months = ticks(
            date(2014, 1, 15),
            date(2014, 12, 31),
            Interval {
                unit: Unit::Month,
                step: 3,
            },
            0,
        );
        assert_eq!(
            months,
            [date(2014, 4, 1), date(2014, 7, 1), date(2014, 10, 1)]
        );
        let years = ticks(
            date(2001, 6, 1),
            date(2031, 1, 1),
            Interval {
                unit: Unit::Year,
                step: 5,
            },
            0,
        );
        assert_eq!(years[0], date(2005, 1, 1));
        assert_eq!(years.len(), 6);
        let ms = ticks(
            3,
            21,
            Interval {
                unit: Unit::Millisecond,
                step: 5,
            },
            0,
        );
        assert_eq!(ms, [5, 10, 15, 20]);
        let hours = ticks(
            0,
            7 * HOUR,
            Interval {
                unit: Unit::Hour,
                step: 3,
            },
            0,
        );
        assert_eq!(hours, [0, 3 * HOUR, 6 * HOUR]);
        let mins = ticks(
            0,
            20 * MINUTE,
            Interval {
                unit: Unit::Minute,
                step: 5,
            },
            0,
        );
        assert_eq!(mins.len(), 5);
        let secs = ticks(
            0,
            40 * SECOND,
            Interval {
                unit: Unit::Second,
                step: 15,
            },
            0,
        );
        assert_eq!(secs.len(), 3);
    }

    #[test]
    fn durations() {
        let t = date(2014, 1, 31);
        assert_eq!(add_duration(t, "3d"), Some(date(2014, 2, 3)));
        assert_eq!(add_duration(t, "1.5d"), Some(date(2014, 2, 2)));
        assert_eq!(add_duration(t, "1w"), Some(date(2014, 2, 7)));
        assert_eq!(add_duration(t, "1M"), Some(date(2014, 2, 28)));
        assert_eq!(add_duration(t, "1.9y"), Some(date(2015, 1, 31)));
        assert_eq!(add_duration(t, "1.5h"), Some(t + 90 * MINUTE));
        assert_eq!(add_duration(t, "2m"), Some(t + 2 * MINUTE));
        assert_eq!(add_duration(t, "10s"), Some(t + 10 * SECOND));
        assert_eq!(add_duration(t, "500ms"), Some(t + 500));
        for bad in ["d", "3", "3x", ".5d", "1.d", "1.2.3d", "-1d"] {
            assert_eq!(add_duration(t, bad), None, "{bad}");
        }
    }
}
