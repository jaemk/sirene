//! A Visio page lowered to the display list (VISIO-5 to VISIO-10): the transform stack, geometry,
//! line and fill, text, groups, masters, layers, and foreign placeholders.

use std::collections::{BTreeMap, HashSet};

use super::geom::{self, Geom, Op};
use super::limits;
use super::model::{Cell, Cells, Document, Page, Row, Section, Shape, ShapeType, TextPart, Theme};
use super::{PX_PER_INCH, Visio, theme_for};
use crate::scene::{
    Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle,
};
use crate::{Error, Result};

const PX: f64 = PX_PER_INCH as f64;

/// The 24 classic indexed colors (VISIO-8).
pub(super) const PALETTE: [[u8; 3]; 24] = [
    [0x00, 0x00, 0x00],
    [0xFF, 0xFF, 0xFF],
    [0xFF, 0x00, 0x00],
    [0x00, 0xFF, 0x00],
    [0x00, 0x00, 0xFF],
    [0xFF, 0xFF, 0x00],
    [0xFF, 0x00, 0xFF],
    [0x00, 0xFF, 0xFF],
    [0x80, 0x00, 0x00],
    [0x00, 0x80, 0x00],
    [0x00, 0x00, 0x80],
    [0x80, 0x80, 0x00],
    [0x80, 0x00, 0x80],
    [0x00, 0x80, 0x80],
    [0xC0, 0xC0, 0xC0],
    [0xE6, 0xE6, 0xE6],
    [0xCD, 0xCD, 0xCD],
    [0xB3, 0xB3, 0xB3],
    [0x9A, 0x9A, 0x9A],
    [0x80, 0x80, 0x80],
    [0x66, 0x66, 0x66],
    [0x4D, 0x4D, 0x4D],
    [0x33, 0x33, 0x33],
    [0x1A, 0x1A, 0x1A],
];

/// A page drawn: the scene and the warnings found drawing it.
pub(crate) struct Drawn {
    pub scene: Scene,
    pub warnings: Vec<String>,
}

/// An affine map `(x, y) -> (a x + c y + e, b x + d y + f)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Mat {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Mat {
    /// `self` applied after `inner`.
    fn after(&self, inner: &Mat) -> Mat {
        Mat {
            a: self.a * inner.a + self.c * inner.b,
            b: self.b * inner.a + self.d * inner.b,
            c: self.a * inner.c + self.c * inner.d,
            d: self.b * inner.c + self.d * inner.d,
            e: self.a * inner.e + self.c * inner.f + self.e,
            f: self.b * inner.e + self.d * inner.f + self.f,
        }
    }

    fn apply(&self, p: (f64, f64)) -> (f64, f64) {
        (
            self.a * p.0 + self.c * p.1 + self.e,
            self.b * p.0 + self.d * p.1 + self.f,
        )
    }

    fn pt(&self, p: (f64, f64)) -> Pt {
        let (x, y) = self.apply(p);
        Pt::new(x as f32, y as f32)
    }

    /// The shape-to-parent transform of VISIO-5.
    pub(super) fn shape(t: &Transform) -> Mat {
        let (sin, cos) = t.angle.sin_cos();
        let (sx, tx) = if t.flip_x { (-1.0, t.w) } else { (1.0, 0.0) };
        let (sy, ty) = if t.flip_y { (-1.0, t.h) } else { (1.0, 0.0) };
        let (ox, oy) = (tx - t.loc_x, ty - t.loc_y);
        Mat {
            a: cos * sx,
            b: sin * sx,
            c: -sin * sy,
            d: cos * sy,
            e: t.pin_x + cos * ox - sin * oy,
            f: t.pin_y + sin * ox + cos * oy,
        }
    }
}

/// A shape's transform cells (VISIO-5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Transform {
    pub pin_x: f64,
    pub pin_y: f64,
    pub w: f64,
    pub h: f64,
    pub loc_x: f64,
    pub loc_y: f64,
    pub angle: f64,
    pub flip_x: bool,
    pub flip_y: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum StyleKind {
    Line,
    Fill,
    Text,
}

fn style_kind(name: &str) -> StyleKind {
    match name {
        "LineWeight" | "LineColor" | "LinePattern" | "LineColorTrans" | "LineCap" | "Rounding"
        | "BeginArrow" | "EndArrow" | "BeginArrowSize" | "EndArrowSize" => StyleKind::Line,
        "FillForegnd"
        | "FillBkgnd"
        | "FillPattern"
        | "FillForegndTrans"
        | "FillBkgndTrans"
        | "FillGradientEnabled" => StyleKind::Fill,
        _ => StyleKind::Text,
    }
}

fn usable(c: &Cell) -> bool {
    !c.v.trim().is_empty()
}

/// A shape with the master shape it inherits from (VISIO-6).
struct Ctx<'a> {
    doc: &'a Document,
    shape: &'a Shape,
    master: Option<&'a Shape>,
    /// The master the children of this shape inherit through.
    master_id: Option<u32>,
}

fn find_shape(shapes: &[Shape], id: u32, depth: usize) -> Option<&Shape> {
    if depth > limits::MAX_GROUP_DEPTH {
        return None;
    }
    shapes.iter().find(|s| s.id == id).or_else(|| {
        shapes
            .iter()
            .find_map(|s| find_shape(&s.children, id, depth + 1))
    })
}

impl<'a> Ctx<'a> {
    fn new(doc: &'a Document, shape: &'a Shape, inherited: Option<u32>) -> Self {
        let master_id = shape.master.or(inherited);
        let master =
            master_id
                .and_then(|id| doc.masters.get(&id))
                .and_then(|m| match shape.master_shape {
                    Some(ms) => find_shape(&m.shapes, ms, 0),
                    None if shape.master.is_some() && m.shapes.len() == 1 => m.shapes.first(),
                    None => None,
                });
        Ctx {
            doc,
            shape,
            master,
            master_id,
        }
    }

    fn style_id(&self, kind: StyleKind) -> Option<u32> {
        let pick = |s: &Shape| match kind {
            StyleKind::Line => s.line_style,
            StyleKind::Fill => s.fill_style,
            StyleKind::Text => s.text_style,
        };
        pick(self.shape).or_else(|| self.master.and_then(pick))
    }

    /// The style sheets of a kind, the shape's own first and then each one's parent.
    fn styles(&self, kind: StyleKind) -> Vec<&'a super::model::StyleSheet> {
        let mut out = Vec::new();
        let mut id = self.style_id(kind);
        while let Some(i) = id {
            if out.len() >= limits::MAX_STYLE_DEPTH {
                break;
            }
            let Some(s) = self.doc.styles.get(&i) else {
                break;
            };
            out.push(s);
            id = match kind {
                StyleKind::Line => s.line_style,
                StyleKind::Fill => s.fill_style,
                StyleKind::Text => s.text_style,
            };
        }
        out
    }

    /// The cell `name`: the shape's, the master shape's, then the style sheet chain (VISIO-6).
    fn cell(&self, name: &str) -> Option<&'a Cell> {
        self.shape
            .cells
            .get(name)
            .filter(|c| usable(c))
            .or_else(|| {
                self.master
                    .and_then(|m| m.cells.get(name))
                    .filter(|c| usable(c))
            })
            .or_else(|| {
                self.styles(style_kind(name))
                    .into_iter()
                    .find_map(|s| s.sheet.cells.get(name).filter(|c| usable(c)))
            })
    }

    fn num(&self, name: &str) -> Option<f64> {
        let v: f64 = self.cell(name)?.v.trim().parse().ok()?;
        v.is_finite().then_some(v)
    }

    /// The sections called `name`, merged shape over master shape over style, one per `IX`.
    fn sections(&self, name: &str) -> Vec<Section> {
        let mut layers: Vec<&'a [Section]> = vec![&self.shape.sections];
        if let Some(m) = self.master {
            layers.push(&m.sections);
        }
        if name == "Character" || name == "Paragraph" {
            layers.extend(
                self.styles(StyleKind::Text)
                    .into_iter()
                    .map(|s| &s.sheet.sections[..]),
            );
        }
        let mut merged: BTreeMap<i32, (Cells, BTreeMap<i32, Row>)> = BTreeMap::new();
        for layer in layers.iter().rev() {
            for s in layer.iter().filter(|s| s.n == name) {
                let (cells, rows) = merged.entry(s.ix).or_default();
                cells.extend(s.cells.iter().map(|(k, v)| (k.clone(), v.clone())));
                for r in &s.rows {
                    if r.del {
                        rows.remove(&r.ix);
                        continue;
                    }
                    let e = rows.entry(r.ix).or_insert_with(|| Row {
                        cells: Cells::new(),
                        ..r.clone()
                    });
                    e.t.clone_from(&r.t);
                    for (k, v) in &r.cells {
                        if usable(v) || !e.cells.contains_key(k) {
                            e.cells.insert(k.clone(), v.clone());
                        }
                    }
                }
            }
        }
        merged
            .into_iter()
            .map(|(ix, (cells, rows))| Section {
                n: name.to_owned(),
                ix,
                cells,
                rows: rows.into_values().collect(),
            })
            .collect()
    }

    /// How many rows the sections called `name` have, the shape's and its master shape's, before
    /// they are merged.
    fn row_estimate(&self, name: &str) -> usize {
        let count = |sections: &[Section]| {
            sections
                .iter()
                .filter(|s| s.n == name)
                .map(|s| s.rows.len())
                .sum::<usize>()
        };
        count(&self.shape.sections) + self.master.map_or(0, |m| count(&m.sections))
    }

    fn has_own_geometry(&self) -> bool {
        self.shape.sections.iter().any(|s| s.n == "Geometry")
    }
}

fn parse_index_color(doc: &Document, v: f64) -> Option<[u8; 3]> {
    if !(0.0..=1e6).contains(&v) || v.fract() != 0.0 {
        return None;
    }
    let i = v as usize;
    PALETTE
        .get(i)
        .copied()
        .or_else(|| doc.palette.get(&(i as u32)).copied())
        .or(Some([0, 0, 0]))
}

/// A color cell's value (VISIO-8): `#RRGGBB`, or an index into the palette; a `THEMEVAL` cell
/// with no usable value reads its `QuickStyle` color from the theme.
fn color_of(ctx: &Ctx, theme: Option<&Theme>, cell: &Cell, quick: &str) -> Option<[u8; 3]> {
    let v = cell.v.trim();
    if let Some(c) = super::model::hex(v).filter(|_| v.starts_with('#')) {
        return Some(c);
    }
    if let Ok(n) = v.parse::<f64>()
        && let Some(c) = parse_index_color(ctx.doc, n)
    {
        return Some(c);
    }
    if cell
        .f
        .as_deref()
        .is_some_and(|f| f.trim_start().starts_with("THEMEVAL("))
    {
        let slot = match ctx.num(quick)? as i64 {
            0 => "dk1",
            1 => "accent1",
            2 => "accent2",
            3 => "accent3",
            4 => "accent4",
            5 => "accent5",
            6 => "accent6",
            7 => "lt1",
            8 => "dk2",
            9 => "lt2",
            _ => return None,
        };
        return theme?.colors.get(slot).copied();
    }
    None
}

fn rgba(rgb: [u8; 3], transparency: f64) -> Color {
    let alpha = ((1.0 - transparency.clamp(0.0, 1.0)) * 255.0).round() as u8;
    Color {
        r: rgb[0],
        g: rgb[1],
        b: rgb[2],
        a: alpha,
    }
}

/// Line and fill of a shape.
struct Look {
    fill: Option<Color>,
    stroke: Option<Stroke>,
    begin_arrow: bool,
    end_arrow: bool,
    begin_size: f64,
    end_size: f64,
}

#[derive(Clone, Copy)]
struct CharFmt {
    size: f64,
    color: Color,
    bold: bool,
    italic: bool,
    underline: bool,
}

#[derive(Clone, Copy)]
struct ParaFmt {
    align: u8,
    ind_left: f64,
    ind_right: f64,
    ind_first: f64,
    sp_line: f64,
    sp_before: f64,
    sp_after: f64,
}

struct Run {
    text: String,
    fmt: CharFmt,
}

struct Para {
    fmt: ParaFmt,
    runs: Vec<Run>,
}

struct Seg {
    text: String,
    fmt: CharFmt,
    width: f64,
}

struct Line {
    segs: Vec<Seg>,
    width: f64,
    height: f64,
}

struct Drawer<'a, M: TextMetrics> {
    doc: &'a Document,
    metrics: &'a M,
    theme: Option<&'a Theme>,
    root: Mat,
    layers: Vec<bool>,
    elements: Vec<Element>,
    warnings: Vec<String>,
    seen: HashSet<String>,
    count: usize,
    /// Geometry rows and text bytes spent on this page, against the per-page budgets.
    rows: usize,
    text_bytes: usize,
    page_w: f64,
    page_h: f64,
}

fn finite(v: f64) -> bool {
    v.is_finite() && v.abs() <= limits::MAX_COORD
}

impl<M: TextMetrics> Drawer<'_, M> {
    fn warn(&mut self, message: String) {
        if self.warnings.len() < 100 && self.seen.insert(message.clone()) {
            self.warnings.push(message);
        }
    }

    fn push(&mut self, primitive: Primitive) {
        self.elements.push(Element {
            owner: Owner::Diagram,
            primitive,
        });
    }

    fn transform(&self, ctx: &Ctx) -> Option<Transform> {
        let n = |name: &str| ctx.num(name);
        let w = n("Width").unwrap_or(0.0);
        let h = n("Height").unwrap_or(0.0);
        let t = Transform {
            pin_x: n("PinX").unwrap_or(0.0),
            pin_y: n("PinY").unwrap_or(0.0),
            w,
            h,
            loc_x: n("LocPinX").unwrap_or(w / 2.0),
            loc_y: n("LocPinY").unwrap_or(h / 2.0),
            angle: n("Angle").unwrap_or(0.0),
            flip_x: n("FlipX").is_some_and(|v| v != 0.0),
            flip_y: n("FlipY").is_some_and(|v| v != 0.0),
        };
        [t.pin_x, t.pin_y, t.w, t.h, t.loc_x, t.loc_y, t.angle]
            .iter()
            .all(|v| finite(*v))
            .then_some(t)
    }

    fn look(&mut self, ctx: &Ctx) -> Look {
        let pattern = ctx.num("FillPattern").unwrap_or(1.0);
        let fill = if pattern <= 0.0 {
            None
        } else {
            if pattern > 1.0 || ctx.num("FillGradientEnabled").is_some_and(|g| g != 0.0) {
                self.warn("fill patterns and gradients are drawn as a solid color".into());
            }
            let rgb = ctx
                .cell("FillForegnd")
                .and_then(|c| color_of(ctx, self.theme, c, "QuickStyleFillColor"))
                .unwrap_or(PALETTE[1]);
            Some(rgba(rgb, ctx.num("FillForegndTrans").unwrap_or(0.0)))
        };
        let line_pattern = ctx.num("LinePattern").unwrap_or(1.0);
        let weight = ctx.num("LineWeight").unwrap_or(0.01).max(0.0);
        let width = (weight * PX).max(0.5) as f32;
        let stroke = (line_pattern > 0.0).then(|| {
            let rgb = ctx
                .cell("LineColor")
                .and_then(|c| color_of(ctx, self.theme, c, "QuickStyleLineColor"))
                .unwrap_or(PALETTE[0]);
            let w = width.max(1.0);
            let dash = match line_pattern as i64 {
                1 => None,
                2 => Some((4.0 * w, 2.0 * w)),
                3 => Some((w, 2.0 * w)),
                _ => Some((3.0 * w, 2.0 * w)),
            };
            Stroke {
                width,
                color: rgba(rgb, ctx.num("LineColorTrans").unwrap_or(0.0)),
                dash,
            }
        });
        Look {
            fill,
            stroke,
            begin_arrow: ctx.num("BeginArrow").is_some_and(|a| a > 0.0),
            end_arrow: ctx.num("EndArrow").is_some_and(|a| a > 0.0),
            begin_size: ctx.num("BeginArrowSize").unwrap_or(2.0).clamp(0.0, 6.0),
            end_size: ctx.num("EndArrowSize").unwrap_or(2.0).clamp(0.0, 6.0),
        }
    }

    fn visible(&self, ctx: &Ctx) -> bool {
        let Some(members) = ctx.cell("LayerMember") else {
            return true;
        };
        let ids: Vec<usize> = members
            .v
            .split(';')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        ids.is_empty()
            || ids
                .iter()
                .any(|i| self.layers.get(*i).copied().unwrap_or(true))
    }

    fn shape(
        &mut self,
        parent: &Mat,
        shape: &Shape,
        inherited: Option<u32>,
        depth: usize,
    ) -> Result<()> {
        self.count += 1;
        if self.count > limits::MAX_SHAPES || depth > limits::MAX_GROUP_DEPTH {
            return Err(Error::new(format!(
                "the page draws more than {} shapes",
                limits::MAX_SHAPES
            )));
        }
        let ctx = Ctx::new(self.doc, shape, inherited);
        if shape.kind == ShapeType::Other {
            self.warn("shapes of an unsupported Type are not drawn".into());
            return Ok(());
        }
        if !self.visible(&ctx) {
            return Ok(());
        }
        let Some(t) = self.transform(&ctx) else {
            self.warn(format!(
                "shape {}: its transform is not a usable number; skipped",
                shape.id
            ));
            return Ok(());
        };
        let total = parent.after(&Mat::shape(&t));
        let look = self.look(&ctx);
        self.geometry(&ctx, &total, &t, &look);
        if shape.kind == ShapeType::Foreign {
            self.placeholder(&ctx, &total, &t);
        }
        for child in &shape.children {
            self.shape(&total, child, ctx.master_id, depth + 1)?;
        }
        self.text(&ctx, &total, &t);
        Ok(())
    }

    fn geometry(&mut self, ctx: &Ctx, total: &Mat, t: &Transform, look: &Look) {
        self.rows = self.rows.saturating_add(ctx.row_estimate("Geometry"));
        if self.rows > limits::MAX_PAGE_ROWS {
            self.warn(format!(
                "the page has more than {} geometry rows; the shapes after them are not drawn",
                limits::MAX_PAGE_ROWS
            ));
            return;
        }
        let mut sections = ctx.sections("Geometry");
        if sections.iter().map(|s| s.rows.len()).sum::<usize>() > limits::MAX_ROWS {
            self.warn(format!(
                "shape {}: too many geometry rows; skipped",
                ctx.shape.id
            ));
            return;
        }
        // Geometry cached for the master's size, on an instance of another size (VISIO-6).
        let mut m = *total;
        if !ctx.has_own_geometry()
            && let Some(ms) = ctx.master
            && let (Some(mw), Some(mh)) = (
                ms.cells
                    .get("Width")
                    .and_then(|c| c.v.trim().parse::<f64>().ok()),
                ms.cells
                    .get("Height")
                    .and_then(|c| c.v.trim().parse::<f64>().ok()),
            )
            && mw > 1e-9
            && mh > 1e-9
            && ctx.shape.cells.contains_key("Width")
        {
            let scale = Mat {
                a: t.w / mw,
                b: 0.0,
                c: 0.0,
                d: t.h / mh,
                e: 0.0,
                f: 0.0,
            };
            m = total.after(&scale);
        }
        let mut begin: Option<Vec<Pt>> = None;
        let mut end: Option<Vec<Pt>> = None;
        let mut warned = Vec::new();
        let geoms: Vec<Geom> = sections
            .drain(..)
            .map(|s| geom::build(&s, t.w, t.h, &mut |msg| warned.push(msg.to_owned())))
            .collect();
        for w in warned {
            self.warn(format!("shape {}: {w}", ctx.shape.id));
        }
        for g in geoms.iter().filter(|g| !g.no_show) {
            for sub in &g.subs {
                let mut segments = Vec::with_capacity(sub.ops.len() + 1);
                let mut points = Vec::new();
                for op in &sub.ops {
                    match *op {
                        Op::Move(p) => {
                            segments.push(Segment::Move(m.pt(p)));
                            points.push(m.pt(p));
                        }
                        Op::Line(p) => {
                            segments.push(Segment::Line(m.pt(p)));
                            points.push(m.pt(p));
                        }
                        Op::Cubic(c1, c2, p) => {
                            segments.push(Segment::Cubic {
                                c1: m.pt(c1),
                                c2: m.pt(c2),
                                to: m.pt(p),
                            });
                            points.push(m.pt(c1));
                            points.push(m.pt(c2));
                            points.push(m.pt(p));
                        }
                    }
                }
                let fill = look.fill.filter(|_| !g.no_fill);
                let stroke = look.stroke.filter(|_| !g.no_line);
                if sub.closed {
                    segments.push(Segment::Close);
                }
                if !sub.closed && fill.is_some() && stroke.is_some() {
                    let mut closed = segments.clone();
                    closed.push(Segment::Close);
                    self.push(Primitive::Path {
                        segments: closed,
                        fill,
                        stroke: None,
                        clip: None,
                    });
                    self.push(Primitive::Path {
                        segments,
                        fill: None,
                        stroke,
                        clip: None,
                    });
                } else if fill.is_some() || stroke.is_some() {
                    self.push(Primitive::Path {
                        segments,
                        fill,
                        stroke,
                        clip: None,
                    });
                }
                if !sub.closed && stroke.is_some() {
                    if begin.is_none() {
                        begin = Some(points.clone());
                    }
                    end = Some(points);
                }
            }
        }
        if let (Some(stroke), true, Some(points)) = (look.stroke, look.begin_arrow, begin) {
            self.arrow(&points, stroke, look.begin_size);
        }
        if let (Some(stroke), true, Some(mut points)) = (look.stroke, look.end_arrow, end) {
            points.reverse();
            self.arrow(&points, stroke, look.end_size);
        }
    }

    /// A filled triangle with its tip at the first of `points`, pointing away from the rest.
    fn arrow(&mut self, points: &[Pt], stroke: Stroke, size: f64) {
        let Some(&tip) = points.first() else { return };
        let Some(&next) = points
            .iter()
            .skip(1)
            .find(|p| (p.x - tip.x).hypot(p.y - tip.y) > 0.01)
        else {
            return;
        };
        let len = f64::from((next.x - tip.x).hypot(next.y - tip.y));
        let (ux, uy) = (
            f64::from(next.x - tip.x) / len,
            f64::from(next.y - tip.y) / len,
        );
        let s = (0.06 + 0.02 * size) * PX + f64::from(stroke.width);
        let half = s * 0.4;
        let base = (f64::from(tip.x) + ux * s, f64::from(tip.y) + uy * s);
        let (px, py) = (-uy * half, ux * half);
        let pt = |x: f64, y: f64| Pt::new(x as f32, y as f32);
        self.push(Primitive::Path {
            segments: vec![
                Segment::Move(tip),
                Segment::Line(pt(base.0 + px, base.1 + py)),
                Segment::Line(pt(base.0 - px, base.1 - py)),
                Segment::Close,
            ],
            fill: Some(stroke.color),
            stroke: None,
            clip: None,
        });
    }

    /// A foreign shape's box and diagonals (VISIO-10).
    fn placeholder(&mut self, ctx: &Ctx, total: &Mat, t: &Transform) {
        let kind = ctx
            .shape
            .foreign
            .clone()
            .unwrap_or_else(|| "Object".to_owned());
        self.warn(format!(
            "{kind} foreign shapes are drawn as a placeholder box"
        ));
        let c = [(0.0, 0.0), (t.w, 0.0), (t.w, t.h), (0.0, t.h)].map(|p| total.pt(p));
        let gray = Color::rgb(0x888888);
        let stroke = Some(Stroke {
            width: 1.0,
            color: gray,
            dash: None,
        });
        self.push(Primitive::Path {
            segments: vec![
                Segment::Move(c[0]),
                Segment::Line(c[1]),
                Segment::Line(c[2]),
                Segment::Line(c[3]),
                Segment::Close,
                Segment::Move(c[0]),
                Segment::Line(c[2]),
                Segment::Move(c[1]),
                Segment::Line(c[3]),
            ],
            fill: None,
            stroke,
            clip: None,
        });
    }

    fn char_fmt(&self, ctx: &Ctx, rows: &BTreeMap<i32, Row>, ix: i32) -> CharFmt {
        let cell = |name: &str| -> Option<&Cell> {
            rows.get(&ix)
                .and_then(|r| r.cells.get(name))
                .filter(|c| usable(c))
                .or_else(|| {
                    rows.get(&0)
                        .and_then(|r| r.cells.get(name))
                        .filter(|c| usable(c))
                })
        };
        let size = cell("Size")
            .and_then(|c| c.v.trim().parse::<f64>().ok())
            .filter(|s| s.is_finite() && *s > 0.0)
            .unwrap_or(0.1111)
            .min(10.0);
        let rgb = cell("Color")
            .and_then(|c| color_of(ctx, self.theme, c, "QuickStyleFontColor"))
            .unwrap_or(PALETTE[0]);
        let style = cell("Style")
            .and_then(|c| c.v.trim().parse::<f64>().ok())
            .filter(|s| s.is_finite() && *s >= 0.0)
            .map_or(0, |s| s as u32);
        CharFmt {
            size: size * PX,
            color: rgba(rgb, 0.0),
            bold: style & 1 != 0,
            italic: style & 2 != 0,
            underline: style & 4 != 0,
        }
    }

    fn para_fmt(rows: &BTreeMap<i32, Row>, ix: i32) -> ParaFmt {
        let get = |name: &str| -> Option<f64> {
            rows.get(&ix)
                .and_then(|r| r.cells.get(name))
                .filter(|c| usable(c))
                .or_else(|| {
                    rows.get(&0)
                        .and_then(|r| r.cells.get(name))
                        .filter(|c| usable(c))
                })
                .and_then(|c| c.v.trim().parse::<f64>().ok())
                .filter(|v| v.is_finite())
        };
        ParaFmt {
            align: get("HorzAlign").map_or(1, |a| a as u8),
            ind_left: get("IndLeft").unwrap_or(0.0) * PX,
            ind_right: get("IndRight").unwrap_or(0.0) * PX,
            ind_first: get("IndFirst").unwrap_or(0.0) * PX,
            sp_line: get("SpLine").unwrap_or(-1.2),
            sp_before: get("SpBefore").unwrap_or(0.0) * PX,
            sp_after: get("SpAfter").unwrap_or(0.0) * PX,
        }
    }

    fn style_of(fmt: CharFmt) -> TextStyle {
        TextStyle {
            bold: fmt.bold,
            italic: fmt.italic,
        }
    }

    fn measure(&self, text: &str, fmt: CharFmt) -> f64 {
        f64::from(
            self.metrics
                .styled_width(text, fmt.size as f32, Self::style_of(fmt)),
        )
    }

    fn text(&mut self, ctx: &Ctx, total: &Mat, t: &Transform) {
        let parts = ctx
            .shape
            .text
            .as_ref()
            .or_else(|| ctx.master.and_then(|m| m.text.as_ref()));
        let Some(parts) = parts else { return };
        if !parts
            .iter()
            .any(|p| matches!(p, TextPart::Chars(s) if !s.trim().is_empty()))
        {
            return;
        }
        let bytes: usize = parts
            .iter()
            .map(|p| match p {
                TextPart::Chars(s) => s.len(),
                _ => 0,
            })
            .sum();
        self.text_bytes = self.text_bytes.saturating_add(bytes);
        if self.text_bytes > limits::MAX_PAGE_TEXT {
            self.warn(format!(
                "the page has more than {} bytes of text; the shapes after it have none",
                limits::MAX_PAGE_TEXT
            ));
            return;
        }
        let char_rows: BTreeMap<i32, Row> = ctx
            .sections("Character")
            .into_iter()
            .flat_map(|s| s.rows)
            .map(|r| (r.ix, r))
            .collect();
        let para_rows: BTreeMap<i32, Row> = ctx
            .sections("Paragraph")
            .into_iter()
            .flat_map(|s| s.rows)
            .map(|r| (r.ix, r))
            .collect();
        let (mut cp, mut pp) = (0, 0);
        let mut paras = vec![Para {
            fmt: Self::para_fmt(&para_rows, 0),
            runs: Vec::new(),
        }];
        for part in parts {
            match part {
                TextPart::Cp(i) => cp = *i,
                TextPart::Pp(i) => {
                    pp = *i;
                    if let Some(p) = paras.last_mut().filter(|p| p.runs.is_empty()) {
                        p.fmt = Self::para_fmt(&para_rows, pp);
                    }
                }
                TextPart::Tp(_) => {}
                TextPart::Chars(s) => {
                    let fmt = self.char_fmt(ctx, &char_rows, cp);
                    let s = s.replace('\r', "").replace(['\u{2028}', '\u{b}'], "\n");
                    for (i, piece) in s.split('\n').enumerate() {
                        if i > 0 {
                            paras.push(Para {
                                fmt: Self::para_fmt(&para_rows, pp),
                                runs: Vec::new(),
                            });
                        }
                        if !piece.is_empty()
                            && let Some(p) = paras.last_mut()
                        {
                            p.runs.push(Run {
                                text: piece.to_owned(),
                                fmt,
                            });
                        }
                    }
                }
            }
        }
        if t.angle != 0.0 {
            self.warn("text is drawn upright; rotated text is not supported".into());
        }

        // The text block's box in the shape's frame, mapped to the page (VISIO-9).
        let n = |name: &str| ctx.num(name);
        let (tw, th) = (n("TxtWidth").unwrap_or(t.w), n("TxtHeight").unwrap_or(t.h));
        let (pin_x, pin_y) = (
            n("TxtPinX").unwrap_or(t.w / 2.0),
            n("TxtPinY").unwrap_or(t.h / 2.0),
        );
        let (loc_x, loc_y) = (
            n("TxtLocPinX").unwrap_or(tw / 2.0),
            n("TxtLocPinY").unwrap_or(th / 2.0),
        );
        let (x0, y0) = (pin_x - loc_x, pin_y - loc_y);
        let corners =
            [(x0, y0), (x0 + tw, y0), (x0 + tw, y0 + th), (x0, y0 + th)].map(|p| total.apply(p));
        let min = |f: fn(&(f64, f64)) -> f64| corners.iter().map(f).fold(f64::INFINITY, f64::min);
        let max =
            |f: fn(&(f64, f64)) -> f64| corners.iter().map(f).fold(f64::NEG_INFINITY, f64::max);
        let margin = |name: &str| n(name).unwrap_or(0.0556).max(0.0) * PX;
        let left = min(|p| p.0) + margin("LeftMargin");
        let right = max(|p| p.0) - margin("RightMargin");
        let top = min(|p| p.1) + margin("TopMargin");
        let bottom = max(|p| p.1) - margin("BottomMargin");
        let avail = (right - left).max(1.0);

        // Lines.
        let mut laid: Vec<(ParaFmt, Vec<Line>)> = Vec::new();
        for para in &paras {
            let mut lines: Vec<Line> = Vec::new();
            let mut cur = Line {
                segs: Vec::new(),
                width: 0.0,
                height: 0.0,
            };
            let first_width =
                (avail - para.fmt.ind_left - para.fmt.ind_right - para.fmt.ind_first).max(1.0);
            let rest_width = (avail - para.fmt.ind_left - para.fmt.ind_right).max(1.0);
            for run in &para.runs {
                for token in run.text.split_inclusive(' ') {
                    let width = self.measure(token, run.fmt);
                    let bare = self.measure(token.trim_end(), run.fmt);
                    let limit = if lines.is_empty() {
                        first_width
                    } else {
                        rest_width
                    };
                    if !cur.segs.is_empty() && cur.width + bare > limit {
                        lines.push(std::mem::replace(
                            &mut cur,
                            Line {
                                segs: Vec::new(),
                                width: 0.0,
                                height: 0.0,
                            },
                        ));
                    }
                    match cur.segs.last_mut() {
                        Some(last)
                            if last.fmt.size == run.fmt.size
                                && last.fmt.bold == run.fmt.bold
                                && last.fmt.italic == run.fmt.italic
                                && last.fmt.underline == run.fmt.underline
                                && last.fmt.color == run.fmt.color =>
                        {
                            last.text.push_str(token);
                            last.width += width;
                        }
                        _ => cur.segs.push(Seg {
                            text: token.to_owned(),
                            fmt: run.fmt,
                            width,
                        }),
                    }
                    cur.width += width;
                    let lh = if para.fmt.sp_line >= 0.0 {
                        (para.fmt.sp_line * PX).max(run.fmt.size)
                    } else {
                        run.fmt.size * -para.fmt.sp_line
                    };
                    cur.height = cur.height.max(lh);
                }
            }
            if !cur.segs.is_empty() || lines.is_empty() {
                lines.push(cur);
            }
            // Trailing spaces do not count toward a line's width.
            for line in &mut lines {
                if let Some(last) = line.segs.last_mut() {
                    let trimmed = last.text.trim_end();
                    if trimmed.len() != last.text.len() {
                        let w = f64::from(self.metrics.styled_width(
                            trimmed,
                            last.fmt.size as f32,
                            Self::style_of(last.fmt),
                        ));
                        line.width -= last.width - w;
                        last.width = w;
                        last.text = trimmed.to_owned();
                    }
                }
                if line.height == 0.0 {
                    line.height = 12.0;
                }
            }
            laid.push((para.fmt, lines));
        }
        let block: f64 = laid
            .iter()
            .map(|(f, ls)| f.sp_before + f.sp_after + ls.iter().map(|l| l.height).sum::<f64>())
            .sum();
        let valign = n("VerticalAlign").unwrap_or(1.0) as i64;
        let mut y = match valign {
            0 => top,
            2 => bottom - block,
            _ => top + ((bottom - top) - block) / 2.0,
        };
        for (fmt, lines) in &laid {
            y += fmt.sp_before;
            for (i, line) in lines.iter().enumerate() {
                let indent = fmt.ind_left + if i == 0 { fmt.ind_first } else { 0.0 };
                let mut x = match fmt.align {
                    1 => {
                        left + fmt.ind_left
                            + (avail - fmt.ind_left - fmt.ind_right - line.width) / 2.0
                    }
                    2 => right - fmt.ind_right - line.width,
                    _ => left + indent,
                };
                let cy = y + line.height / 2.0;
                for seg in &line.segs {
                    if !seg.text.is_empty() {
                        let center = Pt::new((x + seg.width / 2.0) as f32, cy as f32);
                        self.push(Primitive::Text {
                            text: seg.text.clone(),
                            center,
                            size: seg.fmt.size as f32,
                            color: seg.fmt.color,
                            style: Self::style_of(seg.fmt),
                        });
                        if seg.fmt.underline {
                            let uy = (cy + seg.fmt.size * 0.5) as f32;
                            self.push(Primitive::Path {
                                segments: vec![
                                    Segment::Move(Pt::new(x as f32, uy)),
                                    Segment::Line(Pt::new((x + seg.width) as f32, uy)),
                                ],
                                fill: None,
                                stroke: Some(Stroke {
                                    width: (seg.fmt.size / 16.0).max(1.0) as f32,
                                    color: seg.fmt.color,
                                    dash: None,
                                }),
                                clip: None,
                            });
                        }
                    }
                    x += seg.width;
                }
                y += line.height;
            }
            y += fmt.sp_after;
        }
    }
}

fn page_size(page: &Page) -> Result<(f64, f64)> {
    let get = |name: &str| -> Option<f64> { page.sheet.cells.get(name)?.v.trim().parse().ok() };
    match (get("PageWidth"), get("PageHeight")) {
        (Some(w), Some(h))
            if w.is_finite()
                && h.is_finite()
                && w > 0.0
                && h > 0.0
                && w <= limits::MAX_PAGE_INCHES
                && h <= limits::MAX_PAGE_INCHES =>
        {
            Ok((w, h))
        }
        _ => Err(Error::new(format!(
            "page {:?} has no usable PageWidth and PageHeight (0 to {} inches)",
            page.name,
            limits::MAX_PAGE_INCHES
        ))),
    }
}

fn layer_visibility(page: &Page) -> Vec<bool> {
    let mut out = Vec::new();
    for section in page.sheet.sections.iter().filter(|s| s.n == "Layer") {
        for row in &section.rows {
            let Ok(ix) = usize::try_from(row.ix) else {
                continue;
            };
            if ix > 1024 {
                continue;
            }
            if out.len() <= ix {
                out.resize(ix + 1, true);
            }
            let visible = row.cells.get("Visible").is_none_or(|c| c.v.trim() != "0");
            if let Some(slot) = out.get_mut(ix) {
                *slot = visible;
            }
        }
    }
    out
}

/// Draws the page `visio` selects, with text measured by `metrics` (VISIO-11).
pub(crate) fn draw(visio: &Visio, metrics: &impl TextMetrics) -> Result<Drawn> {
    let doc = visio.document();
    let page = visio
        .current()
        .ok_or_else(|| Error::new("the document has no such page"))?;
    let (w, h) = page_size(page)?;
    let theme_index = page
        .sheet
        .cells
        .get("ThemeIndex")
        .and_then(|c| c.v.trim().parse::<u32>().ok());
    let mut d = Drawer {
        doc,
        metrics,
        theme: theme_for(doc, theme_index),
        root: Mat {
            a: PX,
            b: 0.0,
            c: 0.0,
            d: -PX,
            e: 0.0,
            f: h * PX,
        },
        layers: Vec::new(),
        elements: Vec::new(),
        warnings: Vec::new(),
        seen: HashSet::new(),
        count: 0,
        rows: 0,
        text_bytes: 0,
        page_w: w,
        page_h: h,
    };
    let mut pages: Vec<&Page> = Vec::new();
    if let Some(back) = page
        .back_page
        .and_then(|id| doc.pages.iter().find(|p| p.id == id && p.background))
    {
        pages.push(back);
    }
    pages.push(page);
    for p in pages {
        d.layers = layer_visibility(p);
        let root = d.root;
        for shape in &p.shapes {
            d.shape(&root, shape, None, 0)
                .map_err(|e| e.context(format!("page {:?}", p.name)))?;
        }
    }
    let (page_w, page_h) = (d.page_w, d.page_h);
    Ok(Drawn {
        scene: Scene {
            width: (page_w * PX) as f32,
            height: (page_h * PX) as f32,
            background: Color::rgb(0xffffff),
            elements: d.elements,
        },
        warnings: d.warnings,
    })
}

#[cfg(test)]
mod tests;
