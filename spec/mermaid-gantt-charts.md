# Mermaid Gantt Charts

`gantt`, lowered to its own chart model (LANGS-2, MCHART-1). Reference:
<https://mermaid.js.org/syntax/gantt.html>. Code: `crates/sirene-core/src/gantt/`; `diagram::lower`
picks it by the header keyword. Tests: `crates/sirene-core/tests/gantt.rs` and the corpus under
`crates/sirene-core/tests/fixtures/gantt/` (LANGS-6).

## MGANTT-1 Syntax

- Header: `gantt`.
- Settings, one per line: `title`, `dateFormat FORMAT`, `axisFormat FORMAT`, `tickInterval
  INTERVAL`, `excludes LIST`, `includes LIST`, `todayMarker off|STYLE`, `weekday DAY`, `weekend
  friday|saturday`, `inclusiveEndDates`, `topAxis`, `section NAME`.
- A task: `NAME : DATA`, where DATA is comma-separated: leading tags (`active`, `done`, `crit`,
  `milestone`, `vert`), then `END`, `START, END`, or `ID, START, END`. START is a date or `after
  ID ID...`; END is a date, a duration (`3d`, `1.5h`, `2w`, `30m`, `10s`, `500ms`, `1M`, `1y`), or
  `until ID ID...`.
- `click ID href "URL"` and `click ID call fn(args)` are warnings (LANGS-6) and skipped.
- Comments (`%%`, and `#` to the end of a line), directives, frontmatter `title`, `accTitle`,
  `accDescr`. Keywords match in any case, as in Mermaid.

The parser shares `crate::lines` and is lossless. Tests: `snapshots`, `syntax_errors`.

## MGANTT-2 Dates

Dates and times have no time zone. `dateFormat` (default `YYYY-MM-DD`) is a dayjs format, read
strictly: the text must match it exactly and be a real date. The tokens are `YYYY`, `YY`, `M`,
`MM`, `MMM`, `MMMM`, `D`, `DD`, `Do`, `H`, `HH`, `h`, `hh`, `k`, `kk`, `a`, `A`, `m`, `mm`, `s`,
`ss`, `S`, `SS`, `SSS`, `Z`, `ZZ` (applied, giving UTC), `X` and `x` (Unix seconds and
milliseconds), and `[literal]`; the weekday tokens `d` to `dddd` and `Q` only format. A date that
does not match falls back to ISO 8601 (`2014-01-06`, `2014-01-06T09:30`, with seconds and
milliseconds) as Mermaid falls back to `Date`; anything else is an error. `axisFormat` (default
`%Y-%m-%d`) is a d3 time format: `%a %A %b %B %c %d %e %f %H %I %j %L %m %M %p %q %Q %s %S %u %U
%V %w %W %x %X %y %Y %Z %%`, with the `-`, `_`, and `0` padding flags, in the en-US locale.

Both formats, `tickInterval`, `topAxis`, `weekday`, and `displayMode: compact` may also be set in
frontmatter (`config: gantt: axisFormat: ...`) or an `%%{init: {"gantt": {...}}}%%` directive; a
statement in the diagram wins. Pixel settings (`barHeight`, `fontSize`, ...) are
ignored. Tests: `dates`, `formats`, `config`.

## MGANTT-3 Model

`gantt::Gantt`: the title, settings, sections in order (tasks before the first `section` are in
an unnamed one), and each section's tasks with their name, id, tags, start, end, and the end they
are drawn to. Times are `gantt::Time`, milliseconds on a calendar with no time zone.

A task's start is its date, the latest end of the `after` tasks, or, with no START, the end of
the task before it in the source. Its end is its date (plus a day with `inclusiveEndDates`), its
start plus the duration (days and weeks rounded to whole days, months and years truncated, as
dayjs adds them), or the earliest start of the `until` tasks. `after` and `until` may name tasks
declared later. When the end is a duration or `until` and `excludes` is set, each excluded day
from the day after the start through the end moves the end a day later, and the bar is drawn to
the last end that was not moved (Mermaid's `fixTaskDates`). When `excludes` names every weekday,
so that an end would move forever, it is an error on the first such task and `excludes` is
ignored. A task that waits for one that cannot be dated is not drawn, and tasks that wait on each
other are an error each. Dating visits each task once, whatever the order of the source. Tests:
`endless_excludes`, `waits`.

`excludes` holds `weekends`, weekday names, and dates in `dateFormat`, separated by commas or
spaces, matched in any case; `includes` holds dates that are never excluded. `weekend` sets which
two days are the weekend (`saturday`: Saturday and Sunday, the default; `friday`: Friday and
Saturday). Errors, each skipping the task or setting: an unknown `after` or `until` id, a task
with no START and no task before it, an id used twice, an END that is neither a date, a duration,
nor `until`, an unreadable date, an unknown `weekday`, `weekend`, or `tickInterval`. Tests:
`tasks`, `excludes`, `errors`.

## MGANTT-4 Axis

The time axis runs from the earliest start to the latest end. Without `tickInterval`, ticks
follow d3's choice for about ten: 1, 5, 15, 30 seconds; 1, 5, 15, 30 minutes; 1, 3, 6, 12 hours;
1, 2 days; 1 week; 1, 3 months; and whole years, picking the interval nearest the span divided by
ten. `tickInterval` (`1day`, `2week`, `1month`, and the `millisecond`, `second`, `minute`, `hour`
units) sets it. Ticks fall on whole units: days at midnight, weeks on `weekday` (default
`sunday`), months on the 1st, an every-N interval where the unit's number divides by N. Each tick
is labeled with `axisFormat`. The today marker is the current date in UTC, `today` in the model.
Tests: `ticks`.

## MGANTT-5 Drawing

A row per task (per non-overlapping run of tasks in a section with `displayMode: compact`), each
section's rows on a band of the next of four colors with its name at the left. Tasks are bars from
start to drawn end, the name inside when it fits and else beside it: plain, `active` (lighter),
`done` (grey), `crit` (red outline; red fill unless also `active` or `done`). A `milestone` is a
diamond at the middle of its span. A `vert` task takes no row: a line across the chart at its
start, with its name at the top. Excluded days are grey columns behind the bars, a vertical grid
line marks each tick with its label below the chart (and above it with `topAxis`), and today is a
red line unless `todayMarker off`, or drawn with the `stroke`, `stroke-width`, and `opacity` of
its style. The title is centered above. Every element is owned by the diagram. Tests: `drawing`.

## MGANTT-6 ASCII

The task names in a column, each section's name above its tasks, a `crit` task marked `!`, then a
chart 60 cells wide: bars of `█` (plain), `▓` (`active`), `░` (`done`) (`#`, `=`, `-` in ASCII),
`◆` (`*`) for a milestone, `·` (`.`) for excluded days, `┆` (`:`) for today, and `│` (`|`) for
a `vert` task with its name above the chart. With `displayMode: compact`, a row holds a section's
non-overlapping tasks with each name written over its bar. The axis below is a `─` line with `┬`
(`+`) at each tick and the labels under it, dropping a label that would overlap the one before;
with `topAxis` it is repeated above. Tests: `snapshots`, `ascii_charset_snapshots`.

## MGANTT-7 Editor

Drawing edits are read-only (LANGS-5). Theme variables (`taskBkgColor`, ...) are ignored.
