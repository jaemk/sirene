//! Flowchart to scene (DISPLA-8).

use super::shapes::{self, BoxF, Outline};
use super::style::{self, Resolved};
use super::{
    Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle, Theme,
};
use crate::flowchart::{
    Arrow, Edge, Flowchart, Label, Measure, Node, Shape, Span, Stroke as Line, Subgraph,
};
use crate::layout::{self, Config, Rect, Size, Spacing};

/// Spacing in pixels.
pub const CONFIG: Config = Config {
    vertical: Spacing {
        node_gap: 30,
        edge_gap: 12,
        rank_gap: 20,
        track_gap: 10,
        cluster_cross_pad: 16,
        cluster_main_pad: 12,
        port_inset: 10,
        loop_size: 20,
        loop_inset: 8,
    },
    horizontal: Spacing {
        node_gap: 20,
        edge_gap: 12,
        rank_gap: 24,
        track_gap: 10,
        cluster_cross_pad: 12,
        cluster_main_pad: 16,
        port_inset: 8,
        loop_size: 20,
        loop_inset: 8,
    },
};

/// Space around the drawing.
const MARGIN: f32 = 16.0;
/// Length of a pointed arrowhead.
const ARROW: f32 = 9.0;
/// Radius of edge corners.
const CORNER: f32 = 8.0;
/// Jogs shorter than this between parallel runs are drawn as S-curves.
const JOG: f32 = 24.0;
/// How far an S-curve reaches into the runs on either side of its jog.
const SWING: f32 = 14.0;
/// The straight run kept between a curve and an arrowhead.
const LEAD: f32 = 4.0;

/// Sizes in pixels. Layout rects span `x..=x+w-1`, so each size is one more than the drawn
/// extent.
struct Px<'a, M> {
    metrics: &'a M,
    theme: &'a Theme,
}

fn cells(v: f32) -> i32 {
    v.ceil() as i32 + 1
}

/// Labels wider than this wrap at spaces, as in mermaid.
const WRAP: f32 = 200.0;
/// One-line labels of diamond-like shapes wider than this wrap to keep the shape compact.
const SQUARE_WRAP: f32 = 120.0;

fn style_of(s: &Span) -> TextStyle {
    TextStyle {
        bold: s.bold,
        italic: s.italic,
    }
}

/// Appends `text` in the style of `like` to `line`, merging it into the last span when the style
/// matches.
fn push_span(line: &mut Vec<Span>, text: &str, like: &Span) {
    match line.last_mut() {
        Some(s) if s.bold == like.bold && s.italic == like.italic => s.text.push_str(text),
        _ => line.push(Span {
            text: text.to_owned(),
            ..like.clone()
        }),
    }
}

impl<M: TextMetrics> Px<'_, M> {
    fn width(&self, text: &str, style: TextStyle) -> f32 {
        self.metrics.styled_width(text, self.theme.font_size, style)
    }

    fn line_width(&self, line: &[Span]) -> f32 {
        line.iter().map(|s| self.width(&s.text, style_of(s))).sum()
    }

    fn text_size(&self, lines: &[Vec<Span>]) -> (f32, f32) {
        let w = lines.iter().map(|l| self.line_width(l)).fold(0.0, f32::max);
        (w, lines.len() as f32 * self.theme.line_height)
    }

    /// `lines` with each line wider than `max` broken at spaces; a word wider than `max` keeps
    /// a line of its own.
    fn wrap(&self, lines: &[Vec<Span>], max: f32) -> Vec<Vec<Span>> {
        let mut out = Vec::new();
        for line in lines {
            if self.line_width(line) <= max {
                out.push(line.clone());
                continue;
            }
            // Words, each a run of spans without spaces.
            let mut words: Vec<Vec<Span>> = vec![Vec::new()];
            for span in line {
                for (k, part) in span.text.split(' ').enumerate() {
                    if k > 0 && !words.last().is_some_and(Vec::is_empty) {
                        words.push(Vec::new());
                    }
                    if !part.is_empty() {
                        push_span(words.last_mut().expect("one word"), part, span);
                    }
                }
            }
            let space = self.width(" ", TextStyle::default());
            let mut cur: Vec<Span> = Vec::new();
            let mut cur_w = 0.0;
            for word in words.into_iter().filter(|w| !w.is_empty()) {
                let ww = self.line_width(&word);
                if !cur.is_empty() && cur_w + space + ww > max {
                    out.push(std::mem::take(&mut cur));
                    cur_w = 0.0;
                }
                if !cur.is_empty() {
                    push_span(&mut cur, " ", &Span::default());
                    cur_w += space;
                }
                for s in &word {
                    push_span(&mut cur, &s.text, s);
                }
                cur_w += ww;
            }
            out.push(cur);
        }
        out
    }

    /// A label's lines as drawn: wrapped at `WRAP`, and for a diamond-like `shape` at the width
    /// that makes the smallest shape.
    fn label_lines(&self, label: &Label, shape: Option<Shape>) -> Vec<Vec<Span>> {
        let lines = label.spans();
        let mut best = self.wrap(&lines, WRAP);
        if let Some(shape) = shape.filter(|&s| shapes::grows_square(s))
            && let [line] = lines.as_slice()
            && self.line_width(line) > SQUARE_WRAP
        {
            let area = |ls: &[Vec<Span>]| {
                let (tw, th) = self.text_size(ls);
                let (w, h) = shapes::size(shape, tw, th);
                w * h
            };
            let total = self.line_width(line);
            for k in 2..=4 {
                let candidate = self.wrap(&lines, total / k as f32);
                if area(&candidate) < area(&best) {
                    best = candidate;
                }
            }
        }
        best
    }
}

impl<M: TextMetrics> Measure for Px<'_, M> {
    fn node(&self, node: &Node) -> Size {
        let (tw, th) = if shapes::has_label(node.shape) {
            self.text_size(&self.label_lines(&node.label, Some(node.shape)))
        } else {
            (0.0, 0.0)
        };
        let (w, h) = shapes::size(node.shape, tw, th);
        Size::new(cells(w), cells(h))
    }

    fn edge_label(&self, label: &Label) -> Option<Size> {
        let (tw, th) = self.text_size(&self.label_lines(label, None));
        (tw > 0.0).then(|| Size::new(cells(tw + 8.0), cells(th + 4.0)))
    }

    fn subgraph_title(&self, subgraph: &Subgraph) -> Size {
        let t = subgraph_title(subgraph);
        if t.is_empty() {
            return Size::default();
        }
        let tw = self.metrics.width(&t, self.theme.font_size);
        Size::new(cells(tw + 12.0), cells(self.theme.line_height + 6.0))
    }
}

fn subgraph_title(s: &Subgraph) -> String {
    s.title.lines().join(" ").trim().to_owned()
}

/// Draws `chart`, measuring text with `metrics` (DISPLA-1).
pub fn flowchart(chart: &Flowchart, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let px = Px { metrics, theme };
    let layout = layout::layout(&chart.layout_graph(&px), &CONFIG);
    let title = chart
        .title
        .as_deref()
        .map(|t| crate::flowchart::clean(t).trim().to_owned())
        .filter(|t| !t.is_empty());
    let title_h = if title.is_some() {
        theme.line_height + 12.0
    } else {
        0.0
    };
    let content_w = (layout.size.w - 1).max(0) as f32;
    let content_h = (layout.size.h - 1).max(0) as f32;
    let title_w = title
        .as_deref()
        .map_or(0.0, |t| metrics.width(t, theme.font_size * 1.2));
    let width = content_w.max(title_w) + 2.0 * MARGIN;
    let height = content_h + title_h + 2.0 * MARGIN;
    let off = Pt::new((width - content_w) / 2.0, MARGIN + title_h);
    let at = |r: Rect| BoxF {
        x: r.x as f32 + off.x,
        y: r.y as f32 + off.y,
        w: (r.w - 1) as f32,
        h: (r.h - 1) as f32,
    };

    let mut p = Painter {
        theme,
        metrics,
        elements: Vec::new(),
    };
    if let Some(t) = title {
        p.text(
            Owner::Diagram,
            t,
            Pt::new(width / 2.0, MARGIN + theme.line_height / 2.0),
            theme.font_size * 1.2,
            theme.text,
            TextStyle::default(),
        );
    }

    // Outer clusters first, so nested ones paint over them.
    let mut order: Vec<usize> = (0..chart.subgraphs.len()).collect();
    order.sort_by_key(|&i| {
        std::cmp::Reverse(i64::from(layout.clusters[i].w) * i64::from(layout.clusters[i].h))
    });
    for i in order {
        let s = &chart.subgraphs[i];
        let classes: Vec<&str> = s.classes.iter().map(String::as_str).collect();
        let r = style::resolve(&[], &classes, &s.styles, &chart.classes);
        let b = at(layout.clusters[i]);
        p.path(
            Owner::Cluster(i),
            rect(b),
            Some(r.fill.unwrap_or(theme.cluster_fill)),
            Some(stroke(&r, theme.cluster_stroke, 1.0, None)),
        );
        if let Some(t) = layout.titles[i] {
            p.lines(
                Owner::Cluster(i),
                &[vec![Span::plain(subgraph_title(s))]],
                at(t).center(),
                text_color(&r, theme),
            );
        }
    }

    let outlines: Vec<Outline> = chart
        .nodes
        .iter()
        .zip(&layout.nodes)
        .map(|(n, &r)| shapes::outline(n.shape, at(r)))
        .collect();
    let boundary = |id: &str| -> Vec<Pt> {
        match chart.subgraphs.iter().position(|s| s.id == id) {
            Some(i) => shapes::flatten(&rect(at(layout.clusters[i]))),
            None => {
                let i = chart.nodes.iter().position(|n| n.id == id);
                outlines[i.expect("edge endpoint exists")].boundary.clone()
            }
        }
    };
    let edge_styles: Vec<Resolved> = chart
        .edges
        .iter()
        .map(|e| {
            let classes: Vec<&str> = e.classes.iter().map(String::as_str).collect();
            style::resolve(
                &chart.default_edge_styles,
                &classes,
                &e.styles,
                &chart.classes,
            )
        })
        .collect();
    for (i, (e, route)) in chart.edges.iter().zip(&layout.edges).enumerate() {
        let Some(route) = route else { continue };
        if e.stroke == Line::Invisible {
            continue;
        }
        let points: Vec<Pt> = route
            .points
            .iter()
            .map(|q| Pt::new(q.x as f32 + off.x, q.y as f32 + off.y))
            .collect();
        p.edge(
            i,
            e,
            &edge_styles[i],
            points,
            &boundary(&e.from),
            &boundary(&e.to),
        );
    }

    for (i, ((n, &r), o)) in chart
        .nodes
        .iter()
        .zip(&layout.nodes)
        .zip(outlines)
        .enumerate()
    {
        let mut classes = vec!["default"];
        classes.extend(n.classes.iter().map(String::as_str));
        let s = style::resolve(&[], &classes, &n.styles, &chart.classes);
        let lines = px.label_lines(&n.label, Some(n.shape));
        p.node(i, n, &lines, &s, at(r), o);
    }

    for (i, (e, route)) in chart.edges.iter().zip(&layout.edges).enumerate() {
        let label = route.as_ref().and_then(|r| r.label).zip(e.label.as_ref());
        if let Some((r, l)) = label
            && e.stroke != Line::Invisible
        {
            let b = at(r);
            p.path(Owner::Edge(i), rect(b), Some(theme.edge_label_fill), None);
            let color = edge_styles[i].color.unwrap_or(theme.text);
            p.lines(Owner::Edge(i), &px.label_lines(l, None), b.center(), color);
        }
    }

    log::debug!(
        "drew a {width}x{height} scene of {} elements",
        p.elements.len()
    );
    Scene {
        width,
        height,
        background: theme.background,
        elements: p.elements,
    }
}

fn rect(b: BoxF) -> Vec<Segment> {
    shapes::polygon(&[
        (b.x, b.y),
        (b.right(), b.y),
        (b.right(), b.bottom()),
        (b.x, b.bottom()),
    ])
}

/// The text color for a style: its `color`, or else one that reads on its `fill`. The theme's
/// text color is kept unless it is hard to read there, as dark-theme text on a light fill is.
fn text_color(r: &Resolved, theme: &Theme) -> Color {
    if let Some(c) = r.color {
        return c;
    }
    let Some(fill) = r.fill else {
        return theme.text;
    };
    let fill = fill.over(theme.background);
    if contrast(theme.text, fill) >= 3.0 {
        return theme.text;
    }
    let (dark, light) = (Theme::LIGHT.text, Theme::DARK.text);
    if contrast(dark, fill) >= contrast(light, fill) {
        dark
    } else {
        light
    }
}

/// The WCAG contrast ratio of two opaque colors, from 1 to 21.
fn contrast(a: Color, b: Color) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// WCAG relative luminance.
fn luminance(c: Color) -> f32 {
    let channel = |v: u8| {
        let v = f32::from(v) / 255.0;
        if v <= 0.040_45 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
}

fn stroke(r: &Resolved, color: Color, width: f32, dash: Option<(f32, f32)>) -> Stroke {
    Stroke {
        width: r.stroke_width.unwrap_or(width),
        color: r.stroke.unwrap_or(color),
        dash: r.dash.unwrap_or(dash),
    }
}

struct Painter<'a> {
    theme: &'a Theme,
    metrics: &'a dyn TextMetrics,
    elements: Vec<Element>,
}

impl Painter<'_> {
    fn push(&mut self, owner: Owner, primitive: Primitive) {
        self.elements.push(Element { owner, primitive });
    }

    fn path(
        &mut self,
        owner: Owner,
        segments: Vec<Segment>,
        fill: Option<Color>,
        stroke: Option<Stroke>,
    ) {
        self.push(
            owner,
            Primitive::Path {
                segments,
                fill,
                stroke,
            },
        );
    }

    fn text(
        &mut self,
        owner: Owner,
        text: String,
        center: Pt,
        size: f32,
        color: Color,
        style: TextStyle,
    ) {
        self.push(
            owner,
            Primitive::Text {
                text,
                center,
                size,
                color,
                style,
            },
        );
    }

    /// Lines of styled spans centered as a block on `center`, one text run per span.
    fn lines(&mut self, owner: Owner, lines: &[Vec<Span>], center: Pt, color: Color) {
        let (lh, size) = (self.theme.line_height, self.theme.font_size);
        let width = |text: &str, style| self.metrics.styled_width(text, size, style);
        let top = center.y - lh * (lines.len() as f32 - 1.0) / 2.0;
        let mut runs = Vec::new();
        for (k, line) in lines.iter().enumerate() {
            let y = top + lh * k as f32;
            let total: f32 = line.iter().map(|s| width(&s.text, style_of(s))).sum();
            let mut x = center.x - total / 2.0;
            for span in line {
                let style = style_of(span);
                let trimmed = span.text.trim();
                if !trimmed.is_empty() {
                    let lead = &span.text[..span.text.len() - span.text.trim_start().len()];
                    let left = x + width(lead, style);
                    let mid = left + width(trimmed, style) / 2.0;
                    runs.push((trimmed.to_owned(), Pt::new(mid, y), style));
                }
                x += width(&span.text, style);
            }
        }
        for (text, at, style) in runs {
            self.text(owner, text, at, size, color, style);
        }
    }

    fn node(
        &mut self,
        i: usize,
        node: &Node,
        lines: &[Vec<Span>],
        s: &Resolved,
        b: BoxF,
        o: Outline,
    ) {
        let owner = Owner::Node(i);
        let theme = self.theme;
        let line = stroke(s, theme.node_stroke, 1.0, None);
        let ink = Some(s.stroke.unwrap_or(theme.edge));
        if !o.body.is_empty() {
            if o.solid {
                self.path(owner, o.body, Some(s.fill.unwrap_or(theme.edge)), None);
            } else {
                self.path(
                    owner,
                    o.body,
                    Some(s.fill.unwrap_or(theme.node_fill)),
                    Some(line),
                );
            }
        }
        for l in o.lines {
            self.path(owner, l, None, Some(line));
        }
        for d in o.dots {
            self.path(owner, d, ink, None);
        }
        if shapes::has_label(node.shape) {
            let center = shapes::text_box(node.shape, b).center();
            self.lines(owner, lines, center, text_color(s, theme));
        }
    }

    fn edge(
        &mut self,
        i: usize,
        e: &Edge,
        s: &Resolved,
        mut points: Vec<Pt>,
        from: &[Pt],
        to: &[Pt],
    ) {
        points.dedup();
        straighten(&mut points);
        if points.len() < 2 {
            return;
        }
        let (width, dash) = match e.stroke {
            Line::Thick => (3.5, None),
            Line::Dotted => (2.0, Some((3.0, 3.0))),
            _ => (1.5, None),
        };
        let line = stroke(s, self.theme.edge, width, dash);
        let n = points.len();
        points[0] = clip(points[1], points[0], from);
        points[n - 1] = clip(points[n - 2], points[n - 1], to);
        let scale = arrow_scale(line.width);
        for (at_end, arrow) in [(false, e.start), (true, e.end)] {
            if let Some(a) = arrow {
                make_room(&mut points, at_end, cap_len(a, scale) + LEAD + SWING);
            }
        }
        let n = points.len();
        let owner = Owner::Edge(i);
        let solid = Stroke { dash: None, ..line };
        for (at_end, arrow) in [(false, e.start), (true, e.end)] {
            if let Some(a) = arrow {
                let (end, prev) = if at_end { (n - 1, n - 2) } else { (0, 1) };
                let (marks, inner) = cap(a, points[prev], points[end], line.width);
                for (segments, filled) in marks {
                    if filled {
                        self.path(owner, segments, Some(line.color), None);
                    } else {
                        self.path(owner, segments, None, Some(solid));
                    }
                }
                points[end] = inner;
            }
        }
        self.path(owner, rounded_polyline(&points, CORNER), None, Some(line));
    }
}

/// Removes a jog shorter than `NUDGE` between parallel runs at either end of a route, by moving
/// the end run onto the line of the run before it, so near-aligned nodes connect straight.
fn straighten(points: &mut Vec<Pt>) {
    let jog = |p: &[Pt], a: usize, b: usize, c: usize, d: usize| {
        let shift = Pt::new(p[b].x - p[c].x, p[b].y - p[c].y);
        let parallel = unit(p[a], p[b]) == unit(p[c], p[d]);
        (parallel && shift.x.hypot(shift.y) < NUDGE).then_some(shift)
    };
    let n = points.len();
    if n >= 4
        && let Some(s) = jog(points, n - 4, n - 3, n - 2, n - 1)
    {
        for p in &mut points[n - 2..] {
            *p = Pt::new(p.x + s.x, p.y + s.y);
        }
        points.dedup();
    }
    let n = points.len();
    if n >= 4
        && let Some(s) = jog(points, 3, 2, 1, 0)
    {
        for p in &mut points[..2] {
            *p = Pt::new(p.x + s.x, p.y + s.y);
        }
        points.dedup();
    }
    // Drop the corners that straightening left in the middle of a straight run.
    let mut k = 1;
    while k + 1 < points.len() {
        if unit(points[k - 1], points[k]) == unit(points[k], points[k + 1]) {
            points.remove(k);
        } else {
            k += 1;
        }
    }
}

/// Moves the last jog of a route (the first when `at_end` is false) back from that end until
/// the run after it is at least `need` long, so its curve does not bend under the arrowhead.
fn make_room(points: &mut Vec<Pt>, at_end: bool, need: f32) {
    if !at_end {
        points.reverse();
    }
    loop {
        let n = points.len();
        if n < 4 {
            break;
        }
        let (a, b, c, d) = (points[n - 4], points[n - 3], points[n - 2], points[n - 1]);
        let din = unit(a, b);
        let run = dist(c, d);
        if din != unit(c, d) || run >= need {
            break;
        }
        let shift = (need - run).min((dist(a, b) - SWING).max(0.0));
        points[n - 3] = step(b, din, -shift);
        points[n - 2] = step(c, din, -shift);
        if shift >= need - run || n < 6 {
            break;
        }
        // The run before the jog is too short to move it back: fold it and the jog before it
        // into one jog, from where that one starts.
        let (j, k) = (points[n - 6], points[n - 5]);
        if unit(j, k) != din {
            break;
        }
        // Both jogs together, starting where the first one does.
        let (b, c) = (points[n - 3], points[n - 2]);
        let corner = Pt::new(a.x + c.x - b.x, a.y + c.y - b.y);
        points.splice(n - 5..n - 1, [k, corner]);
        points.dedup();
    }
    if !at_end {
        points.reverse();
    }
}

/// How much an edge's arrowheads grow with its width.
fn arrow_scale(width: f32) -> f32 {
    1.0 + (width - 1.5).max(0.0) / 4.0
}

/// How far back from the tip an arrowhead reaches.
fn cap_len(arrow: Arrow, scale: f32) -> f32 {
    match arrow {
        Arrow::Point => ARROW * scale,
        Arrow::Circle => 8.0 * scale,
        Arrow::Cross => 7.0 * scale + 1.0,
    }
}

/// The longest jog `straighten` removes.
const NUDGE: f32 = 2.0;

/// Moves the end `end` of the segment from `prev` along its direction onto `boundary`.
fn clip(prev: Pt, end: Pt, boundary: &[Pt]) -> Pt {
    let dir = unit(prev, end);
    shapes::ray_hit(boundary, prev, dir, 0.0).unwrap_or(end)
}

fn unit(a: Pt, b: Pt) -> Pt {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len = dx.hypot(dy).max(f32::EPSILON);
    Pt::new(dx / len, dy / len)
}

/// The marks of an arrowhead at `tip`, pointing away from `prev`, as `(path, filled)`, and the
/// point where the line should stop.
fn cap(arrow: Arrow, prev: Pt, tip: Pt, width: f32) -> (Vec<(Vec<Segment>, bool)>, Pt) {
    let d = unit(prev, tip);
    let n = Pt::new(-d.y, d.x);
    let scale = arrow_scale(width);
    let along =
        |k: f32, side: f32| Pt::new(tip.x - d.x * k + n.x * side, tip.y - d.y * k + n.y * side);
    match arrow {
        Arrow::Point => {
            let len = ARROW * scale;
            let half = len / 2.0;
            let head = shapes::polygon(&[
                (tip.x, tip.y),
                (along(len, half).x, along(len, half).y),
                (along(len, -half).x, along(len, -half).y),
            ]);
            (vec![(head, true)], along(len - 1.0, 0.0))
        }
        Arrow::Circle => {
            let r = 4.0 * scale;
            (
                vec![(shapes::ellipse(along(r, 0.0), r, r), true)],
                along(r, 0.0),
            )
        }
        Arrow::Cross => {
            let r = 3.5 * scale;
            let c = along(r + 1.0, 0.0);
            let arm = |k: f32| -> Vec<Segment> {
                vec![
                    Segment::Move(Pt::new(
                        c.x + (d.x + n.x * k) * r,
                        c.y + (d.y + n.y * k) * r,
                    )),
                    Segment::Line(Pt::new(
                        c.x - (d.x + n.x * k) * r,
                        c.y - (d.y + n.y * k) * r,
                    )),
                ]
            };
            (vec![(arm(1.0), false), (arm(-1.0), false)], tip)
        }
    }
}

fn dist(a: Pt, b: Pt) -> f32 {
    (b.x - a.x).hypot(b.y - a.y)
}

fn step(p: Pt, d: Pt, k: f32) -> Pt {
    Pt::new(p.x + d.x * k, p.y + d.y * k)
}

/// A polyline whose corners are rounded with radius up to `radius`. A short jog between two
/// parallel runs becomes one S-curve.
fn rounded_polyline(points: &[Pt], radius: f32) -> Vec<Segment> {
    let n = points.len();
    let mut out = vec![Segment::Move(points[0])];
    let mut k = 1;
    while k + 1 < n {
        let (a, b, c) = (points[k - 1], points[k], points[k + 1]);
        let (din, dout) = (unit(a, b), unit(b, c));
        let cross = din.x * dout.y - din.y * dout.x;
        // The run before this corner may already be half used by the previous corner.
        let len_in = if k == 1 { dist(a, b) } else { dist(a, b) / 2.0 };
        if k + 2 < n && dist(b, c) < JOG {
            let d = points[k + 2];
            let len_out = if k + 3 == n {
                dist(c, d)
            } else {
                dist(c, d) / 2.0
            };
            if unit(c, d) == din && cross.abs() > 1e-3 {
                let s = step(b, din, -SWING.min(len_in));
                let e = step(c, din, SWING.min(len_out));
                out.push(Segment::Line(s));
                out.push(Segment::Cubic {
                    c1: b,
                    c2: c,
                    to: e,
                });
                k += 2;
                continue;
            }
        }
        let r = radius.min(len_in).min(dist(b, c) / 2.0);
        if cross.abs() < 1e-3 || r <= 0.0 {
            out.push(Segment::Line(b));
        } else {
            out.push(Segment::Line(step(b, din, -r)));
            out.push(Segment::Arc {
                radii: Pt::new(r, r),
                large: false,
                sweep: cross > 0.0,
                to: step(b, dout, r),
            });
        }
        k += 1;
    }
    out.push(Segment::Line(points[n - 1]));
    out
}
