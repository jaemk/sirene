//! Mermaid gantt charts (MGANTT-*): the fixture corpus with its diagnostics and drawings
//! (LANGS-6), and the model each statement builds.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind, Lowered};
use sirene_core::gantt::time::{DAY, HOUR, MINUTE, Parts, format_d3};
use sirene_core::gantt::{self, Gantt, Interval, Time, TodayMarker, Unit};
use sirene_core::scene::{Approx, Owner, Primitive, Pt, Scene, Theme};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gantt");
    let mut out: Vec<(String, String)> = fs::read_dir(root.join(dir))
        .expect("fixture dir exists")
        .map(|e| e.expect("readable entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "mmd"))
        .map(|p| {
            let name = p
                .file_stem()
                .expect("file name")
                .to_string_lossy()
                .into_owned();
            (name, fs::read_to_string(&p).expect("readable fixture"))
        })
        .collect();
    out.sort();
    out
}

fn diagnostics(src: &str, lowered: &Lowered) -> String {
    let mut out = String::new();
    for d in &lowered.diagnostics {
        let (line, col) = d.line_col(src);
        let severity = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        writeln!(out, "{line}:{col}: {severity}: {}", d.message).unwrap();
    }
    out
}

/// Each fixture's diagnostics and drawing; the tree always holds the source exactly.
#[test]
fn snapshots() {
    for dir in ["valid", "errors"] {
        for (name, src) in fixtures(dir) {
            assert_eq!(gantt::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::Gantt, "{name}");
            let lowered = diagram::lower(&src);
            if dir == "valid" {
                assert!(
                    lowered.diagnostics.is_empty(),
                    "{name}: {:?}",
                    lowered.diagnostics
                );
            } else {
                assert!(!lowered.diagnostics.is_empty(), "{name}");
            }
            let mut out = diagnostics(&src, &lowered);
            out.push('\n');
            out.push_str(&lowered.diagram.ascii(&Options {
                charset: Charset::Unicode,
            }));
            insta::assert_snapshot!(format!("{dir}__{name}"), out, &src);
            for theme in [Theme::LIGHT, Theme::DARK] {
                let s = lowered.diagram.scene(&Approx, &theme);
                assert!(!s.elements.is_empty(), "{name}");
                assert!(s.elements.iter().all(|e| e.owner == Owner::Diagram));
            }
        }
    }
}

#[test]
fn ascii_charset_snapshots() {
    for (name, src) in fixtures("valid") {
        let out = diagram::lower(&src).diagram.ascii(&Options {
            charset: Charset::Ascii,
        });
        insta::assert_snapshot!(format!("ascii__{name}"), out, &src);
    }
}

fn date(y: i64, m: i64, d: i64) -> Time {
    Parts {
        year: y,
        month: m,
        day: d,
        hour: 0,
        minute: 0,
        second: 0,
        ms: 0,
    }
    .time()
}

/// A fixed time to read charts at: 2020-06-15 12:00.
const NOW: Time = 1_592_222_400_000;

fn chart(src: &str) -> Gantt {
    let (g, d) = gantt::parse(src).lower_at(NOW);
    assert!(d.is_empty(), "{d:?}\n{src}");
    g
}

fn task<'a>(g: &'a Gantt, name: &str) -> &'a gantt::Task {
    g.tasks()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("no task {name}"))
}

/// MGANTT-1, MGANTT-3: starts, ends, `after`, `until`, tags, and sections.
#[test]
fn tasks() {
    let g = chart(
        "gantt\n  title T\n  early : e, 2014-01-01, 1d\n  section S\n  a : a, 2014-01-02, 3d\n  \
         b : after a e, 2d\n  c : 1w\n  d : crit, active, 2014-01-20, until z\n  \
         z : milestone, z, 2014-01-25, 0d\n  v : vert, 2014-01-03, 0d\n",
    );
    assert_eq!(g.title.as_deref(), Some("T"));
    let names: Vec<Option<&str>> = g.sections.iter().map(|s| s.name.as_deref()).collect();
    assert_eq!(names, [None, Some("S")]);
    let span = |n: &str| {
        let t = task(&g, n);
        (t.start, t.end)
    };
    assert_eq!(span("early"), (date(2014, 1, 1), date(2014, 1, 2)));
    assert_eq!(span("a"), (date(2014, 1, 2), date(2014, 1, 5)));
    // `after` takes the latest end; no START follows the task before.
    assert_eq!(span("b"), (date(2014, 1, 5), date(2014, 1, 7)));
    assert_eq!(span("c"), (date(2014, 1, 7), date(2014, 1, 14)));
    // `until` ends at a task declared later.
    assert_eq!(span("d"), (date(2014, 1, 20), date(2014, 1, 25)));
    let d = task(&g, "d");
    assert!(d.tags.crit && d.tags.active && !d.tags.done);
    assert!(task(&g, "z").tags.milestone);
    assert_eq!(task(&g, "a").id.as_deref(), Some("a"));
    assert_eq!(task(&g, "c").id, None);
    // A `vert` task takes no row; compact rows share.
    assert_eq!(g.rows().len(), 6);
    assert!(task(&g, "v").tags.vert);
    // Date ends, inclusive or not.
    let g = chart("gantt\n  x : 2014-01-01, 2014-01-03\n");
    assert_eq!(task(&g, "x").end, date(2014, 1, 3));
    let g = chart("gantt\n  inclusiveEndDates\n  x : 2014-01-01, 2014-01-03\n");
    assert_eq!(task(&g, "x").end, date(2014, 1, 4));
}

/// MGANTT-2: `dateFormat` read strictly, the ISO fallback, and times.
#[test]
fn dates() {
    let g = chart("gantt\n  dateFormat D MMM YYYY h:mm a\n  x : 6 Jan 2014 3:30 pm, 2h\n");
    let x = task(&g, "x");
    assert_eq!(x.start, date(2014, 1, 6) + 15 * HOUR + 30 * MINUTE);
    assert_eq!(x.end, x.start + 2 * HOUR);
    // A date in another form falls back to ISO 8601.
    let g = chart("gantt\n  dateFormat DD.MM.YYYY\n  x : 2014-01-06T08:00, 06.01.2014\n");
    assert_eq!(task(&g, "x").start, date(2014, 1, 6) + 8 * HOUR);
    // Missing fields come from the time the chart is read.
    let g = chart("gantt\n  dateFormat HH:mm\n  x : 09:00, 30m\n");
    assert_eq!(task(&g, "x").start, date(2020, 6, 15) + 9 * HOUR);
    let g = chart("gantt\n  dateFormat X\n  x : 1389000000, 1d\n");
    assert_eq!(task(&g, "x").start, 1_389_000_000_000);
}

/// MGANTT-2: the axis format, and settings from frontmatter and directives.
#[test]
fn formats() {
    let g = chart("gantt\n  axisFormat %d %b, week %V\n  x : 2014-01-06, 1d\n");
    assert_eq!(g.label(date(2014, 1, 6)), "06 Jan, week 02");
    assert_eq!(format_d3(date(2014, 1, 6), "%-m/%-d"), "1/6");
}

#[test]
fn config() {
    let g = chart(
        "---\ntitle: Plan\nconfig:\n  gantt:\n    axisFormat: '%m/%d'\n    tickInterval: 2day\n    \
         topAxis: true\n    displayMode: compact\n    weekday: monday\n    dateFormat: DD.MM.YYYY\n---\n\
         gantt\n  x : 06.01.2014, 1d\n",
    );
    assert_eq!(g.title.as_deref(), Some("Plan"));
    assert_eq!(g.axis_format, "%m/%d");
    assert_eq!(
        g.tick_interval,
        Some(Interval {
            unit: Unit::Day,
            step: 2
        })
    );
    assert!(g.top_axis && g.compact);
    assert_eq!(g.weekday, 1);
    assert_eq!(task(&g, "x").start, date(2014, 1, 6));
    // A statement in the diagram wins over the directive.
    let g = chart(
        "%%{init: {'gantt': {'axisFormat': '%Y', 'topAxis': 'true'}}}%%\ngantt\n  axisFormat %d\n  \
         x : 2014-01-06, 1d\n",
    );
    assert_eq!(g.axis_format, "%d");
    assert!(g.top_axis);
    let (_, d) = gantt::parse(
        "---\nconfig:\n  gantt:\n    tickInterval: often\n    weekday: x\n---\ngantt\n",
    )
    .lower_at(NOW);
    assert_eq!(d.len(), 2);
}

/// MGANTT-3: excluded days push a duration's end later and are not drawn.
#[test]
fn excludes() {
    // 2014-01-09 is a Thursday: three days run Thu, Fri, then Mon past the weekend.
    let g = chart("gantt\n  excludes weekends\n  x : 2014-01-09, 3d\n");
    let x = task(&g, "x");
    assert_eq!(x.end, date(2014, 1, 14));
    assert_eq!(x.drawn_end, date(2014, 1, 14));
    // Ending on the weekend: the bar stops at Saturday, the end moves to Monday.
    let g = chart("gantt\n  excludes weekends\n  x : 2014-01-09, 2d\n");
    let x = task(&g, "x");
    assert_eq!((x.end, x.drawn_end), (date(2014, 1, 13), date(2014, 1, 11)));
    // A date end is kept.
    let g = chart("gantt\n  excludes weekends\n  x : 2014-01-09, 2014-01-13\n");
    assert_eq!(task(&g, "x").end, date(2014, 1, 13));
    // Friday weekends, a weekday name, a date, and an included date.
    let g = chart(
        "gantt\n  excludes Weekends,tuesday 2014-01-08\n  includes 2014-01-11\n  weekend friday\n  \
         x : 2014-01-06, 1d\n",
    );
    let off: Vec<bool> = (6..=12).map(|d| g.is_excluded(date(2014, 1, d))).collect();
    // Mon 6, Tue 7, Wed 8, Thu 9, Fri 10, Sat 11 (included), Sun 12.
    assert_eq!(off, [false, true, true, false, true, false, false]);
    // One day from Monday: Tuesday and Wednesday are excluded, so it ends on Thursday.
    assert_eq!(task(&g, "x").end, date(2014, 1, 9));
    assert_eq!(g.excluded_days(), [date(2014, 1, 7), date(2014, 1, 8)]);
    let g = chart("gantt\n  excludes weekends\n  x : 2014-01-09, 3d\n");
    assert_eq!(g.excluded_days(), [date(2014, 1, 11), date(2014, 1, 12)]);
}

/// MGANTT-3: with every weekday excluded no end would settle; `excludes` is ignored, once.
#[test]
fn endless_excludes() {
    for days in [
        "monday tuesday wednesday thursday friday saturday sunday",
        "weekends, monday, tuesday, wednesday, thursday, friday, 2014-01-10",
    ] {
        let src = format!("gantt\n  excludes {days}\n  x : 2014-01-09, 3d\n  y : 2014-01-09, 2d\n");
        let (g, d) = gantt::parse(&src).lower_at(NOW);
        let messages: Vec<&str> = d.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(
            messages,
            ["`excludes` leaves no day to work on; ignoring it"],
            "{days}"
        );
        assert_eq!(task(&g, "x").end, date(2014, 1, 12));
        assert_eq!(task(&g, "y").drawn_end, date(2014, 1, 11));
    }
    // A task too long to walk keeps its end and leaves `excludes` on for the others.
    let (g, d) = gantt::parse(
        "gantt\n  excludes weekends\n  x : 2014-01-09, 2000000d\n  y : 2014-01-09, 2d\n",
    )
    .lower_at(NOW);
    let messages: Vec<&str> = d.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(
        messages,
        ["this task is too long to apply `excludes` to; ignoring it here"]
    );
    assert_eq!(task(&g, "y").end, date(2014, 1, 13));
    // A duration past the dates a JavaScript `Date` holds is an error, not an overflow.
    let (_, d) = gantt::parse("gantt\n  x : 2014-01-09, 99999999999999999999d\n").lower_at(NOW);
    assert_eq!(d.len(), 1);
    // Six weekdays and a run of dates still leave a day.
    let g = chart(
        "gantt\n  excludes monday tuesday wednesday thursday friday saturday 2014-01-12 2014-01-19\n  \
         x : 2014-01-09, 1d\n",
    );
    assert_eq!(task(&g, "x").end, date(2014, 1, 26));
}

/// MGANTT-3: a task waiting for one that cannot be dated is left out without a second error,
/// in any source order.
#[test]
fn waits() {
    let (g, d) = gantt::parse(
        "gantt\n  a : a, after b, 1d\n  b : b, after c, 1d\n  c : c, 2014-02-30, 1d\n  \
         d : d, after e f, 1d\n  e : e, 2014-01-01, until f\n  f : f, 2014-01-03, 1d\n  g : 2d\n",
    )
    .lower_at(NOW);
    let messages: Vec<&str> = d.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(messages, ["`2014-02-30` is not a date in `YYYY-MM-DD`"]);
    let names: Vec<&str> = g.tasks().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["d", "e", "f", "g"]);
    assert_eq!(task(&g, "e").end, date(2014, 1, 3));
    assert_eq!(task(&g, "d").start, date(2014, 1, 4));
    assert_eq!(task(&g, "g").end, date(2014, 1, 6));
}

/// MGANTT-3: what cannot be read or dated is an error, and the task is skipped.
#[test]
fn errors() {
    let (g, d) = gantt::parse(
        "gantt\n  first : 3d\n  a : a, 2014-01-01, 2d\n  b : a, 2014-01-03, 2d\n  c : after x, 1d\n  \
         d : 2014-02-30, 1d\n  e : 2014-01-01, soon\n  f : h, after g, 1d\n  g : g, after f2, 1d\n  \
         f2 : f2, 2014-01-01, until g\n  click a call cb()\n",
    )
    .lower_at(NOW);
    let got: Vec<(Severity, &str)> = d.iter().map(|d| (d.severity, d.message.as_str())).collect();
    assert_eq!(
        got,
        [
            (
                Severity::Error,
                "`first` has no start and no task before it"
            ),
            (Severity::Error, "`a` is already a task id"),
            (Severity::Error, "no task has the id `x`"),
            (
                Severity::Error,
                "`2014-02-30` is not a date in `YYYY-MM-DD`"
            ),
            (
                Severity::Error,
                "`soon` is not an end date in `YYYY-MM-DD`, a duration such as `3d`, or `until ID`"
            ),
            (Severity::Error, CIRCLE_F),
            (Severity::Error, CIRCLE_G),
            (Severity::Error, CIRCLE_F2),
            (
                Severity::Warning,
                "`click` is not supported in gantt charts; ignoring it"
            ),
        ]
    );
    let names: Vec<&str> = g.tasks().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["a"]);
}

// `g` and `f2` wait on each other, and `f` waits on `g`.
const CIRCLE_F: &str = "`f` cannot be dated: its `after` and `until` tasks wait on each other";
const CIRCLE_G: &str = "`g` cannot be dated: its `after` and `until` tasks wait on each other";
const CIRCLE_F2: &str = "`f2` cannot be dated: its `after` and `until` tasks wait on each other";

#[test]
fn syntax_errors() {
    let errors = |src: &str| -> Vec<String> {
        diagram::lower(src)
            .diagnostics
            .into_iter()
            .map(|d| d.message)
            .collect()
    };
    assert_eq!(
        errors("gantt\n  tickInterval 1year\n"),
        ["`1year` is not an interval; expected a number and a unit, such as `1day` or `2week`"]
    );
    assert_eq!(
        errors("gantt\n  weekend sunday\n"),
        ["`sunday` cannot start the weekend; expected `friday` or `saturday`"]
    );
    assert_eq!(
        errors("gantt\n  words\n"),
        ["expected a task (`name : dates`) or a setting"]
    );
    assert!(errors("gantt\n").is_empty());
}

/// MGANTT-4: ticks d3 would pick, or every `tickInterval`, starting weeks on `weekday`.
#[test]
fn ticks() {
    let g = chart("gantt\n  x : 2014-01-01, 30d\n");
    assert_eq!(g.ticks()[0], date(2014, 1, 1));
    assert!(g.ticks().windows(2).all(|w| w[1] - w[0] == 2 * DAY));
    let g = chart("gantt\n  tickInterval 1week\n  weekday monday\n  x : 2014-01-01, 30d\n");
    assert_eq!(g.ticks()[0], date(2014, 1, 6));
    assert_eq!(g.ticks().len(), 4);
    assert_eq!(g.label(g.ticks()[0]), "2014-01-06");
    assert!(chart("gantt\n").ticks().is_empty());
    assert_eq!(chart("gantt\n").range(), None);
}

fn texts(s: &Scene) -> Vec<(String, Pt)> {
    s.elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Text { text, center, .. } => Some((text.clone(), *center)),
            _ => None,
        })
        .collect()
}

/// MGANTT-5: rows top to bottom, bars left to right, labels, and the today marker.
#[test]
fn drawing() {
    let src = "gantt\n  title Plan\n  section S\n  a : a, 2014-01-01, 3d\n  \
               b long name that does not fit : after a, 1d\n  m : milestone, 2014-01-06, 0d\n  \
               v : vert, 2014-01-02, 0d\n";
    let mut g = chart(src);
    let s = gantt_scene(&g);
    let t = texts(&s);
    let at = |want: &str| {
        t.iter()
            .find(|(text, _)| text == want)
            .map(|&(_, p)| p)
            .unwrap_or_else(|| panic!("no {want} in {t:?}"))
    };
    let (title, sec, a, b, m, v) = (
        at("Plan"),
        at("S"),
        at("a"),
        at("b long name that does not fit"),
        at("m"),
        at("v"),
    );
    assert!(title.y < v.y && v.y < a.y && a.y < b.y && b.y < m.y);
    assert!(sec.x < a.x && a.x < b.x);
    assert!(t.iter().any(|(text, _)| text == "2014-01-01"));
    let red = |s: &Scene| {
        s.elements.iter().any(|e| {
            matches!(&e.primitive, Primitive::Path { stroke: Some(st), .. } if st.color.r == 0xff && st.color.g == 0)
        })
    };
    assert!(!red(&s));
    g.now = date(2014, 1, 3);
    assert!(red(&gantt_scene(&g)));
    g.today_marker = TodayMarker::Off;
    assert!(!red(&gantt_scene(&g)));
    // Nothing to draw but the title.
    let s = gantt_scene(&chart("gantt\n  title Empty\n"));
    assert_eq!(texts(&s).len(), 1);
}

fn gantt_scene(g: &Gantt) -> Scene {
    Diagram::Gantt(g.clone()).scene(&Approx, &Theme::LIGHT)
}
