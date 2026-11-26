//! A diagram with its `header`, `footer`, `caption`, and `legend` (PCOM-1): the diagram's own
//! scene, moved down and widened to make room, with the decorations drawn around it.
//!
//! From the top: the header, a top legend, the diagram, a bottom legend, the caption, the
//! footer. Each is set apart from its neighbors by [`PAD`]. Text is set at the theme's font size
//! on lines one line height tall, or as tall as the formulas in them (LEMB-4).

use super::mathtext::{self, Formulas};
use super::{
    Color, Element, Owner, Primitive, Pt, Rect, Scene, Segment, Stroke, TextMetrics, Theme,
    parse_color,
};
use crate::decor::{self, Decorated, HAlign, Rule, Run, VAlign};
use crate::flowchart::Span;

/// Space above and below each decoration, in pixels.
const PAD: f32 = 8.0;
/// The least space between a decoration and the side of the drawing.
const SIDE: f32 = 16.0;
/// Space between a legend's text and its border.
const INSET: f32 = 8.0;

/// A decoration set out from its top left corner.
struct Part {
    w: f32,
    h: f32,
    primitives: Vec<Primitive>,
    align: HAlign,
}

/// The scene of `d`: its diagram with the decorations around it.
pub fn decorated(d: &Decorated, metrics: &impl TextMetrics, theme: &Theme) -> crate::Result<Scene> {
    let mut scene = d.diagram.scene(metrics, theme)?;
    let formulas = Formulas::default();
    let banner = |b: &Option<decor::Banner>| {
        b.as_ref()
            .map(|b| text_block(metrics, &formulas, theme, &b.lines, b.align, b.align))
    };
    let header = banner(&d.decor.header);
    let footer = banner(&d.decor.footer);
    let caption = banner(&d.decor.caption);
    let legend = d.decor.legend.as_ref().map(|l| {
        let inner = text_block(
            metrics,
            &formulas,
            theme,
            &l.lines,
            HAlign::Left,
            l.horizontal,
        );
        boxed(inner, theme)
    });
    let top_legend = (d.decor.legend.as_ref()).is_some_and(|l| l.vertical == VAlign::Top);
    let (above, below) = if top_legend {
        (vec![header, legend], vec![caption, footer])
    } else {
        (vec![header], vec![legend, caption, footer])
    };
    let above: Vec<Part> = above.into_iter().flatten().collect();
    let below: Vec<Part> = below.into_iter().flatten().collect();

    let stack = |parts: &[Part]| parts.iter().map(|p| p.h + 2.0 * PAD).sum::<f32>();
    let widest = above
        .iter()
        .chain(&below)
        .map(|p| p.w + 2.0 * SIDE)
        .fold(0.0, f32::max);
    let width = scene.width.max(widest);
    let top = stack(&above);
    let height = top + scene.height + stack(&below);

    let dx = (width - scene.width) / 2.0;
    for e in &mut scene.elements {
        translate(&mut e.primitive, Pt::new(dx, top));
    }
    let mut elements = Vec::new();
    let mut y = 0.0;
    for p in &above {
        place(p, y, width, &mut elements);
        y += p.h + 2.0 * PAD;
    }
    y += scene.height;
    let inner = std::mem::take(&mut scene.elements);
    for p in &below {
        place(p, y, width, &mut elements);
        y += p.h + 2.0 * PAD;
    }
    // The decorations above are painted first, then the diagram, then the ones below.
    let split = above.iter().map(|p| p.primitives.len()).sum::<usize>();
    let mut out: Vec<Element> = elements.drain(..split).collect();
    out.extend(inner);
    out.extend(elements);
    Ok(Scene {
        width,
        height,
        background: scene.background,
        elements: out,
    })
}

/// Adds `part` to `out`, its box starting `PAD` below `y`, set against the side `part.align` says
/// of a drawing `width` wide.
fn place(part: &Part, y: f32, width: f32, out: &mut Vec<Element>) {
    let x = match part.align {
        HAlign::Left => SIDE,
        HAlign::Center => (width - part.w) / 2.0,
        HAlign::Right => width - SIDE - part.w,
    };
    for p in &part.primitives {
        let mut p = p.clone();
        translate(&mut p, Pt::new(x, y + PAD));
        out.push(Element {
            owner: Owner::Diagram,
            primitive: p,
        });
    }
}

/// `part` in a bordered box.
fn boxed(part: Part, theme: &Theme) -> Part {
    let (w, h) = (part.w + 2.0 * INSET, part.h + 2.0 * INSET);
    let mut primitives = vec![Primitive::Path {
        segments: vec![
            Segment::Move(Pt::new(0.0, 0.0)),
            Segment::Line(Pt::new(w, 0.0)),
            Segment::Line(Pt::new(w, h)),
            Segment::Line(Pt::new(0.0, h)),
            Segment::Close,
        ],
        fill: Some(theme.cluster_fill),
        stroke: Some(Stroke {
            width: 1.0,
            color: theme.cluster_stroke,
            dash: None,
        }),
        clip: None,
    }];
    for mut p in part.primitives {
        translate(&mut p, Pt::new(INSET, INSET));
        primitives.push(p);
    }
    Part {
        w,
        h,
        primitives,
        align: part.align,
    }
}

/// The color a `<color:...>` names, or the theme's text color.
fn color_of(name: Option<&str>, theme: &Theme) -> Color {
    name.and_then(named).unwrap_or(theme.text)
}

/// Height of a rule line, in pixels.
const RULE: f32 = 8.0;
/// Space between a table cell's text and its border, across and down, in pixels.
const CELL_X: f32 = 6.0;
const CELL_Y: f32 = 2.0;

/// A line of a block: text, a rule, or the rows of a table.
enum Item {
    Text(Vec<Run>),
    Rule(Rule),
    Table(Vec<Vec<Vec<Run>>>),
}

/// The lines of a block as items, the list marker of a line made the start of its text and
/// the rows of a table gathered.
fn items(lines: &[String]) -> Vec<Item> {
    let mut out: Vec<Item> = Vec::new();
    for line in decor::lines(lines) {
        if let Some(rule) = line.rule {
            out.push(Item::Rule(rule));
        } else if !line.cells.is_empty() {
            match out.last_mut() {
                Some(Item::Table(rows)) => rows.push(line.cells),
                _ => out.push(Item::Table(vec![line.cells])),
            }
        } else {
            let mut runs = line.runs;
            let mut prefix = line.marker.prefix("\u{2022}");
            // The marker is the start of the line's first run, when that is text.
            if let Some(first) = runs.first_mut().filter(|r| !r.span.math) {
                first.span.text.insert_str(0, &std::mem::take(&mut prefix));
            }
            if !prefix.is_empty() {
                runs.insert(
                    0,
                    Run {
                        span: Span::plain(prefix),
                        ..Run::default()
                    },
                );
            }
            out.push(Item::Text(runs));
        }
    }
    out
}

/// A row of runs measured: its width, its height, and the height above its baseline when it
/// holds a formula.
#[derive(Clone, Copy)]
struct Geo {
    w: f32,
    h: f32,
    above: Option<f32>,
}

/// A table measured: its column widths and row heights, and the geometry of each cell.
struct Grid {
    cols: Vec<f32>,
    rows: Vec<f32>,
    cells: Vec<Vec<Geo>>,
}

impl Grid {
    fn w(&self) -> f32 {
        self.cols.iter().sum()
    }

    fn h(&self) -> f32 {
        self.rows.iter().sum()
    }
}

/// What a block is measured and drawn with.
struct Ctx<'a, M: TextMetrics + ?Sized> {
    metrics: &'a M,
    formulas: &'a Formulas,
    theme: &'a Theme,
}

impl<M: TextMetrics + ?Sized> Ctx<'_, M> {
    fn size(&self, run: &Run) -> f32 {
        run.size.map_or(self.theme.font_size, |n| n as f32)
    }

    fn measure(&self, runs: &[Run]) -> Geo {
        let w = runs
            .iter()
            .map(|r| mathtext::span_width(self.metrics, self.formulas, self.size(r), &r.span))
            .sum();
        let spans: Vec<Span> = runs.iter().map(|r| r.span.clone()).collect();
        match mathtext::line_box(self.formulas, self.theme, &spans) {
            Some((above, below)) => Geo {
                w,
                h: above + below,
                above: Some(above),
            },
            None => {
                let big = runs
                    .iter()
                    .map(|r| self.size(r))
                    .fold(self.theme.font_size, f32::max);
                Geo {
                    w,
                    h: self.theme.line_height * big / self.theme.font_size,
                    above: None,
                }
            }
        }
    }

    fn grid(&self, rows: &[Vec<Vec<Run>>]) -> Grid {
        let cells: Vec<Vec<Geo>> = rows
            .iter()
            .map(|row| row.iter().map(|c| self.measure(c)).collect())
            .collect();
        let count = cells.iter().map(Vec::len).max().unwrap_or(0);
        let cols = (0..count)
            .map(|c| {
                let widest = cells.iter().filter_map(|r| r.get(c)).map(|g| g.w);
                widest.fold(0.0, f32::max) + 2.0 * CELL_X
            })
            .collect();
        let rows = cells
            .iter()
            .map(|r| r.iter().map(|g| g.h).fold(0.0, f32::max) + 2.0 * CELL_Y)
            .collect();
        Grid { cols, rows, cells }
    }

    /// Draws `runs`, measured as `g`, on a row from `(x, top)`.
    fn draw(&self, runs: &[Run], g: Geo, x: f32, top: f32, out: &mut Vec<Primitive>) {
        let theme = self.theme;
        let baseline = top + g.above.unwrap_or(g.h / 2.0);
        let mut x = x;
        for run in runs {
            let size = self.size(run);
            let color = color_of(run.color.as_deref(), theme);
            if let Some(f) = self.formulas.inline(&run.span) {
                out.extend(f.place(Pt::new(x, baseline), size, color));
                x += f.width * size;
                continue;
            }
            let mid = match g.above {
                Some(_) => baseline - mathtext::BASELINE * size,
                None => baseline,
            };
            let style = mathtext::style_of(&run.span);
            let width = |t: &str| self.metrics.styled_width(t, size, style);
            let text = &run.span.text;
            let back = run.back.as_deref().and_then(named);
            if let Some(fill) = back {
                let half = theme.line_height * size / theme.font_size / 2.0;
                out.push(rect(
                    x,
                    mid - half,
                    width(text),
                    2.0 * half,
                    Some(fill),
                    None,
                ));
            }
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                let lead = &text[..text.len() - text.trim_start().len()];
                let left = x + width(lead);
                let w = width(trimmed);
                out.push(Primitive::Text {
                    text: trimmed.to_owned(),
                    center: Pt::new(left + w / 2.0, mid),
                    size,
                    color,
                    style,
                });
                if run.underline {
                    out.push(hline(left, left + w, mid + size * 0.55, 1.0, color, None));
                }
                if run.strike {
                    out.push(hline(left, left + w, mid, 1.0, color, None));
                }
            }
            x += width(text);
        }
    }
}

/// The color a name is, without the theme's default.
fn named(name: &str) -> Option<Color> {
    parse_color(name).or_else(|| parse_color(&format!("#{}", name.trim_start_matches('#'))))
}

/// A horizontal line from `x0` to `x1` at `y`.
fn hline(
    x0: f32,
    x1: f32,
    y: f32,
    width: f32,
    color: Color,
    dash: Option<(f32, f32)>,
) -> Primitive {
    Primitive::Path {
        segments: vec![Segment::Move(Pt::new(x0, y)), Segment::Line(Pt::new(x1, y))],
        fill: None,
        stroke: Some(Stroke { width, color, dash }),
        clip: None,
    }
}

/// A rectangle, filled and stroked as given.
fn rect(x: f32, y: f32, w: f32, h: f32, fill: Option<Color>, stroke: Option<Stroke>) -> Primitive {
    Primitive::Path {
        segments: vec![
            Segment::Move(Pt::new(x, y)),
            Segment::Line(Pt::new(x + w, y)),
            Segment::Line(Pt::new(x + w, y + h)),
            Segment::Line(Pt::new(x, y + h)),
            Segment::Close,
        ],
        fill,
        stroke,
        clip: None,
    }
}

/// `lines` as a block of text: each line is a row of runs on one baseline, a rule across the
/// block, or the rows of a table, set against the side `within` says of the block's own width.
/// The block sits at the side `align` says.
fn text_block(
    metrics: &(impl TextMetrics + ?Sized),
    formulas: &Formulas,
    theme: &Theme,
    lines: &[String],
    within: HAlign,
    align: HAlign,
) -> Part {
    let cx = Ctx {
        metrics,
        formulas,
        theme,
    };
    enum Laid {
        Text(Geo),
        Rule,
        Table(Grid),
    }
    let laid: Vec<(Item, Laid)> = items(lines)
        .into_iter()
        .map(|item| {
            let m = match &item {
                Item::Text(runs) => Laid::Text(cx.measure(runs)),
                Item::Rule(_) => Laid::Rule,
                Item::Table(rows) => Laid::Table(cx.grid(rows)),
            };
            (item, m)
        })
        .collect();
    let size_of = |m: &Laid| match m {
        Laid::Text(g) => (g.w, g.h),
        Laid::Rule => (0.0, RULE),
        Laid::Table(g) => (g.w(), g.h()),
    };
    let w = laid.iter().map(|(_, m)| size_of(m).0).fold(0.0, f32::max);
    let h = laid.iter().map(|(_, m)| size_of(m).1).sum::<f32>();
    let mut primitives = Vec::new();
    let mut top = 0.0;
    for (item, m) in &laid {
        let (own, height) = size_of(m);
        let x = match within {
            HAlign::Left => 0.0,
            HAlign::Center => (w - own) / 2.0,
            HAlign::Right => w - own,
        };
        match (item, m) {
            (Item::Text(runs), Laid::Text(g)) => cx.draw(runs, *g, x, top, &mut primitives),
            (Item::Rule(rule), _) => {
                let y = top + height / 2.0;
                let ink = theme.text;
                match rule {
                    Rule::Single => primitives.push(hline(0.0, w, y, 1.0, ink, None)),
                    Rule::Double => {
                        primitives.push(hline(0.0, w, y - 1.5, 1.0, ink, None));
                        primitives.push(hline(0.0, w, y + 1.5, 1.0, ink, None));
                    }
                    Rule::Thick => primitives.push(hline(0.0, w, y, 2.0, ink, None)),
                    Rule::Dotted => primitives.push(hline(0.0, w, y, 1.0, ink, Some((1.0, 3.0)))),
                }
            }
            (Item::Table(rows), Laid::Table(grid)) => {
                let mut y = top;
                for (r, row) in rows.iter().enumerate() {
                    let mut cx0 = x;
                    for (c, cell) in row.iter().enumerate() {
                        let (cw, rh) = (grid.cols[c], grid.rows[r]);
                        let border = Stroke {
                            width: 1.0,
                            color: theme.cluster_stroke,
                            dash: None,
                        };
                        primitives.push(rect(cx0, y, cw, rh, None, Some(border)));
                        let g = grid.cells[r][c];
                        let inner = y + (rh - g.h) / 2.0;
                        cx.draw(cell, g, cx0 + CELL_X, inner, &mut primitives);
                        cx0 += cw;
                    }
                    y += grid.rows[r];
                }
            }
            _ => {}
        }
        top += height;
    }
    Part {
        w,
        h,
        primitives,
        align,
    }
}

/// Moves a primitive by `d`, and the rectangle it is clipped to.
fn translate(primitive: &mut Primitive, d: Pt) {
    let shift = |p: &mut Pt| {
        p.x += d.x;
        p.y += d.y;
    };
    match primitive {
        Primitive::Path { segments, clip, .. } => {
            for s in segments {
                match s {
                    Segment::Move(p) | Segment::Line(p) => shift(p),
                    Segment::Arc { to, .. } => shift(to),
                    Segment::Cubic { c1, c2, to } => {
                        shift(c1);
                        shift(c2);
                        shift(to);
                    }
                    Segment::Close => {}
                }
            }
            if let Some(Rect { x, y, .. }) = clip {
                *x += d.x;
                *y += d.y;
            }
        }
        Primitive::Text { center, .. } => shift(center),
    }
}

#[cfg(all(test, feature = "math"))]
mod tests {
    use super::super::{Approx, Theme};

    /// A formula in a decoration is typeset once to check it when lowering and once to draw it,
    /// in the scene and as text, however often a line is measured and placed.
    #[test]
    fn decorations_typeset_each_formula_once_per_render() {
        let src = "@startuml\nheader <latex>a_1</latex> and <math>x^2</math>\nfooter <latex>b_2</latex>\ncaption <latex>c_3</latex> <latex>c_3</latex>\nlegend\n* <latex>d_4</latex>\nend legend\nA -> B\n@enduml\n";
        crate::equation::count::reset();
        let lowered = crate::diagram::lower(src);
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        assert_eq!(crate::equation::count::get(), 6, "lowering checks each");
        crate::equation::count::reset();
        let scene = lowered.diagram.scene(&Approx, &Theme::LIGHT).unwrap();
        assert_eq!(crate::equation::count::get(), 5, "drawing sets each once");
        assert!(!scene.elements.is_empty());
        crate::equation::count::reset();
        let options = crate::ascii::Options::UNICODE;
        let text = lowered.diagram.ascii(&options).unwrap();
        assert_eq!(crate::equation::count::get(), 5, "text sets each once");
        assert!(!text.contains("latex>"), "{text}");
    }
}
