//! XY chart to scene (MXY-3): bars and lines over a category or numeric axis, with a value axis,
//! ticks, and a legend for named series.

use super::Theme;
use super::git::branch_color;
use super::paint::{Painter, Px};
use super::shapes::{self, BoxF};
use super::{Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle};
use crate::flowchart::{Label, Span};
use crate::xychart::{SeriesKind, XAxis, XyChart, format_number, ticks};

const MARGIN: f32 = 16.0;
/// Length of the value axis.
const VALUE_LEN: f32 = 340.0;
/// Least length of the category axis, and the least room per position along it.
const CAT_LEN: f32 = 560.0;
const PER_POSITION: f32 = 48.0;
/// Least room per position down a horizontal chart, and the widest a category label wraps at
/// there.
const PER_ROW: f32 = 36.0;
const ROW_LABEL: f32 = 160.0;
/// Length of a tick mark, and the space between it and its label.
const TICK: f32 = 5.0;
const TICK_GAP: f32 = 4.0;
/// A bar's share of its position's room, and its widest.
const BAR_SHARE: f32 = 0.7;
const BAR_MAX: f32 = 80.0;
/// Legend swatch side, and the space between legend entries.
const SWATCH: f32 = 12.0;
const LEGEND_GAP: f32 = 16.0;

/// Series `k`'s color.
pub(super) fn series_color(theme: &Theme, k: usize) -> Color {
    branch_color(theme, k)
}

/// The value ticks: about 5 on a vertical chart's side, about 6 across a horizontal one.
fn value_ticks(chart: &XyChart) -> Vec<f64> {
    let (lo, hi) = chart.y_bounds();
    ticks(lo, hi, if chart.horizontal { 6 } else { 5 })
}

/// Draws `chart` (MXY-3).
pub fn xychart(chart: &XyChart, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let px = Px { metrics, theme };
    let (fs, lh) = (theme.font_size, theme.line_height);
    let small = fs - 2.0;
    let n = chart.positions();
    let (lo, hi) = chart.y_bounds();
    let frac = |v: f64| (((v - lo) / (hi - lo)) as f32).clamp(0.0, 1.0);
    let title = chart
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_size = fs * 1.2;

    // The legend: named series in order.
    let legend: Vec<(usize, &str)> = chart
        .series
        .iter()
        .enumerate()
        .filter_map(|(k, s)| s.name.as_deref().map(|name| (k, name)))
        .collect();
    let legend_w: f32 = legend
        .iter()
        .map(|(_, name)| SWATCH + 6.0 + metrics.width(name, fs))
        .sum::<f32>()
        + LEGEND_GAP * legend.len().saturating_sub(1) as f32;

    // Labels along the category axis, each with its position as a fraction of the axis.
    let cat_len = if chart.horizontal {
        (n as f32 * PER_ROW).max(300.0)
    } else {
        (n as f32 * PER_POSITION).max(CAT_LEN)
    };
    let band = cat_len / n.max(1) as f32;
    // Where position `i` (or numeric-axis value `x`) sits along the category axis.
    let center = |i: usize| (i as f32 + 0.5) * band;
    let x_along = |x: f64, min: f64, max: f64| {
        let f = if max > min {
            ((x - min) / (max - min)) as f32
        } else {
            0.5
        };
        band / 2.0 + f * (cat_len - band)
    };
    let position = |i: usize| match &chart.x_axis {
        XAxis::Range { min, max } => x_along(XyChart::x_at(*min, *max, i, n), *min, *max),
        XAxis::Categories(_) => center(i),
    };
    let wrap_at = if chart.horizontal {
        ROW_LABEL
    } else {
        band - 4.0
    };
    let cat_labels: Vec<(Vec<Vec<Span>>, f32)> = match &chart.x_axis {
        XAxis::Range { min, max } => ticks(*min, *max, if chart.horizontal { 5 } else { 8 })
            .into_iter()
            .map(|t| {
                (
                    Label::plain(format_number(t)).spans(),
                    x_along(t, *min, *max),
                )
            })
            .collect(),
        XAxis::Categories(_) => chart
            .x_labels()
            .iter()
            .enumerate()
            .map(|(i, l)| (px.wrap(&Label::plain(l).spans(), wrap_at), center(i)))
            .collect(),
    };
    let cat_sizes: Vec<(f32, f32)> = cat_labels.iter().map(|(l, _)| px.text_size(l)).collect();
    let cat_w = cat_sizes.iter().map(|s| s.0).fold(0.0f32, f32::max);
    let cat_h = cat_sizes.iter().map(|s| s.1).fold(0.0f32, f32::max);
    let value_labels: Vec<(String, f32)> = value_ticks(chart)
        .into_iter()
        .map(|t| (format_number(t), frac(t)))
        .collect();
    let value_w = value_labels
        .iter()
        .map(|(s, _)| metrics.width(s, fs))
        .fold(0.0f32, f32::max);
    let (side_title, foot_title) = if chart.horizontal {
        (chart.x_title.as_deref(), chart.y_title.as_deref())
    } else {
        (chart.y_title.as_deref(), chart.x_title.as_deref())
    };

    // The plot's box.
    let mut top = MARGIN;
    if title.is_some() {
        top += lh + 16.0;
    }
    let legend_y = top + lh / 2.0;
    if !legend.is_empty() {
        top += lh + 8.0;
    }
    let side_title_y = top + lh / 2.0;
    if side_title.is_some() {
        top += lh + 4.0;
    }
    let (gutter, plot_w, plot_h, foot) = if chart.horizontal {
        (cat_w + TICK + TICK_GAP, VALUE_LEN * 1.5, cat_len, lh)
    } else {
        (value_w + TICK + TICK_GAP, cat_len, VALUE_LEN, cat_h)
    };
    let plot = BoxF {
        x: MARGIN + gutter,
        y: top,
        w: plot_w,
        h: plot_h,
    };
    let foot_top = plot.bottom() + TICK + TICK_GAP;
    let foot_title_y = foot_top + foot + 4.0 + lh / 2.0;
    let bottom = if foot_title.is_some() {
        foot_title_y + lh / 2.0
    } else {
        foot_top + foot
    };
    // Tick labels at the plot's far end may reach past it.
    let overhang = if chart.horizontal {
        value_w / 2.0
    } else {
        cat_w / 2.0
    };
    let content_w = plot.right() + overhang + MARGIN;
    let width = content_w
        .max(title.map_or(0.0, |t| metrics.width(t, title_size)) + 2.0 * MARGIN)
        .max(legend_w + 2.0 * MARGIN);
    let height = bottom + MARGIN;

    // A point from a place along the category axis and a value's fraction of the value axis.
    let at = |c: f32, v: f32| {
        if chart.horizontal {
            Pt::new(plot.x + v * plot.w, plot.y + c)
        } else {
            Pt::new(plot.x + c, plot.bottom() - v * plot.h)
        }
    };

    let owner = Owner::Diagram;
    let mut p = Painter {
        theme,
        metrics,
        elements: Vec::new(),
    };
    let text = |p: &mut Painter, s: &str, center: Pt, color: Color, size: f32| {
        p.elements.push(Element {
            owner,
            primitive: Primitive::Text {
                text: s.to_owned(),
                center,
                size,
                color,
                style: TextStyle::default(),
            },
        });
    };
    if let Some(t) = title {
        text(
            &mut p,
            t,
            Pt::new(width / 2.0, MARGIN + lh / 2.0),
            theme.text,
            title_size,
        );
    }
    let mut x = (width - legend_w) / 2.0;
    for &(k, name) in &legend {
        let b = BoxF {
            x,
            y: legend_y - SWATCH / 2.0,
            w: SWATCH,
            h: SWATCH,
        };
        p.path(
            owner,
            shapes::rounded(b, 2.0),
            Some(series_color(theme, k)),
            None,
        );
        let w = metrics.width(name, fs);
        text(
            &mut p,
            name,
            Pt::new(x + SWATCH + 6.0 + w / 2.0, legend_y),
            theme.text,
            fs,
        );
        x += SWATCH + 6.0 + w + LEGEND_GAP;
    }

    // The series, in order, so later ones draw over earlier ones as in Mermaid.
    let bar_w = (band * BAR_SHARE).min(BAR_MAX);
    for (k, s) in chart.series.iter().enumerate() {
        let color = series_color(theme, k);
        let values = s.values.iter().take(n).enumerate();
        match s.kind {
            SeriesKind::Bar => {
                for (i, &v) in values {
                    let c = position(i);
                    let (a, b) = (at(c - bar_w / 2.0, 0.0), at(c + bar_w / 2.0, frac(v)));
                    let r = BoxF {
                        x: a.x.min(b.x),
                        y: a.y.min(b.y),
                        w: (a.x - b.x).abs(),
                        h: (a.y - b.y).abs(),
                    };
                    p.path(owner, rect(r), Some(color), None);
                }
            }
            SeriesKind::Line => {
                let segments: Vec<Segment> = values
                    .map(|(i, &v)| {
                        let pt = at(position(i), frac(v));
                        if i == 0 {
                            Segment::Move(pt)
                        } else {
                            Segment::Line(pt)
                        }
                    })
                    .collect();
                let stroke = Stroke {
                    width: 2.0,
                    color,
                    dash: None,
                };
                p.path(owner, segments, None, Some(stroke));
            }
        }
    }
    // Point labels, over every series.
    for s in &chart.series {
        for (i, (&v, label)) in s.values.iter().zip(&s.labels).take(n).enumerate() {
            let Some(l) = label else { continue };
            let pt = at(position(i), frac(v));
            let w = metrics.width(l, small);
            let c = if chart.horizontal {
                Pt::new(pt.x + 8.0 + w / 2.0, pt.y)
            } else {
                Pt::new(pt.x, pt.y - 6.0 - lh / 2.0)
            };
            text(&mut p, l, c, theme.text, small);
        }
    }

    // The axes along the plot's left and bottom, with ticks.
    let axis = Stroke {
        width: 2.0,
        color: theme.edge,
        dash: None,
    };
    let mut lines = vec![
        Segment::Move(Pt::new(plot.x, plot.y)),
        Segment::Line(Pt::new(plot.x, plot.bottom())),
        Segment::Line(Pt::new(plot.right(), plot.bottom())),
    ];
    for (_, f) in &value_labels {
        let pt = at(0.0, *f);
        let (pt, end) = if chart.horizontal {
            let pt = Pt::new(pt.x, plot.bottom());
            (pt, Pt::new(pt.x, pt.y + TICK))
        } else {
            (Pt::new(plot.x, pt.y), Pt::new(plot.x - TICK, pt.y))
        };
        lines.push(Segment::Move(pt));
        lines.push(Segment::Line(end));
    }
    for (_, c) in &cat_labels {
        let pt = at(*c, 0.0);
        let end = if chart.horizontal {
            Pt::new(pt.x - TICK, pt.y)
        } else {
            Pt::new(pt.x, pt.y + TICK)
        };
        lines.push(Segment::Move(pt));
        lines.push(Segment::Line(end));
    }
    p.path(owner, lines, None, Some(axis));
    for (s, f) in &value_labels {
        let w = metrics.width(s, fs);
        let c = if chart.horizontal {
            Pt::new(plot.x + f * plot.w, foot_top + lh / 2.0)
        } else {
            Pt::new(
                plot.x - TICK - TICK_GAP - w / 2.0,
                plot.bottom() - f * plot.h,
            )
        };
        text(&mut p, s, c, theme.text, fs);
    }
    for ((lines, c), (w, h)) in cat_labels.iter().zip(&cat_sizes) {
        let center = if chart.horizontal {
            Pt::new(plot.x - TICK - TICK_GAP - w / 2.0, plot.y + c)
        } else {
            Pt::new(plot.x + c, foot_top + h / 2.0)
        };
        p.lines(owner, lines, center, theme.text);
    }
    if let Some(t) = side_title {
        let w = metrics.width(t, fs);
        text(
            &mut p,
            t,
            Pt::new(MARGIN + w / 2.0, side_title_y),
            theme.text,
            fs,
        );
    }
    if let Some(t) = foot_title {
        let c = Pt::new(plot.x + plot.w / 2.0, foot_title_y);
        text(&mut p, t, c, theme.text, fs);
    }
    Scene {
        width,
        height,
        background: theme.background,
        elements: p.elements,
    }
}

fn rect(b: BoxF) -> Vec<Segment> {
    let (r, bottom) = (b.right(), b.bottom());
    shapes::polygon(&[(b.x, b.y), (r, b.y), (r, bottom), (b.x, bottom)])
}
