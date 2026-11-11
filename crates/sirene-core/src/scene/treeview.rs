//! Tree view to scene (MTREEV-4), as Mermaid draws it.

use super::shapes::{self, BoxF};
use super::{
    Color, Element, Owner, Primitive, Pt, Scene, Segment, Stroke, TextMetrics, TextStyle, Theme,
};
use crate::treeview::{Config, Entry, TreeView};

const FONT: f32 = 16.0;
/// The height of a label's box, as a browser measures 16px text.
const LABEL_H: f32 = FONT * 1.15;
const ICON_SIZE: f32 = 14.0;
const ICON_GAP: f32 = 4.0;
const DESC_GAP: f32 = 16.0;
/// Mermaid's `iconColor`, `descriptionColor`, and `highlightStroke`.
const ICON: Color = Color::rgb(0x546e7a);
const DESCRIPTION: Color = Color::rgb(0x6a9955);
const HIGHLIGHT: Color = Color::rgb(0xffc107);

/// A drawn row: its box, label's right edge, and center line.
struct Row<'a> {
    entry: &'a Entry,
    x: f32,
    y: f32,
    label_right: f32,
    center: f32,
}

struct Draw<'a, M: TextMetrics> {
    metrics: &'a M,
    config: Config,
    color: Color,
    /// Where x = 0 is drawn: half a line in, so the root's line is not cut off.
    left: f32,
    height: f32,
    width: f32,
    rows: Vec<Row<'a>>,
    lines: Vec<Element>,
    labels: Vec<Element>,
}

fn el(primitive: Primitive) -> Element {
    Element {
        owner: Owner::Diagram,
        primitive,
    }
}

impl<'a, M: TextMetrics> Draw<'a, M> {
    fn line(&mut self, a: Pt, b: Pt) {
        let stroke = Stroke {
            width: self.config.line_thickness,
            color: self.color,
            dash: None,
        };
        self.lines.push(el(Primitive::Path {
            segments: vec![
                Segment::Move(Pt::new(a.x + self.left, a.y)),
                Segment::Line(Pt::new(b.x + self.left, b.y)),
            ],
            fill: None,
            stroke: Some(stroke),
        }));
    }

    /// Text starting at `x`, centered on `y`.
    fn text(&mut self, text: &str, x: f32, y: f32, color: Color, style: TextStyle) -> f32 {
        let w = self.metrics.styled_width(text, FONT, style);
        if !text.is_empty() {
            self.labels.push(el(Primitive::Text {
                text: text.to_owned(),
                center: Pt::new(x + self.left + w / 2.0, y),
                size: FONT,
                color,
                style,
            }));
        }
        w
    }

    /// Draws `entry` at `depth` and its children below it; returns its row's index.
    fn node(&mut self, entry: &'a Entry, depth: usize) -> usize {
        let c = self.config;
        let indent = depth as f32 * (c.row_indent + c.padding_x);
        let y = self.height;
        let h = LABEL_H + c.padding_y * 2.0;
        let icon = entry.shown_icon(c.show_icons);
        let offset = if icon.is_some() {
            ICON_SIZE + ICON_GAP
        } else {
            0.0
        };
        if let Some(name) = icon {
            self.icon(name, indent + c.padding_x, y + c.padding_y);
        }
        let style = TextStyle {
            bold: entry.directory,
            italic: false,
        };
        let label_x = indent + c.padding_x + offset;
        let w = self.text(&entry.name, label_x, y + h / 2.0, self.color, style);
        if depth > 0 {
            self.line(
                Pt::new(indent - c.row_indent, y + h / 2.0),
                Pt::new(indent, y + h / 2.0),
            );
        }
        self.width = self.width.max(indent + w + c.padding_x * 2.0 + offset);
        self.height += h;
        let row = self.rows.len();
        self.rows.push(Row {
            entry,
            x: indent,
            y,
            label_right: label_x + w,
            center: y + h / 2.0,
        });
        let mut last = None;
        for child in &entry.children {
            last = Some(self.node(child, depth + 1));
        }
        if let Some(l) = last {
            let end = self.rows[l].center + c.line_thickness / 2.0;
            self.line(
                Pt::new(indent + c.padding_x, y + h),
                Pt::new(indent + c.padding_x, end),
            );
        }
        row
    }

    /// A 14px icon with its top left at `(x, y)`: the built-in `file` and `folder`, or a `?`
    /// for an icon from a pack, which sirene does not have.
    fn icon(&mut self, name: &str, x: f32, y: f32) {
        let (x, s) = (x + self.left, ICON_SIZE);
        let p = |dx: f32, dy: f32| (x + dx * s, y + dy * s);
        match name {
            "file" => {
                let pts = [
                    p(0.2, 0.08),
                    p(0.6, 0.08),
                    p(0.82, 0.3),
                    p(0.82, 0.92),
                    p(0.2, 0.92),
                ];
                self.labels.push(el(Primitive::Path {
                    segments: shapes::polygon(&pts),
                    fill: None,
                    stroke: Some(Stroke {
                        width: 1.2,
                        color: ICON,
                        dash: None,
                    }),
                }));
            }
            "folder" => {
                let pts = [
                    p(0.06, 0.2),
                    p(0.4, 0.2),
                    p(0.5, 0.32),
                    p(0.94, 0.32),
                    p(0.94, 0.84),
                    p(0.06, 0.84),
                ];
                self.labels.push(el(Primitive::Path {
                    segments: shapes::polygon(&pts),
                    fill: Some(ICON),
                    stroke: None,
                }));
            }
            _ => self.labels.push(el(Primitive::Text {
                text: "?".to_owned(),
                center: Pt::new(x + s / 2.0, y + s / 2.0),
                size: s,
                color: ICON,
                style: TextStyle {
                    bold: true,
                    italic: false,
                },
            })),
        }
    }
}

/// Draws `view` (MTREEV-4).
pub fn treeview(view: &TreeView, metrics: &impl TextMetrics, theme: &Theme) -> Scene {
    let root = Entry {
        name: "/".to_owned(),
        directory: true,
        children: view.entries.clone(),
        ..Entry::default()
    };
    let mut d = Draw {
        metrics,
        config: view.config,
        color: theme.text,
        left: view.config.line_thickness / 2.0,
        height: 0.0,
        width: 0.0,
        rows: Vec::new(),
        lines: Vec::new(),
        labels: Vec::new(),
    };
    d.node(&root, 0);
    // Descriptions line up right of the widest label.
    let right = d.rows.iter().map(|r| r.label_right).fold(0.0, f32::max);
    let desc_x = right + DESC_GAP;
    let descs: Vec<(String, f32)> = d
        .rows
        .iter()
        .filter_map(|r| Some((r.entry.description.clone()?, r.center)))
        .collect();
    let italic = TextStyle {
        bold: false,
        italic: true,
    };
    for (text, y) in descs {
        let w = d.text(&text, desc_x, y, DESCRIPTION, italic);
        d.width = d.width.max(desc_x + w + view.config.padding_x);
    }
    // Highlights span to the right edge, behind everything.
    let mut highlights = Vec::new();
    let h = LABEL_H + view.config.padding_y * 2.0;
    for r in d.rows.iter().filter(|r| r.entry.highlighted()) {
        let w = d.width - r.x + 8.0;
        let b = BoxF {
            x: r.x + d.left,
            y: r.y + 1.0,
            w,
            h: h - 2.0,
        };
        highlights.push(el(Primitive::Path {
            segments: shapes::rounded(b, 3.0),
            fill: Some(HIGHLIGHT.with_alpha(38)),
            stroke: Some(Stroke {
                width: 1.0,
                color: HIGHLIGHT,
                dash: None,
            }),
        }));
        d.width = d.width.max(r.x + w + 2.0);
    }
    let mut elements = highlights;
    elements.append(&mut d.lines);
    elements.append(&mut d.labels);
    Scene {
        width: d.width,
        height: d.height,
        background: theme.background,
        elements,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Approx;

    fn draw(src: &str) -> Scene {
        let (view, d) = crate::treeview::parse(src).lower();
        assert!(d.is_empty(), "{d:?}");
        treeview(&view, &Approx, &Theme::LIGHT)
    }

    fn texts(s: &Scene) -> Vec<(String, Pt, TextStyle)> {
        s.elements
            .iter()
            .filter_map(|e| match &e.primitive {
                Primitive::Text {
                    text,
                    center,
                    style,
                    ..
                } => Some((text.clone(), *center, *style)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn rows() {
        let s = draw("treeView-beta\n  src/\n    a.ts ## entry\n  README.md\n");
        let t = texts(&s);
        let names: Vec<&str> = t.iter().map(|(s, ..)| s.as_str()).collect();
        assert_eq!(names, ["/", "src", "a.ts", "README.md", "entry"]);
        let h = LABEL_H + 10.0;
        assert!((s.height - 4.0 * h).abs() < 0.01);
        // Each depth is 15 further in, and rows stack down.
        let left = |i: usize| t[i].1.x - Approx.styled_width(&t[i].0, FONT, t[i].2) / 2.0;
        assert!((left(1) - left(0) - 15.0).abs() < 0.01);
        assert!((left(2) - left(1) - 15.0).abs() < 0.01);
        assert!((t[2].1.y - 2.5 * h).abs() < 0.01);
        assert!(t[1].2.bold && !t[2].2.bold);
        // The description is italic, right of the widest label.
        assert!(t[4].2.italic && left(4) > left(2) + Approx.width("a.ts", FONT));
    }

    #[test]
    fn highlight_and_icons() {
        let s = draw("treeView-beta\n  a :::highlight\n  b icon(file)\n  c icon(x:y)\n");
        let fills: Vec<Option<Color>> = s
            .elements
            .iter()
            .filter_map(|e| match &e.primitive {
                Primitive::Path { fill, .. } => Some(*fill),
                _ => None,
            })
            .collect();
        assert_eq!(fills[0], Some(HIGHLIGHT.with_alpha(38)));
        assert!(texts(&s).iter().any(|(t, ..)| t == "?"));
    }
}
