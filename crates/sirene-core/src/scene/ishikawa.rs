//! Ishikawa diagram to scene (MISH-3), following Mermaid's fishbone layout: the effect in a head
//! at the right end of a spine, causes alternating above and below on bones angled back from it,
//! and their sub-causes on bones alternating flat and angled.

use super::{Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle, Theme};
use crate::ishikawa::{Cause, Ishikawa};

/// The spine's least half-height budget, and the spine's height above with few causes.
const SPINE_BASE: f32 = 250.0;
/// The length of a flat bone with no sub-causes.
const BONE_STUB: f32 = 30.0;
/// The length of a flat bone with sub-causes, plus `BONE_PER_CHILD` for each.
const BONE_BASE: f32 = 60.0;
const BONE_PER_CHILD: f32 = 5.0;
/// A cause bone's angle from the spine, in degrees.
const ANGLE: f32 = 82.0;
const PADDING: f32 = 20.0;
/// Where a label wraps, in characters.
const WRAP: usize = 15;

#[derive(Clone, Copy)]
enum Anchor {
    Middle,
    End,
}

/// Where a label's lines sit against its `y`.
#[derive(Clone, Copy)]
enum Baseline {
    Middle,
    /// The last line's baseline at `y`: the label above it.
    Above,
    /// The first line's top at `y`: the label below it.
    Below,
}

/// Lines of text placed for drawing, and their box.
struct Placed {
    lines: Vec<(String, Pt)>,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

/// One drawn bone, for its sub-causes to start from.
#[derive(Clone, Copy)]
struct Bone {
    from: Pt,
    to: Pt,
    children: usize,
    drawn: usize,
}

/// A sub-cause in drawing order.
struct Entry {
    lines: Vec<String>,
    depth: usize,
    parent: Option<usize>,
    children: usize,
}

struct Draw<'a, M> {
    metrics: &'a M,
    theme: &'a Theme,
    elements: Vec<Element>,
    min: Pt,
    max: Pt,
}

/// `lines` wrapped at word boundaries to `max` characters, as Mermaid does.
fn wrap(lines: &[String], max: usize) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        if line.chars().count() <= max {
            out.push(line.clone());
            continue;
        }
        let mut wrapped: Vec<String> = Vec::new();
        for word in line.split_whitespace() {
            match wrapped.last_mut() {
                Some(last) if last.chars().count() + 1 + word.chars().count() <= max => {
                    last.push(' ');
                    last.push_str(word);
                }
                _ => wrapped.push(word.to_owned()),
            }
        }
        out.extend(wrapped);
    }
    out
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

impl<M: TextMetrics> Draw<'_, M> {
    fn grow(&mut self, p: Pt) {
        self.min = Pt::new(self.min.x.min(p.x), self.min.y.min(p.y));
        self.max = Pt::new(self.max.x.max(p.x), self.max.y.max(p.y));
    }

    fn path(&mut self, segments: Vec<Segment>, fill: Option<super::Color>, width: Option<f32>) {
        for s in &segments {
            match *s {
                Segment::Move(p) | Segment::Line(p) => self.grow(p),
                Segment::Cubic { to, .. } | Segment::Arc { to, .. } => self.grow(to),
                Segment::Close => {}
            }
        }
        let stroke = width.map(|width| Stroke {
            width,
            color: self.theme.edge,
            dash: None,
        });
        self.elements.push(Element {
            owner: Owner::Diagram,
            primitive: Primitive::Path {
                segments,
                fill,
                stroke,
            },
        });
    }

    /// A bone from `from` to `to`, `width` wide, with an arrowhead at `from` pointing away
    /// from `to`.
    fn bone(&mut self, from: Pt, to: Pt, width: f32) {
        self.path(
            vec![Segment::Move(from), Segment::Line(to)],
            None,
            Some(width),
        );
        let (dx, dy) = (to.x - from.x, to.y - from.y);
        let len = (dx * dx + dy * dy).sqrt();
        if len == 0.0 {
            return;
        }
        let (ux, uy) = (dx / len, dy / len);
        let (l, w) = (6.0 * width, 3.0 * width);
        let base = Pt::new(from.x + ux * l, from.y + uy * l);
        self.path(
            vec![
                Segment::Move(from),
                Segment::Line(Pt::new(base.x - uy * w, base.y + ux * w)),
                Segment::Line(Pt::new(base.x + uy * w, base.y - ux * w)),
                Segment::Close,
            ],
            Some(self.theme.edge),
            None,
        );
    }

    /// Places `lines` at `at`, anchored and aligned as Mermaid's labels are.
    fn place(&self, lines: &[String], at: Pt, anchor: Anchor, baseline: Baseline) -> Placed {
        let fs = self.theme.font_size;
        let lh = fs * 1.05;
        let n = lines.len().max(1) as f32;
        let first = at.y - (n - 1.0) * lh / 2.0;
        let shift = match baseline {
            Baseline::Middle => 0.0,
            Baseline::Above => -0.3 * fs,
            Baseline::Below => 0.5 * fs,
        };
        let mut placed = Placed {
            lines: Vec::new(),
            left: f32::INFINITY,
            top: first + shift - 0.6 * fs,
            right: f32::NEG_INFINITY,
            bottom: first + (n - 1.0) * lh + shift + 0.6 * fs,
        };
        for (i, line) in lines.iter().enumerate() {
            let w = self.metrics.width(line, fs);
            let cx = match anchor {
                Anchor::Middle => at.x,
                Anchor::End => at.x - w / 2.0,
            };
            placed.left = placed.left.min(cx - w / 2.0);
            placed.right = placed.right.max(cx + w / 2.0);
            let center = Pt::new(cx, first + i as f32 * lh + shift);
            placed.lines.push((line.clone(), center));
        }
        if placed.lines.is_empty() {
            (placed.left, placed.right) = (at.x, at.x);
        }
        placed
    }

    fn text(&mut self, placed: Placed, bold: bool) -> f32 {
        self.grow(Pt::new(placed.left, placed.top));
        self.grow(Pt::new(placed.right, placed.bottom));
        for (text, center) in placed.lines {
            self.elements.push(Element {
                owner: Owner::Diagram,
                primitive: Primitive::Text {
                    text,
                    center,
                    size: self.theme.font_size,
                    color: self.theme.text,
                    style: TextStyle {
                        bold,
                        italic: false,
                    },
                },
            });
        }
        placed.left
    }

    /// The effect's head at `at`, on the spine's right end.
    fn head(&mut self, effect: &Cause, at: Pt) {
        let fs = self.theme.font_size;
        let max = ((110.0 / (fs * 0.6)).floor() as usize).max(6);
        let lines = wrap(&effect.lines, max);
        let probe = self.place(&lines, Pt::new(0.0, 0.0), Anchor::Middle, Baseline::Middle);
        let (tw, th) = (probe.right - probe.left, probe.bottom - probe.top);
        let w = (tw + 6.0).max(60.0);
        let h = (th * 2.0 + 40.0).max(40.0);
        let top = Pt::new(at.x, at.y - h / 2.0);
        let bottom = Pt::new(at.x, at.y + h / 2.0);
        // Mermaid's quadratic `Q 2.4w 0`, as a cubic.
        let q = Pt::new(at.x + 2.4 * w, at.y);
        let c = |from: Pt| {
            Pt::new(
                from.x + (q.x - from.x) * 2.0 / 3.0,
                from.y + (q.y - from.y) * 2.0 / 3.0,
            )
        };
        self.path(
            vec![
                Segment::Move(top),
                Segment::Line(bottom),
                Segment::Cubic {
                    c1: c(bottom),
                    c2: c(top),
                    to: top,
                },
                Segment::Close,
            ],
            Some(self.theme.node_fill),
            Some(2.0),
        );
        self.grow(Pt::new(at.x + 1.2 * w, at.y));
        let placed = self.place(
            &lines,
            Pt::new(at.x + w / 2.0 + 3.0, at.y),
            Anchor::Middle,
            Baseline::Middle,
        );
        self.text(placed, true);
    }

    /// A cause's bone from `start` on the spine, up (`dir` -1) or down (1), and its sub-causes.
    /// Returns the left edge of its labels and the cause's box, where the next pair starts.
    fn branch(&mut self, cause: &Cause, mut start: Pt, dir: f32, len: f32) -> f32 {
        let (sin, cos) = ANGLE.to_radians().sin_cos();
        if cause.children.is_empty() {
            // A short bone's label sits beside the spine: start it a label's width further
            // left so the label clears the head and the bones to its right.
            let probe = self.place(
                &cause.lines,
                Pt::new(0.0, 0.0),
                Anchor::Middle,
                Baseline::Middle,
            );
            start.x -= (probe.right - probe.left) / 2.0 + 24.0;
        }
        let line_len = if cause.children.is_empty() {
            len * 0.2
        } else {
            len
        };
        let (dx, dy) = (-cos * line_len, sin * line_len * dir);
        let end = Pt::new(start.x + dx, start.y + dy);
        self.bone(start, end, 2.0);
        let placed = self.place(
            &cause.lines,
            Pt::new(end.x, end.y + 11.0 * dir),
            Anchor::Middle,
            Baseline::Middle,
        );
        let (x0, y0) = (placed.left - 20.0, placed.top - 2.0);
        let (x1, y1) = (placed.right + 20.0, placed.bottom + 2.0);
        self.path(
            vec![
                Segment::Move(Pt::new(x0, y0)),
                Segment::Line(Pt::new(x1, y0)),
                Segment::Line(Pt::new(x1, y1)),
                Segment::Line(Pt::new(x0, y1)),
                Segment::Close,
            ],
            Some(self.theme.node_fill),
            Some(2.0),
        );
        let mut left = self.text(placed, false).min(x0);
        if cause.children.is_empty() {
            return left;
        }
        let mut entries = Vec::new();
        let mut order = Vec::new();
        flatten(&cause.children, None, 2, dir, &mut entries, &mut order);
        let mut ys = vec![0.0; entries.len()];
        for (slot, &i) in order.iter().enumerate() {
            ys[i] = start.y + dy * ((slot + 1) as f32 / (entries.len() + 1) as f32);
        }
        let mut root = Bone {
            from: start,
            to: end,
            children: cause.children.len(),
            drawn: 0,
        };
        let mut bones: Vec<Option<Bone>> = vec![None; entries.len()];
        let (diag_x, diag_y) = (-cos, sin * dir);
        for (i, e) in entries.iter().enumerate() {
            let y = ys[i];
            let par = match e.parent {
                Some(p) => bones[p].as_mut().expect("a parent bone is drawn first"),
                None => &mut root,
            };
            let (from, to, placed) = if e.depth.is_multiple_of(2) {
                // Flat: from the parent's bone at this height, out to the left.
                let dy_p = par.to.y - par.from.y;
                let t = if dy_p != 0.0 {
                    (y - par.from.y) / dy_p
                } else {
                    0.5
                };
                let x0 = lerp(par.from.x, par.to.x, t);
                let x1 = x0
                    - if e.children > 0 {
                        BONE_BASE + e.children as f32 * BONE_PER_CHILD
                    } else {
                        BONE_STUB
                    };
                let placed = self.place(&e.lines, Pt::new(x1, y), Anchor::End, Baseline::Middle);
                (Pt::new(x0, y), Pt::new(x1, y), placed)
            } else {
                // Angled: from evenly spaced points on the parent's flat bone.
                let k = par.drawn;
                par.drawn += 1;
                let t = (par.children - k) as f32 / (par.children + 1) as f32;
                let x0 = lerp(par.from.x, par.to.x, t);
                let y0 = par.from.y;
                let x1 = x0 + diag_x * ((y - y0) / diag_y);
                let baseline = if dir < 0.0 {
                    Baseline::Above
                } else {
                    Baseline::Below
                };
                let placed = self.place(&e.lines, Pt::new(x1, y), Anchor::End, baseline);
                (Pt::new(x0, y0), Pt::new(x1, y), placed)
            };
            self.bone(from, to, 1.0);
            left = left.min(self.text(placed, false));
            if e.children > 0 {
                bones[i] = Some(Bone {
                    from,
                    to,
                    children: e.children,
                    drawn: 0,
                });
            }
        }
        left
    }
}

/// `nodes` and their descendants in drawing order, from `depth`: flat (even) depths before
/// their children, angled (odd) ones after, so each angled bone stays inside its parent's wedge.
/// `order` gets each entry's place along the cause's bone.
fn flatten(
    nodes: &[Cause],
    parent: Option<usize>,
    depth: usize,
    dir: f32,
    entries: &mut Vec<Entry>,
    order: &mut Vec<usize>,
) {
    let ordered: Vec<&Cause> = if dir < 0.0 {
        nodes.iter().rev().collect()
    } else {
        nodes.iter().collect()
    };
    for child in ordered {
        let i = entries.len();
        entries.push(Entry {
            lines: wrap(&child.lines, WRAP),
            depth,
            parent,
            children: child.children.len(),
        });
        if depth.is_multiple_of(2) {
            order.push(i);
            flatten(&child.children, Some(i), depth + 1, dir, entries, order);
        } else {
            flatten(&child.children, Some(i), depth + 1, dir, entries, order);
            order.push(i);
        }
    }
}

/// The total and the largest count of descendants of `causes`.
fn stats(causes: &[&Cause]) -> (f32, f32) {
    causes.iter().fold((0.0, 0.0), |(total, max), c| {
        let d = c.descendants() as f32;
        (total + d, f32::max(max, d))
    })
}

fn shift(p: Pt, d: Pt) -> Pt {
    Pt::new(p.x + d.x, p.y + d.y)
}

/// Draws `chart` (MISH-3).
pub fn ishikawa(chart: &Ishikawa, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let mut d = Draw {
        metrics,
        theme,
        elements: Vec::new(),
        min: Pt::new(f32::INFINITY, f32::INFINITY),
        max: Pt::new(f32::NEG_INFINITY, f32::NEG_INFINITY),
    };
    if let Some(effect) = &chart.effect {
        let causes = &effect.children;
        let mut spine_y = SPINE_BASE;
        let (mut upper_len, mut lower_len) = (SPINE_BASE, SPINE_BASE);
        if !causes.is_empty() {
            let upper: Vec<&Cause> = causes.iter().step_by(2).collect();
            let lower: Vec<&Cause> = causes.iter().skip(1).step_by(2).collect();
            let (up_total, up_max) = stats(&upper);
            let (low_total, low_max) = stats(&lower);
            let total = up_total + low_total;
            if total > 0.0 {
                let pool = SPINE_BASE * 2.0;
                let min = SPINE_BASE * 0.3;
                upper_len = (pool * up_total / total).max(min);
                lower_len = (pool * low_total / total).max(min);
            }
            let spacing = theme.font_size * 2.0;
            upper_len = upper_len.max(up_max * spacing);
            lower_len = lower_len.max(low_max * spacing);
            spine_y = upper_len.max(SPINE_BASE);
        }
        d.head(effect, Pt::new(0.0, spine_y));
        if !causes.is_empty() {
            let mut spine_x = -20.0;
            for pair in causes.chunks(2) {
                let mut left = f32::INFINITY;
                left = left.min(d.branch(&pair[0], Pt::new(spine_x, spine_y), -1.0, upper_len));
                if let Some(c) = pair.get(1) {
                    left = left.min(d.branch(c, Pt::new(spine_x, spine_y), 1.0, lower_len));
                }
                spine_x = left;
            }
            let at = d.elements.len();
            d.path(
                vec![
                    Segment::Move(Pt::new(spine_x, spine_y)),
                    Segment::Line(Pt::new(0.0, spine_y)),
                ],
                None,
                Some(2.0),
            );
            let spine = d.elements.remove(at);
            d.elements.insert(0, spine);
        }
    }
    let title = chart
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_size = theme.font_size * 1.2;
    let title_h = if title.is_some() {
        theme.line_height + 16.0
    } else {
        0.0
    };
    if d.elements.is_empty() {
        d.min = Pt::new(0.0, 0.0);
        d.max = Pt::new(0.0, 0.0);
    }
    let off = Pt::new(PADDING - d.min.x, PADDING + title_h - d.min.y);
    let mut width = d.max.x - d.min.x + 2.0 * PADDING;
    if let Some(t) = title {
        width = width.max(metrics.width(t, title_size) + 2.0 * PADDING);
    }
    let mut elements = Vec::with_capacity(d.elements.len() + 1);
    if let Some(t) = title {
        elements.push(Element {
            owner: Owner::Diagram,
            primitive: Primitive::Text {
                text: t.to_owned(),
                center: Pt::new(width / 2.0, PADDING + theme.line_height / 2.0),
                size: title_size,
                color: theme.text,
                style: TextStyle::default(),
            },
        });
    }
    for mut e in d.elements {
        match &mut e.primitive {
            Primitive::Path { segments, .. } => {
                for s in segments.iter_mut() {
                    match s {
                        Segment::Move(p) | Segment::Line(p) => *p = shift(*p, off),
                        Segment::Arc { to, .. } => *to = shift(*to, off),
                        Segment::Cubic { c1, c2, to } => {
                            *c1 = shift(*c1, off);
                            *c2 = shift(*c2, off);
                            *to = shift(*to, off);
                        }
                        Segment::Close => {}
                    }
                }
            }
            Primitive::Text { center, .. } => *center = shift(*center, off),
        }
        elements.push(e);
    }
    Scene {
        width,
        height: d.max.y - d.min.y + 2.0 * PADDING + title_h,
        background: theme.background,
        elements,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn wraps() {
        assert_eq!(wrap(&s(&["short"]), 15), s(&["short"]));
        assert_eq!(
            wrap(&s(&["Protective film not removed"]), 15),
            s(&["Protective film", "not removed"])
        );
        assert_eq!(
            wrap(&s(&["a", "Shutter speed too slow"]), 15),
            s(&["a", "Shutter speed", "too slow"])
        );
        assert_eq!(
            wrap(&s(&["Extraordinarily long"]), 6),
            s(&["Extraordinarily", "long"])
        );
    }
}
