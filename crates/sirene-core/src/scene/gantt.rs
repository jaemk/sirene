//! Gantt chart to scene (MGANTT-5).

use super::git::{branch_color, ink, luminance};
use super::shapes::{self, BoxF};
use super::style::parse_color;
use super::{
    Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle, Theme,
};
use crate::gantt::time::DAY;
use crate::gantt::{Gantt, Tags, Time, TodayMarker};

const MARGIN: f32 = 16.0;
/// Height of a row, and of a bar in it.
const ROW: f32 = 24.0;
const BAR: f32 = 20.0;
/// Width of the time axis.
const CHART_W: f32 = 720.0;
/// Padding around text.
const PAD: f32 = 8.0;
/// Narrowest the section-name column gets.
const MIN_LEFT: f32 = 75.0;
/// Height of an axis's labels, and of the `vert` labels.
const LABEL_H: f32 = 22.0;

struct Painter<'a, M> {
    metrics: &'a M,
    theme: &'a Theme,
    elements: Vec<Element>,
    right: f32,
}

impl<M: TextMetrics> Painter<'_, M> {
    fn width(&self, text: &str, size: f32) -> f32 {
        self.metrics.width(text, size)
    }

    fn path(&mut self, segments: Vec<Segment>, fill: Option<Color>, stroke: Option<Stroke>) {
        self.elements.push(Element {
            owner: Owner::Diagram,
            primitive: Primitive::Path {
                segments,
                fill,
                stroke,
            },
        });
    }

    fn rect(&mut self, b: BoxF, fill: Color) {
        self.path(shapes::polygon(&corners(b)), Some(fill), None);
    }

    fn line(&mut self, from: Pt, to: Pt, stroke: Stroke) {
        self.path(
            vec![Segment::Move(from), Segment::Line(to)],
            None,
            Some(stroke),
        );
    }

    /// Text starting at `x`, centered on `y`.
    fn text_at(&mut self, text: &str, x: f32, y: f32, size: f32, color: Color) {
        let w = self.width(text, size);
        self.right = self.right.max(x + w);
        self.elements.push(Element {
            owner: Owner::Diagram,
            primitive: Primitive::Text {
                text: text.to_owned(),
                center: Pt::new(x + w / 2.0, y),
                size,
                color,
                style: TextStyle::default(),
            },
        });
    }
}

fn corners(b: BoxF) -> [(f32, f32); 4] {
    [
        (b.x, b.y),
        (b.x + b.w, b.y),
        (b.x + b.w, b.y + b.h),
        (b.x, b.y + b.h),
    ]
}

/// A task bar's fill and outline.
fn task_colors(theme: &Theme, tags: Tags) -> (Color, Color) {
    let dark = luminance(theme.background) < 0.5;
    let (plain, active, done, line, done_line) = if dark {
        (0x5a5fb0, 0x3a3f7a, 0x5a5a5a, 0x9fa4f0, 0x9a9a9a)
    } else {
        (0x8a90dd, 0xbfc7ff, 0xd3d3d3, 0x534fbc, 0x808080)
    };
    let crit = if dark { 0xd04848 } else { 0xff8888 };
    let fill = if tags.done {
        done
    } else if tags.active {
        active
    } else if tags.crit {
        crit
    } else {
        plain
    };
    let stroke = if tags.crit {
        crit
    } else if tags.done {
        done_line
    } else {
        line
    };
    (Color::rgb(fill), Color::rgb(stroke))
}

/// The today marker's stroke, or `None` when it is off.
fn today_stroke(marker: &TodayMarker) -> Option<Stroke> {
    let mut stroke = Stroke {
        width: 2.0,
        color: Color::rgb(0xff0000),
        dash: None,
    };
    match marker {
        TodayMarker::Off => return None,
        TodayMarker::Default => {}
        TodayMarker::Style(css) => {
            for decl in css.split([',', ';']) {
                let Some((k, v)) = decl.split_once(':') else {
                    continue;
                };
                let v = v.trim();
                match k.trim() {
                    "stroke" => {
                        if let Some(c) = parse_color(v) {
                            stroke.color = Color {
                                a: stroke.color.a,
                                ..c
                            };
                        }
                    }
                    "stroke-width" => {
                        if let Ok(w) = v.trim_end_matches("px").parse::<f32>() {
                            stroke.width = w;
                        }
                    }
                    "opacity" => {
                        if let Ok(o) = v.parse::<f32>() {
                            stroke.color.a = (o.clamp(0.0, 1.0) * 255.0).round() as u8;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    Some(stroke)
}

/// Draws `gantt` (MGANTT-5).
pub fn gantt(gantt: &Gantt, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let mut p = Painter {
        metrics,
        theme,
        elements: Vec::new(),
        right: 0.0,
    };
    let fs = theme.font_size;
    let small = fs * 0.85;
    let lh = theme.line_height;
    let title = gantt
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_h = if title.is_some() { lh + 16.0 } else { 0.0 };
    let rows = gantt.rows();
    let left_w = gantt
        .sections
        .iter()
        .filter_map(|s| s.name.as_deref())
        .map(|n| p.width(n, fs) + 2.0 * PAD)
        .fold(MIN_LEFT, f32::max);
    let x0 = MARGIN + left_w;
    let x1 = x0 + CHART_W;
    let verts: Vec<(&str, Time)> = gantt
        .tasks()
        .filter(|t| t.tags.vert)
        .map(|t| (t.name.as_str(), t.start))
        .collect();
    let mut y = MARGIN + title_h;
    let top_axis_y = y;
    if gantt.top_axis {
        y += LABEL_H;
    }
    let vert_y = y;
    if !verts.is_empty() {
        y += LABEL_H;
    }
    let rows_top = y;
    let rows_bottom = rows_top + rows.len() as f32 * ROW;
    let bottom_axis_y = rows_bottom;
    let height = bottom_axis_y + LABEL_H + MARGIN;
    p.right = x1;
    let Some((t0, t1)) = gantt.range() else {
        return finish(p, title, height.max(MARGIN * 2.0 + title_h));
    };
    let span = (t1 - t0) as f64;
    let x = |t: Time| x0 + ((t - t0) as f64 / span * f64::from(CHART_W)) as f32;

    // Section bands, and their names.
    let mut r = 0;
    while r < rows.len() {
        let s = rows[r].section;
        let n = rows[r..].iter().take_while(|row| row.section == s).count();
        let band = BoxF {
            x: MARGIN,
            y: rows_top + r as f32 * ROW,
            w: x1 - MARGIN,
            h: n as f32 * ROW,
        };
        let fill = branch_color(theme, s % 4)
            .with_alpha(0x24)
            .over(theme.background);
        p.rect(band, fill);
        if let Some(name) = gantt.sections[s].name.as_deref() {
            p.text_at(name, MARGIN + PAD, band.y + band.h / 2.0, fs, theme.text);
        }
        r += n;
    }
    // Excluded days.
    let grey = if luminance(theme.background) < 0.5 {
        Color::rgb(0x333333)
    } else {
        Color::rgb(0xe6e6e6)
    };
    for d in gantt.excluded_days() {
        let (a, b) = (x(d).max(x0), x(d + DAY).min(x1));
        if b > a {
            let h = rows_bottom - rows_top;
            p.rect(
                BoxF {
                    x: a,
                    y: rows_top,
                    w: b - a,
                    h,
                },
                grey,
            );
        }
    }
    // Grid lines and tick labels.
    let grid = Stroke {
        width: 1.0,
        color: theme.edge.with_alpha(0x40).over(theme.background),
        dash: None,
    };
    let mut last_label = f32::NEG_INFINITY;
    for t in gantt.ticks() {
        let tx = x(t);
        p.line(Pt::new(tx, rows_top), Pt::new(tx, rows_bottom), grid);
        let label = gantt.label(t);
        let w = p.width(&label, small);
        let lx = tx - w / 2.0;
        if lx < last_label + 4.0 {
            continue;
        }
        last_label = lx + w;
        p.text_at(&label, lx, bottom_axis_y + LABEL_H / 2.0, small, theme.text);
        if gantt.top_axis {
            p.text_at(&label, lx, top_axis_y + LABEL_H / 2.0, small, theme.text);
        }
    }
    // Bars.
    for (r, row) in rows.iter().enumerate() {
        let section = &gantt.sections[row.section];
        let cy = rows_top + (r as f32 + 0.5) * ROW;
        for &i in &row.tasks {
            let task = &section.tasks[i];
            let (fill, line) = task_colors(theme, task.tags);
            let stroke = Stroke {
                width: 1.5,
                color: line,
                dash: None,
            };
            let tw = p.width(&task.name, small);
            if task.tags.milestone {
                let mx = x(task.start + (task.drawn_end - task.start) / 2);
                let h = BAR / 2.0;
                let diamond = [(mx, cy - h), (mx + h, cy), (mx, cy + h), (mx - h, cy)];
                p.path(shapes::polygon(&diamond), Some(fill), Some(stroke));
                p.text_at(&task.name, mx + h + PAD / 2.0, cy, small, theme.text);
                continue;
            }
            let (a, b) = (x(task.start), x(task.drawn_end).max(x(task.start) + 1.0));
            let bar = BoxF {
                x: a,
                y: cy - BAR / 2.0,
                w: b - a,
                h: BAR,
            };
            p.path(shapes::rounded(bar, 3.0), Some(fill), Some(stroke));
            if tw + PAD <= bar.w {
                p.text_at(&task.name, a + (bar.w - tw) / 2.0, cy, small, ink(fill));
            } else if b + PAD / 2.0 + tw <= x1 || a - PAD / 2.0 - tw < x0 {
                p.text_at(&task.name, b + PAD / 2.0, cy, small, theme.text);
            } else {
                p.text_at(&task.name, a - PAD / 2.0 - tw, cy, small, theme.text);
            }
        }
    }
    // `vert` tasks: a line across the rows with the name above.
    let vert_stroke = Stroke {
        width: 2.0,
        color: theme.edge,
        dash: Some((6.0, 3.0)),
    };
    for (name, t) in verts {
        let vx = x(t);
        p.line(
            Pt::new(vx, vert_y + LABEL_H),
            Pt::new(vx, rows_bottom),
            vert_stroke,
        );
        let w = p.width(name, small);
        p.text_at(
            name,
            vx - w / 2.0,
            vert_y + LABEL_H / 2.0,
            small,
            theme.text,
        );
    }
    // Today.
    if let Some(stroke) = today_stroke(&gantt.today_marker)
        && (t0..=t1).contains(&gantt.now)
    {
        let tx = x(gantt.now);
        p.line(Pt::new(tx, rows_top), Pt::new(tx, rows_bottom), stroke);
    }
    finish(p, title, height)
}

/// The scene, as wide as what was drawn, with the title centered above.
fn finish<M: TextMetrics>(p: Painter<'_, M>, title: Option<&str>, height: f32) -> Scene {
    let size = p.theme.font_size * 1.2;
    let width = (p.right + MARGIN).max(title.map_or(0.0, |t| p.width(t, size)) + 2.0 * MARGIN);
    let mut elements = p.elements;
    if let Some(t) = title {
        elements.insert(
            0,
            Element {
                owner: Owner::Diagram,
                primitive: Primitive::Text {
                    text: t.to_owned(),
                    center: Pt::new(width / 2.0, MARGIN + p.theme.line_height / 2.0),
                    size,
                    color: p.theme.text,
                    style: TextStyle::default(),
                },
            },
        );
    }
    Scene {
        width,
        height,
        background: p.theme.background,
        elements,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn today_styles() {
        assert_eq!(today_stroke(&TodayMarker::Off), None);
        let s = today_stroke(&TodayMarker::Style(
            "stroke-width:5px,stroke:#00f,opacity:0.5".to_owned(),
        ))
        .unwrap();
        assert_eq!(s.width, 5.0);
        assert_eq!(s.color, Color::rgb(0x0000ff).with_alpha(128));
        let d = today_stroke(&TodayMarker::Default).unwrap();
        assert_eq!(d.color, Color::rgb(0xff0000));
    }

    #[test]
    fn colors() {
        let t = &Theme::LIGHT;
        let crit = Tags {
            crit: true,
            ..Tags::default()
        };
        assert_eq!(task_colors(t, crit).0, Color::rgb(0xff8888));
        let crit_done = Tags { done: true, ..crit };
        assert_eq!(
            task_colors(t, crit_done),
            (Color::rgb(0xd3d3d3), Color::rgb(0xff8888))
        );
        let active = Tags {
            active: true,
            ..Tags::default()
        };
        assert_eq!(task_colors(t, active).0, Color::rgb(0xbfc7ff));
        assert_ne!(
            task_colors(&Theme::DARK, Tags::default()),
            task_colors(t, Tags::default())
        );
    }
}
