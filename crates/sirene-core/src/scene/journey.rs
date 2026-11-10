//! User journey to scene (MJOUR-3): tasks in a row under their sections, each with a face for
//! its score.

use super::Theme;
use super::git::{branch_color, ink};
use super::paint::{Painter, Px};
use super::shapes::{self, BoxF};
use super::{Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle};
use crate::flowchart::{Label, Span};
use crate::journey::{Journey, MAX_SCORE};

const MARGIN: f32 = 16.0;
/// Padding inside a box.
const PAD: f32 = 8.0;
/// Width of a task box.
const TASK_W: f32 = 150.0;
/// Least height of a section or task box.
const MIN_H: f32 = 50.0;
/// Space between boxes.
const GAP: f32 = 10.0;
/// Vertical distance between faces one score apart.
const STEP: f32 = 30.0;
/// Radius of a face.
const FACE: f32 = 15.0;
/// Radius of an actor's dot.
const DOT: f32 = 7.0;
/// Length of the arrowhead.
const HEAD: f32 = 10.0;

const FACE_FILL: Color = Color::rgb(0xfff8dc);
const FACE_STROKE: Color = Color::rgb(0x999999);
const FACE_INK: Color = Color::rgb(0x666666);

/// Actor `k`'s color.
pub(super) fn actor_color(theme: &Theme, k: usize) -> Color {
    branch_color(theme, k + 4)
}

fn stroke(color: Color, width: f32, dash: Option<(f32, f32)>) -> Option<Stroke> {
    Some(Stroke { width, color, dash })
}

/// Draws `journey` (MJOUR-3).
pub fn journey(journey: &Journey, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let px = Px { metrics, theme };
    let lh = theme.line_height;
    let wrap = |text: &str| px.wrap(&Label::plain(text).spans(), TASK_W - 2.0 * PAD);
    let block_h = |lines: &[Vec<Span>]| (lines.len() as f32 * lh + 2.0 * PAD).max(MIN_H);
    let actors = journey.actors();
    let title = journey
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_h = if title.is_some() { lh + 16.0 } else { 0.0 };
    let top = MARGIN + title_h;

    // The legend: a dot and a name per actor, left of the tasks.
    let legend_w = actors
        .iter()
        .map(|a| metrics.width(a, theme.font_size))
        .fold(0.0f32, f32::max);
    let tasks_x = if actors.is_empty() {
        MARGIN
    } else {
        MARGIN + 2.0 * DOT + 8.0 + legend_w + 2.0 * GAP
    };

    let tasks: Vec<(usize, Vec<Vec<Span>>, &crate::journey::Task)> = journey
        .tasks()
        .map(|(s, t)| (s, wrap(&t.name), t))
        .collect();
    let sections: Vec<Option<Vec<Vec<Span>>>> = journey
        .sections
        .iter()
        .map(|s| s.name.as_deref().filter(|_| !s.tasks.is_empty()).map(wrap))
        .collect();
    let section_h = sections
        .iter()
        .flatten()
        .map(|l| block_h(l))
        .fold(0.0f32, f32::max);
    let task_h = tasks
        .iter()
        .map(|(_, l, _)| block_h(l))
        .fold(MIN_H, f32::max);
    let task_y = if section_h > 0.0 {
        top + section_h + GAP
    } else {
        top
    };
    let faces_top = task_y + task_h + STEP;
    let faces_bottom = faces_top + (MAX_SCORE - 1.0) as f32 * STEP;
    let arrow_y = faces_bottom + FACE + 2.0 * GAP;
    let task_x = |i: usize| tasks_x + i as f32 * (TASK_W + GAP);
    let right = if tasks.is_empty() {
        tasks_x
    } else {
        task_x(tasks.len() - 1) + TASK_W
    };
    let legend_bottom = top + actors.len() as f32 * (lh + 6.0);
    let width = (right + HEAD + MARGIN)
        .max(title.map_or(0.0, |t| metrics.width(t, theme.font_size * 1.2)) + 2.0 * MARGIN);
    let height = if tasks.is_empty() {
        legend_bottom.max(top) + MARGIN
    } else {
        (arrow_y + HEAD).max(legend_bottom) + MARGIN
    };

    let owner = Owner::Diagram;
    let mut p = Painter {
        theme,
        metrics,
        elements: Vec::new(),
    };
    let text = |p: &mut Painter, s: &str, at: Pt, color: Color, size: f32| {
        p.elements.push(Element {
            owner,
            primitive: Primitive::Text {
                text: s.to_owned(),
                center: at,
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
            theme.font_size * 1.2,
        );
    }
    for (k, a) in actors.iter().enumerate() {
        let cy = top + k as f32 * (lh + 6.0) + lh / 2.0;
        let c = Pt::new(MARGIN + DOT, cy);
        p.path(
            owner,
            shapes::ellipse(c, DOT, DOT),
            Some(actor_color(theme, k)),
            None,
        );
        let w = metrics.width(a, theme.font_size);
        text(
            &mut p,
            a,
            Pt::new(MARGIN + 2.0 * DOT + 8.0 + w / 2.0, cy),
            theme.text,
            theme.font_size,
        );
    }
    // Sections, spanning their tasks.
    for (s, lines) in sections.iter().enumerate() {
        let Some(lines) = lines else { continue };
        let idx: Vec<usize> = (0..tasks.len()).filter(|&i| tasks[i].0 == s).collect();
        let (Some(&a), Some(&b)) = (idx.first(), idx.last()) else {
            continue;
        };
        let bx = BoxF {
            x: task_x(a),
            y: top,
            w: task_x(b) + TASK_W - task_x(a),
            h: section_h,
        };
        let fill = branch_color(theme, s);
        p.path(owner, shapes::rounded(bx, 4.0), Some(fill), None);
        p.lines(owner, lines, bx.center(), ink(fill));
    }
    // Each task's line down to its face, behind the box and the face.
    for (i, (_, _, t)) in tasks.iter().enumerate() {
        let cx = task_x(i) + TASK_W / 2.0;
        let cy = faces_top + (MAX_SCORE - t.score) as f32 * STEP;
        p.path(
            owner,
            vec![
                Segment::Move(Pt::new(cx, task_y + task_h)),
                Segment::Line(Pt::new(cx, cy)),
            ],
            None,
            stroke(FACE_STROKE, 1.0, Some((4.0, 4.0))),
        );
    }
    for (i, (s, lines, t)) in tasks.iter().enumerate() {
        let x = task_x(i);
        let color = branch_color(theme, *s);
        let fill = color.with_alpha(0x50).over(theme.background);
        let bx = BoxF {
            x,
            y: task_y,
            w: TASK_W,
            h: task_h,
        };
        p.path(
            owner,
            shapes::rounded(bx, 4.0),
            Some(fill),
            stroke(color, 1.0, None),
        );
        p.lines(owner, lines, bx.center(), ink(fill));
        // The task's actors as dots along its top.
        for (k, a) in t.actors.iter().enumerate() {
            let n = actors.iter().position(|b| b == a).expect("a listed actor");
            let c = Pt::new(x + 10.0 + k as f32 * (DOT + 3.0), task_y);
            p.path(
                owner,
                shapes::ellipse(c, DOT / 2.0 + 1.0, DOT / 2.0 + 1.0),
                Some(actor_color(theme, n)),
                stroke(theme.background, 1.0, None),
            );
        }
        let c = Pt::new(
            x + TASK_W / 2.0,
            faces_top + (MAX_SCORE - t.score) as f32 * STEP,
        );
        face(&mut p, c, t.score);
    }
    if !tasks.is_empty() {
        let (from, to) = (Pt::new(tasks_x, arrow_y), Pt::new(right + HEAD, arrow_y));
        p.path(
            owner,
            vec![
                Segment::Move(from),
                Segment::Line(Pt::new(to.x - HEAD, to.y)),
            ],
            None,
            stroke(theme.edge, 2.0, None),
        );
        let half = HEAD * 0.6;
        p.path(
            owner,
            shapes::polygon(&[
                (to.x - HEAD, to.y - half),
                (to.x, to.y),
                (to.x - HEAD, to.y + half),
            ]),
            Some(theme.edge),
            None,
        );
    }
    Scene {
        width,
        height,
        background: theme.background,
        elements: p.elements,
    }
}

/// A face at `c`: smiling above a score of 3, frowning below it, and level at 3.
fn face(p: &mut Painter, c: Pt, score: f64) {
    let owner = Owner::Diagram;
    p.path(
        owner,
        shapes::ellipse(c, FACE, FACE),
        Some(FACE_FILL),
        stroke(FACE_STROKE, 1.0, None),
    );
    for dx in [-5.0, 5.0] {
        p.path(
            owner,
            shapes::ellipse(Pt::new(c.x + dx, c.y - 4.0), 1.5, 1.5),
            Some(FACE_INK),
            None,
        );
    }
    let (l, r) = (c.x - 6.0, c.x + 6.0);
    let mouth = if score > 3.0 {
        let y = c.y + 4.0;
        vec![
            Segment::Move(Pt::new(l, y)),
            Segment::Arc {
                radii: Pt::new(7.0, 7.0),
                large: false,
                sweep: false,
                to: Pt::new(r, y),
            },
        ]
    } else if score < 3.0 {
        let y = c.y + 9.0;
        vec![
            Segment::Move(Pt::new(l, y)),
            Segment::Arc {
                radii: Pt::new(7.0, 7.0),
                large: false,
                sweep: true,
                to: Pt::new(r, y),
            },
        ]
    } else {
        let y = c.y + 6.0;
        vec![Segment::Move(Pt::new(l, y)), Segment::Line(Pt::new(r, y))]
    };
    p.path(owner, mouth, None, stroke(FACE_INK, 1.5, None));
}
