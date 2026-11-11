//! Wardley map to scene (MWARD-4), as Mermaid draws it.

use super::shapes;
use super::{
    Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle, Theme,
};
use crate::wardley::{Flow, NodeKind, Strategy, Wardley};

const WIDTH: f32 = 900.0;
const HEIGHT: f32 = 600.0;
const PADDING: f32 = 48.0;
const NODE_RADIUS: f32 = 6.0;
const LABEL_OFFSET: f32 = 8.0;
const AXIS_FONT: f32 = 12.0;
const LABEL_FONT: f32 = 10.0;
/// The side of a pipeline parent's square.
const SQUARE: f32 = NODE_RADIUS * 1.6;
/// Mermaid's `wardleyEvolutionColor`.
const EVOLUTION: Color = Color::rgb(0xdc3545);
/// Where text with an alphabetic baseline at `y` is centered, in ems above it.
const BASELINE: f32 = 0.3;

struct Draw<'a, M: TextMetrics> {
    metrics: &'a M,
    theme: &'a Theme,
    out: Vec<Element>,
}

fn el(primitive: Primitive) -> Element {
    Element {
        owner: Owner::Diagram,
        primitive,
    }
}

fn stroke(width: f32, color: Color, dash: Option<(f32, f32)>) -> Option<Stroke> {
    Some(Stroke { width, color, dash })
}

fn circle(c: Pt, r: f32) -> Vec<Segment> {
    shapes::ellipse(c, r, r)
}

/// The unit vector from `a` to `b`, or zero when they meet.
fn unit(a: Pt, b: Pt) -> Pt {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let d = (dx * dx + dy * dy).sqrt();
    if d == 0.0 {
        Pt::new(0.0, 0.0)
    } else {
        Pt::new(dx / d, dy / d)
    }
}

impl<M: TextMetrics> Draw<'_, M> {
    fn path(&mut self, segments: Vec<Segment>, fill: Option<Color>, stroke: Option<Stroke>) {
        self.out.push(el(Primitive::Path {
            segments,
            fill,
            stroke,
        }));
    }

    fn line(&mut self, a: Pt, b: Pt, stroke: Option<Stroke>) {
        self.path(vec![Segment::Move(a), Segment::Line(b)], None, stroke);
    }

    /// A filled triangle with its tip at `tip`, pointing along `dir`, `len` long and wide.
    fn head(&mut self, tip: Pt, dir: Pt, len: f32, color: Color) {
        let base = Pt::new(tip.x - dir.x * len, tip.y - dir.y * len);
        let (nx, ny) = (-dir.y * len / 2.0, dir.x * len / 2.0);
        let pts = [
            (tip.x, tip.y),
            (base.x + nx, base.y + ny),
            (base.x - nx, base.y - ny),
        ];
        self.path(shapes::polygon(&pts), Some(color), None);
    }

    fn text(&mut self, text: &str, center: Pt, size: f32, color: Color, bold: bool) {
        if text.is_empty() {
            return;
        }
        self.out.push(el(Primitive::Text {
            text: text.to_owned(),
            center,
            size,
            color,
            style: TextStyle {
                bold,
                italic: false,
            },
        }));
    }

    fn width(&self, text: &str, size: f32, bold: bool) -> f32 {
        self.metrics.styled_width(
            text,
            size,
            TextStyle {
                bold,
                italic: false,
            },
        )
    }

    /// Text starting at `x` with its baseline at `y`.
    fn text_start(&mut self, text: &str, x: f32, y: f32, size: f32, color: Color, bold: bool) {
        let w = self.width(text, size, bold);
        self.text(
            text,
            Pt::new(x + w / 2.0, y - BASELINE * size),
            size,
            color,
            bold,
        );
    }
}

/// Draws `map` (MWARD-4).
pub fn wardley(map: &Wardley, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let (width, height) = map.size.unwrap_or((WIDTH, HEIGHT));
    let chart_w = width - PADDING * 2.0;
    let chart_h = height - PADDING * 2.0;
    let px = |v: f64| PADDING + v as f32 / 100.0 * chart_w;
    let py = |v: f64| height - PADDING - v as f32 / 100.0 * chart_h;
    let line_color = theme.edge;
    let text_color = theme.text;
    let mut d = Draw {
        metrics,
        theme,
        out: Vec::new(),
    };
    if let Some(t) = map
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        d.text(
            t,
            Pt::new(width / 2.0, PADDING / 2.0),
            AXIS_FONT * 1.05,
            text_color,
            true,
        );
    }

    // Axes, their labels, and the evolution stages.
    let axis = stroke(1.0, line_color, None);
    d.line(
        Pt::new(PADDING, height - PADDING),
        Pt::new(width - PADDING, height - PADDING),
        axis,
    );
    d.line(
        Pt::new(PADDING, PADDING),
        Pt::new(PADDING, height - PADDING),
        axis,
    );
    d.text(
        "Evolution",
        Pt::new(
            PADDING + chart_w / 2.0,
            height - PADDING / 4.0 - BASELINE * AXIS_FONT,
        ),
        AXIS_FONT,
        text_color,
        true,
    );
    // Mermaid turns this up the axis; it sits above the axis instead.
    d.text(
        "Visibility",
        Pt::new(PADDING, PADDING - AXIS_FONT),
        AXIS_FONT,
        text_color,
        true,
    );
    let stages = map.stage_names();
    let spans: Vec<(f32, f32)> = if map.boundaries.len() == stages.len() {
        let mut prev = 0.0;
        map.boundaries
            .iter()
            .map(|&b| {
                let span = (prev, b as f32);
                prev = b as f32;
                span
            })
            .collect()
    } else {
        let w = 1.0 / stages.len() as f32;
        (0..stages.len())
            .map(|i| (i as f32 * w, (i + 1) as f32 * w))
            .collect()
    };
    for (i, (stage, (start, end))) in stages.iter().zip(spans).enumerate() {
        let x0 = PADDING + start * chart_w;
        let x1 = PADDING + end * chart_w;
        if i > 0 {
            d.line(
                Pt::new(x0, PADDING),
                Pt::new(x0, height - PADDING),
                stroke(1.0, line_color.with_alpha(204), Some((5.0, 5.0))),
            );
        }
        let size = AXIS_FONT - 2.0;
        d.text(
            stage,
            Pt::new((x0 + x1) / 2.0, height - PADDING / 1.5 - BASELINE * size),
            size,
            text_color,
            false,
        );
    }

    // Node positions; a pipeline's parent moves to the top of its box.
    let mut pos: Vec<Pt> = map
        .nodes
        .iter()
        .map(|n| Pt::new(px(n.x), py(n.y)))
        .collect();
    for p in &map.pipelines {
        let mut members: Vec<usize> = p.components.clone();
        if members.is_empty() {
            continue;
        }
        members.sort_by(|&a, &b| map.nodes[a].x.total_cmp(&map.nodes[b].x));
        for w in members.windows(2) {
            d.line(
                pos[w[0]],
                pos[w[1]],
                stroke(1.0, line_color, Some((4.0, 4.0))),
            );
        }
        let min = members
            .iter()
            .map(|&i| pos[i].x)
            .fold(f32::INFINITY, f32::min);
        let max = members
            .iter()
            .map(|&i| pos[i].x)
            .fold(f32::NEG_INFINITY, f32::max);
        let y = pos[*p.components.last().unwrap_or(&members[0])].y;
        let box_h = NODE_RADIUS * 4.0;
        let top = y - box_h / 2.0;
        pos[p.parent] = Pt::new((min + max) / 2.0, top - SQUARE / 6.0);
        let b = shapes::BoxF {
            x: min - 15.0,
            y: top,
            w: max - min + 30.0,
            h: box_h,
        };
        d.path(shapes::rounded(b, 4.0), None, stroke(1.5, line_color, None));
    }

    // Links, then their labels.
    let radius = |i: usize| {
        if map.nodes[i].pipeline_parent {
            SQUARE / 2f32.sqrt()
        } else {
            NODE_RADIUS
        }
    };
    let links: Vec<_> = map
        .links
        .iter()
        .filter(|l| {
            // Not from a pipeline's component to its parent.
            !map.pipelines
                .iter()
                .any(|p| p.parent == l.target && p.components.contains(&l.source))
        })
        .collect();
    for l in &links {
        let (s, t) = (pos[l.source], pos[l.target]);
        let u = unit(s, t);
        let a = Pt::new(s.x + u.x * radius(l.source), s.y + u.y * radius(l.source));
        let b = Pt::new(t.x - u.x * radius(l.target), t.y - u.y * radius(l.target));
        let dash = l.dashed.then_some((4.0, 4.0));
        d.line(a, b, stroke(1.0, line_color, dash));
        if matches!(l.flow, Some(Flow::Forward | Flow::Both)) {
            d.head(b, u, 5.0, line_color);
        }
        if matches!(l.flow, Some(Flow::Backward | Flow::Both)) {
            d.head(a, Pt::new(-u.x, -u.y), 5.0, line_color);
        }
    }
    for l in &links {
        let Some(label) = l.label.as_deref() else {
            continue;
        };
        let (s, t) = (pos[l.source], pos[l.target]);
        let u = unit(s, t);
        // Mermaid turns the label along the link; it stays level here.
        let at = Pt::new((s.x + t.x) / 2.0 + u.y * 8.0, (s.y + t.y) / 2.0 - u.x * 8.0);
        d.text(label, at, LABEL_FONT, text_color, false);
    }

    // Evolve arrows, stopping short of the target.
    for t in &map.trends {
        let from = pos[t.node];
        let to = Pt::new(px(t.x), py(t.y));
        let u = unit(from, to);
        let dist = ((to.x - from.x).powi(2) + (to.y - from.y).powi(2)).sqrt();
        let short = NODE_RADIUS + 2.0;
        let end = if dist > short {
            Pt::new(to.x - u.x * short, to.y - u.y * short)
        } else {
            to
        };
        d.line(from, end, stroke(1.0, EVOLUTION, Some((4.0, 4.0))));
        d.head(end, u, 6.0, EVOLUTION);
    }

    for (i, &p) in pos.iter().enumerate() {
        node(&mut d, map, i, p);
    }
    annotations(&mut d, map, width, height, &px, &py);
    for n in &map.notes {
        // Kept inside the chart; Mermaid lets a note near the right run off it.
        let w = d.width(&n.text, 11.0, true);
        let x = px(n.x).min(width - PADDING / 2.0 - w).max(0.0);
        d.text_start(&n.text, x, py(n.y), 11.0, text_color, true);
    }
    for (markers, right) in [(&map.accelerators, true), (&map.deaccelerators, false)] {
        for m in markers {
            let (x, y) = (px(m.x), py(m.y));
            let (w, h, head) = (60.0, 30.0, 20.0);
            let pts: Vec<(f32, f32)> = if right {
                vec![
                    (x, y - h / 2.0),
                    (x + w - head, y - h / 2.0),
                    (x + w - head, y - h / 2.0 - 8.0),
                    (x + w, y),
                    (x + w - head, y + h / 2.0 + 8.0),
                    (x + w - head, y + h / 2.0),
                    (x, y + h / 2.0),
                ]
            } else {
                vec![
                    (x + w, y - h / 2.0),
                    (x + head, y - h / 2.0),
                    (x + head, y - h / 2.0 - 8.0),
                    (x, y),
                    (x + head, y + h / 2.0 + 8.0),
                    (x + head, y + h / 2.0),
                    (x + w, y + h / 2.0),
                ]
            };
            d.path(
                shapes::polygon(&pts),
                Some(theme.background),
                stroke(1.0, line_color, None),
            );
            d.text(
                &m.text,
                Pt::new(x + w / 2.0, y + h / 2.0 + 15.0 - BASELINE * LABEL_FONT),
                LABEL_FONT,
                text_color,
                true,
            );
        }
    }
    Scene {
        width,
        height,
        background: theme.background,
        elements: d.out,
    }
}

/// Draws node `i` at `p`: its sourcing overlay, circle or square, inertia bar, and label.
fn node<M: TextMetrics>(d: &mut Draw<'_, M>, map: &Wardley, i: usize, p: Pt) {
    let n = &map.nodes[i];
    let line = d.theme.edge;
    let fill = d.theme.background;
    let thin = stroke(1.0, line, None);
    if let Some(s) = n.strategy {
        // Mermaid's stylesheet paints these all in the component fill; the greys tell them
        // apart, as its renderer means them to.
        let overlay = match s {
            Strategy::Outsource => Color::rgb(0x666666),
            Strategy::Buy => Color::rgb(0xcccccc),
            Strategy::Build => Color::rgb(0xeeeeee),
            Strategy::Market => fill,
        };
        d.path(circle(p, NODE_RADIUS * 2.0), Some(overlay), thin);
    }
    let market = n.strategy == Some(Strategy::Market);
    if !n.pipeline_parent && !market && n.kind != NodeKind::Anchor {
        d.path(circle(p, NODE_RADIUS), Some(fill), thin);
    }
    if market {
        let r = NODE_RADIUS * 1.2;
        let (c, s) = ((std::f32::consts::PI / 6.0).cos(), 0.5);
        let dots = [
            Pt::new(p.x, p.y - r),
            Pt::new(p.x - r * c, p.y + r * s),
            Pt::new(p.x + r * c, p.y + r * s),
        ];
        for k in 0..3 {
            d.line(dots[k], dots[(k + 1) % 3], thin);
        }
        for dot in dots {
            d.path(
                circle(dot, NODE_RADIUS * 0.7),
                Some(fill),
                stroke(2.0, line, None),
            );
        }
    }
    if n.pipeline_parent {
        let b = shapes::BoxF {
            x: p.x - SQUARE / 2.0,
            y: p.y - SQUARE / 2.0,
            w: SQUARE,
            h: SQUARE,
        };
        d.path(shapes::rounded(b, 0.0), Some(fill), thin);
    }
    if n.inertia {
        let mut off = if n.pipeline_parent {
            SQUARE / 2.0 + 15.0
        } else {
            NODE_RADIUS + 15.0
        };
        if n.strategy.is_some() {
            off += NODE_RADIUS + 10.0;
        }
        let h = if n.pipeline_parent {
            SQUARE
        } else {
            NODE_RADIUS * 2.0
        };
        d.line(
            Pt::new(p.x + off, p.y - h / 2.0),
            Pt::new(p.x + off, p.y + h / 2.0),
            stroke(6.0, line, None),
        );
    }
    let color = d.theme.text;
    let (ox, oy) = (n.label_offset.map(|o| o.0), n.label_offset.map(|o| o.1));
    if n.kind == NodeKind::Anchor {
        let at = Pt::new(p.x + ox.unwrap_or(0.0), p.y + oy.unwrap_or(-3.0));
        d.text(&n.label, at, LABEL_FONT, color, true);
    } else {
        let extra = if n.strategy.is_some() { 10.0 } else { 0.0 };
        let x = p.x + ox.unwrap_or(LABEL_OFFSET + extra);
        let y = p.y + oy.unwrap_or(-LABEL_OFFSET - extra);
        d.text_start(&n.label, x, y, LABEL_FONT, color, false);
    }
}

/// Draws the numbered annotations and the box listing their texts.
fn annotations<M: TextMetrics>(
    d: &mut Draw<'_, M>,
    map: &Wardley,
    width: f32,
    height: f32,
    px: &impl Fn(f64) -> f32,
    py: &impl Fn(f64) -> f32,
) {
    let line = d.theme.edge;
    let color = d.theme.text;
    let fill = d.theme.background;
    for a in &map.annotations {
        let c = Pt::new(px(a.x), py(a.y));
        d.path(circle(c, 10.0), Some(fill), stroke(1.5, line, None));
        d.text(&a.number.to_string(), c, 10.0, color, true);
    }
    let Some((bx, by)) = map.annotations_box else {
        return;
    };
    if map.annotations.is_empty() {
        return;
    }
    let mut sorted: Vec<_> = map
        .annotations
        .iter()
        .filter(|a| !a.text.is_empty())
        .collect();
    sorted.sort_by_key(|a| a.number);
    if sorted.is_empty() {
        return;
    }
    let (pad, line_h, size) = (10.0, 16.0, 11.0);
    let lines: Vec<String> = sorted
        .iter()
        .map(|a| format!("{}. {}", a.number, a.text))
        .collect();
    let max_w = lines
        .iter()
        .map(|l| d.width(l, size, false))
        .fold(0.0, f32::max);
    let box_w = max_w + pad * 2.0 + 105.0;
    let box_h = lines.len() as f32 * line_h + pad * 2.0 + size * 1.2 / 2.0;
    let x = px(bx).min(width - PADDING - box_w).max(PADDING);
    let y = py(by).min(height - PADDING - box_h).max(PADDING);
    let b = shapes::BoxF {
        x,
        y,
        w: box_w,
        h: box_h,
    };
    d.path(shapes::rounded(b, 4.0), Some(fill), stroke(1.5, line, None));
    for (k, l) in lines.iter().enumerate() {
        let w = d.width(l, size, false);
        let at = Pt::new(x + pad + w / 2.0, y + pad + (k + 1) as f32 * line_h);
        d.text(l, at, size, color, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Approx;

    fn draw(src: &str) -> Scene {
        let (map, d) = crate::wardley::parse(src).lower();
        assert!(d.is_empty(), "{d:?}");
        wardley(&map, &Approx, &Theme::LIGHT)
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

    #[test]
    fn positions() {
        let s = draw("wardley-beta\ncomponent A [0.5, 0.25]\n");
        assert_eq!((s.width, s.height), (WIDTH, HEIGHT));
        let circle_center = s.elements.iter().find_map(|e| match &e.primitive {
            Primitive::Path {
                segments,
                fill: Some(_),
                ..
            } => {
                let pts = shapes::flatten(segments);
                let n = pts.len() as f32;
                Some(Pt::new(
                    pts.iter().map(|p| p.x).sum::<f32>() / n,
                    pts.iter().map(|p| p.y).sum::<f32>() / n,
                ))
            }
            _ => None,
        });
        let c = circle_center.unwrap();
        // Evolution 0.25 across, visibility 0.5 up.
        assert!(
            (c.x - (PADDING + 0.25 * (WIDTH - 96.0))).abs() < 0.5,
            "{c:?}"
        );
        assert!(
            (c.y - (HEIGHT - PADDING - 0.5 * (HEIGHT - 96.0))).abs() < 0.5,
            "{c:?}"
        );
        let t = texts(&s);
        for want in [
            "Genesis",
            "Custom Built",
            "Product",
            "Commodity",
            "Evolution",
            "A",
        ] {
            assert!(t.iter().any(|(s, _)| s == want), "{want} in {t:?}");
        }
    }

    #[test]
    fn size_and_stages() {
        let s = draw("wardley-beta\nsize [400, 300]\nevolution A@0.5 -> B@0.75 -> C@1.0\n");
        assert_eq!((s.width, s.height), (400.0, 300.0));
        let t = texts(&s);
        let x = |name: &str| t.iter().find(|(s, _)| s == name).unwrap().1.x;
        let w = 400.0 - 96.0;
        assert!((x("A") - (48.0 + 0.25 * w)).abs() < 0.01);
        assert!((x("C") - (48.0 + 0.875 * w)).abs() < 0.01);
    }

    #[test]
    fn pipeline_parent_moves_above_its_box() {
        let s = draw(
            "wardley-beta\ncomponent P [0.5, 0.5]\npipeline P {\n  component X [0.2]\n  \
             component Y [0.8]\n}\n",
        );
        let t = texts(&s);
        let p = t.iter().find(|(s, _)| s == "P").unwrap().1;
        let x = t.iter().find(|(s, _)| s == "X").unwrap().1;
        let y = t.iter().find(|(s, _)| s == "Y").unwrap().1;
        // Centered between its components, a box's half height above them.
        let mid = (x.x - Approx.width("X", 10.0) / 2.0 + y.x - Approx.width("Y", 10.0) / 2.0) / 2.0;
        assert!(
            (p.x - Approx.width("P", 10.0) / 2.0 - mid).abs() < 0.01,
            "{p:?} {mid}"
        );
        assert!(p.y < x.y - 10.0);
    }
}
