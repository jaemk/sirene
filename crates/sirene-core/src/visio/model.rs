//! The typed model of a `.vsdx` package: pages, masters, style sheets, themes, and the shapes
//! with their cached cell values (VISIO-4). Formulas are kept only as text; nothing evaluates them.

use std::collections::{BTreeMap, HashMap};

use super::limits;
use crate::{Error, Result};

/// One `Cell`: its cached value `V` and its formula `F` (kept for the data formulas of VISIO-7).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cell {
    pub v: String,
    pub f: Option<String>,
}

pub type Cells = BTreeMap<String, Cell>;

/// One `Row` of a section.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Row {
    /// The row type (`MoveTo`, `LineTo`, ...), or empty for a named row.
    pub t: String,
    /// The row's name (`N`), for named rows such as layers and user cells.
    pub n: String,
    pub ix: i32,
    pub del: bool,
    pub cells: Cells,
}

/// One `Section`: `Geometry`, `Character`, `Paragraph`, `Layer`, and so on.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Section {
    pub n: String,
    pub ix: i32,
    pub cells: Cells,
    pub rows: Vec<Row>,
}

/// What a shape is (`Type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShapeType {
    #[default]
    Shape,
    Group,
    Foreign,
    /// Any other `Type` (a guide, say), which is not drawn.
    Other,
}

/// A marker or run in a shape's text (VISIO-9).
#[derive(Debug, Clone, PartialEq)]
pub enum TextPart {
    /// Character data, or a field's cached text.
    Chars(String),
    /// `cp`: the `Character` row of this `IX` applies from here.
    Cp(i32),
    /// `pp`: the `Paragraph` row of this `IX` applies from here.
    Pp(i32),
    /// `tp`: a tab row marker (kept, not used).
    Tp(i32),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Shape {
    pub id: u32,
    pub kind: ShapeType,
    pub master: Option<u32>,
    pub master_shape: Option<u32>,
    pub line_style: Option<u32>,
    pub fill_style: Option<u32>,
    pub text_style: Option<u32>,
    pub cells: Cells,
    pub sections: Vec<Section>,
    pub text: Option<Vec<TextPart>>,
    pub children: Vec<Shape>,
    /// `ForeignData`'s `ForeignType`, for a foreign shape.
    pub foreign: Option<String>,
}

/// The cells and sections of a `PageSheet`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sheet {
    pub cells: Cells,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Page {
    pub id: u32,
    pub name: String,
    pub background: bool,
    pub back_page: Option<u32>,
    pub sheet: Sheet,
    pub shapes: Vec<Shape>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Master {
    pub id: u32,
    pub name: String,
    pub shapes: Vec<Shape>,
}

/// A style sheet (`StyleSheet` in `document.xml`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StyleSheet {
    pub id: u32,
    pub line_style: Option<u32>,
    pub fill_style: Option<u32>,
    pub text_style: Option<u32>,
    pub sheet: Sheet,
}

/// A DrawingML theme's color scheme, by element name (`dk1`, `accent1`, ...).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Theme {
    /// The number in the part's name (`theme3.xml` is 3), for `ThemeIndex`.
    pub number: Option<u32>,
    pub colors: HashMap<String, [u8; 3]>,
}

/// A whole `.vsdx` document.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Document {
    pub pages: Vec<Page>,
    pub masters: HashMap<u32, Master>,
    pub styles: HashMap<u32, StyleSheet>,
    /// `Colors/ColorEntry`: palette entries 24 and up.
    pub palette: HashMap<u32, [u8; 3]>,
    pub themes: Vec<Theme>,
}

type Node<'a, 'input> = roxmltree::Node<'a, 'input>;

fn name_is(n: Node, name: &str) -> bool {
    n.is_element() && n.tag_name().name() == name
}

fn children<'a, 'input: 'a>(
    n: Node<'a, 'input>,
    name: &'a str,
) -> impl Iterator<Item = Node<'a, 'input>> + 'a {
    n.children().filter(move |c| name_is(*c, name))
}

fn num(n: Node, attr: &str) -> Option<u32> {
    n.attribute(attr)?.trim().parse().ok()
}

fn ix(n: Node) -> i32 {
    n.attribute("IX")
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

/// The `r:id` attribute of `n` (any prefix, in the relationships namespace).
pub(crate) fn rel_id<'a>(n: Node<'a, '_>) -> Option<&'a str> {
    n.attributes()
        .find(|a| {
            a.name() == "id"
                && a.namespace()
                    .is_some_and(|ns| ns.ends_with("/relationships"))
        })
        .map(|a| a.value())
}

fn parse_cell(n: Node) -> Option<(String, Cell)> {
    let name = n.attribute("N")?;
    Some((
        name.to_owned(),
        Cell {
            v: n.attribute("V").unwrap_or_default().to_owned(),
            f: n.attribute("F").map(str::to_owned),
        },
    ))
}

fn parse_cells(n: Node) -> Cells {
    children(n, "Cell").filter_map(parse_cell).collect()
}

fn parse_section(n: Node, rows_left: &mut usize) -> Result<Section> {
    let mut section = Section {
        n: n.attribute("N").unwrap_or_default().to_owned(),
        ix: ix(n),
        cells: parse_cells(n),
        rows: Vec::new(),
    };
    for row in children(n, "Row") {
        *rows_left = rows_left.checked_sub(1).ok_or_else(|| {
            Error::new(format!(
                "a shape has more than {} geometry rows",
                limits::MAX_ROWS
            ))
        })?;
        section.rows.push(Row {
            t: row.attribute("T").unwrap_or_default().to_owned(),
            n: row.attribute("N").unwrap_or_default().to_owned(),
            ix: ix(row),
            del: row.attribute("Del").is_some_and(|d| d == "1"),
            cells: parse_cells(row),
        });
    }
    Ok(section)
}

fn parse_sheet(n: Node) -> Result<Sheet> {
    let mut rows_left = limits::MAX_ROWS;
    let mut sheet = Sheet {
        cells: parse_cells(n),
        sections: Vec::new(),
    };
    for s in children(n, "Section") {
        sheet.sections.push(parse_section(s, &mut rows_left)?);
    }
    Ok(sheet)
}

fn parse_text(n: Node, budget: &mut usize) -> Result<Vec<TextPart>> {
    let mut parts = Vec::new();
    for c in n.children() {
        let chars = if c.is_text() {
            c.text().unwrap_or_default().to_owned()
        } else if name_is(c, "cp") {
            parts.push(TextPart::Cp(ix(c)));
            continue;
        } else if name_is(c, "pp") {
            parts.push(TextPart::Pp(ix(c)));
            continue;
        } else if name_is(c, "tp") {
            parts.push(TextPart::Tp(ix(c)));
            continue;
        } else if name_is(c, "fld") {
            c.descendants()
                .filter(|d| d.is_text())
                .filter_map(|d| d.text())
                .collect()
        } else {
            continue;
        };
        *budget = budget
            .checked_sub(chars.len())
            .ok_or_else(|| Error::new("a shape's text is over 64 KiB"))?;
        if !chars.is_empty() {
            parts.push(TextPart::Chars(chars));
        }
    }
    Ok(parts)
}

fn parse_shape(n: Node, depth: usize, count: &mut usize) -> Result<Shape> {
    if depth > limits::MAX_GROUP_DEPTH {
        return Err(Error::new(format!(
            "shapes nest deeper than {}",
            limits::MAX_GROUP_DEPTH
        )));
    }
    *count = count
        .checked_sub(1)
        .ok_or_else(|| Error::new(format!("over {} shapes in a part", limits::MAX_SHAPES)))?;
    let sheet = parse_sheet(n)?;
    let mut shape = Shape {
        id: num(n, "ID").unwrap_or(0),
        kind: match n.attribute("Type") {
            None | Some("Shape") => ShapeType::Shape,
            Some("Group") => ShapeType::Group,
            Some("Foreign") => ShapeType::Foreign,
            Some(_) => ShapeType::Other,
        },
        master: num(n, "Master"),
        master_shape: num(n, "MasterShape"),
        line_style: num(n, "LineStyle"),
        fill_style: num(n, "FillStyle"),
        text_style: num(n, "TextStyle"),
        cells: sheet.cells,
        sections: sheet.sections,
        ..Shape::default()
    };
    if let Some(t) = children(n, "Text").next() {
        let mut budget = limits::MAX_TEXT;
        shape.text = Some(parse_text(t, &mut budget)?);
    }
    if let Some(f) = children(n, "ForeignData").next() {
        shape.foreign = Some(f.attribute("ForeignType").unwrap_or("Object").to_owned());
    }
    for shapes in children(n, "Shapes") {
        for child in children(shapes, "Shape") {
            shape.children.push(parse_shape(child, depth + 1, count)?);
        }
    }
    Ok(shape)
}

/// The top-level shapes of a page or master part (`PageContents` or `MasterContents`).
pub(crate) fn parse_contents(root: Node, part: &str) -> Result<Vec<Shape>> {
    let mut count = limits::MAX_SHAPES;
    let mut shapes = Vec::new();
    for list in children(root, "Shapes") {
        for s in children(list, "Shape") {
            shapes.push(parse_shape(s, 0, &mut count).map_err(|e| e.context(part.to_owned()))?);
        }
    }
    Ok(shapes)
}

/// One `Page` entry of `pages.xml`, without its shapes.
pub(crate) fn parse_page_entry(n: Node) -> Result<Page> {
    Ok(Page {
        id: num(n, "ID").unwrap_or(0),
        name: n
            .attribute("NameU")
            .or_else(|| n.attribute("Name"))
            .unwrap_or_default()
            .to_owned(),
        background: n.attribute("Background").is_some_and(|b| b == "1"),
        back_page: num(n, "BackPage"),
        sheet: match children(n, "PageSheet").next() {
            Some(s) => parse_sheet(s)?,
            None => Sheet::default(),
        },
        shapes: Vec::new(),
    })
}

/// One `Master` entry of `masters.xml`, without its shapes.
pub(crate) fn parse_master_entry(n: Node) -> Master {
    Master {
        id: num(n, "ID").unwrap_or(0),
        name: n
            .attribute("NameU")
            .or_else(|| n.attribute("Name"))
            .unwrap_or_default()
            .to_owned(),
        shapes: Vec::new(),
    }
}

/// The style sheets and palette of `document.xml`.
pub(crate) fn parse_document(root: Node, doc: &mut Document) -> Result<()> {
    for sheets in children(root, "StyleSheets") {
        for s in children(sheets, "StyleSheet") {
            let id = num(s, "ID").unwrap_or(0);
            doc.styles.insert(
                id,
                StyleSheet {
                    id,
                    line_style: num(s, "LineStyle"),
                    fill_style: num(s, "FillStyle"),
                    text_style: num(s, "TextStyle"),
                    sheet: parse_sheet(s)?,
                },
            );
        }
    }
    for colors in children(root, "Colors") {
        for e in children(colors, "ColorEntry") {
            if let (Some(i), Some(rgb)) = (num(e, "IX"), e.attribute("RGB").and_then(hex)) {
                doc.palette.insert(i, rgb);
            }
        }
    }
    Ok(())
}

/// `#RRGGBB` (the `#` optional) as bytes.
pub(crate) fn hex(s: &str) -> Option<[u8; 3]> {
    let s = s.trim();
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 || !s.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// The color scheme of a theme part.
pub(crate) fn parse_theme(root: Node, number: Option<u32>) -> Theme {
    let mut theme = Theme {
        number,
        colors: HashMap::new(),
    };
    for scheme in root.descendants().filter(|d| name_is(*d, "clrScheme")) {
        for slot in scheme.children().filter(|c| c.is_element()) {
            let rgb = slot.children().filter(|c| c.is_element()).find_map(|c| {
                match c.tag_name().name() {
                    "srgbClr" => c.attribute("val").and_then(hex),
                    "sysClr" => c.attribute("lastClr").and_then(hex),
                    _ => None,
                }
            });
            if let Some(rgb) = rgb {
                theme.colors.insert(slot.tag_name().name().to_owned(), rgb);
            }
        }
    }
    theme
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_colors() {
        assert_eq!(hex("#ff8000"), Some([255, 128, 0]));
        assert_eq!(hex("FF8000"), Some([255, 128, 0]));
        assert_eq!(hex("#fff"), None);
        assert_eq!(hex("#gg0000"), None);
        assert_eq!(hex("é23456"), None);
    }

    #[test]
    fn shape_cells_sections_and_text() {
        let xml = r#"<PageContents xmlns="http://schemas.microsoft.com/office/visio/2012/main"><Shapes>
            <Shape ID="1" Type="Group" LineStyle="2"><Cell N="PinX" V="1.5" F="Width*0.5"/>
              <Section N="Geometry" IX="0"><Cell N="NoFill" V="1"/><Row T="MoveTo" IX="1"><Cell N="X" V="0"/><Cell N="Y" V="0"/></Row></Section>
              <Text>a<cp IX="1"/>b<fld IX="2">9</fld></Text>
              <Shapes><Shape ID="2"/></Shapes>
            </Shape></Shapes></PageContents>"#;
        let doc = roxmltree::Document::parse(xml).unwrap();
        let shapes = parse_contents(doc.root_element(), "p").unwrap();
        let s = &shapes[0];
        assert_eq!((s.id, s.kind, s.line_style), (1, ShapeType::Group, Some(2)));
        assert_eq!(s.cells["PinX"].v, "1.5");
        assert_eq!(s.cells["PinX"].f.as_deref(), Some("Width*0.5"));
        assert_eq!(s.sections[0].rows[0].t, "MoveTo");
        assert_eq!(s.sections[0].rows[0].cells["X"].v, "0");
        assert_eq!(
            s.text,
            Some(vec![
                TextPart::Chars("a".into()),
                TextPart::Cp(1),
                TextPart::Chars("b".into()),
                TextPart::Chars("9".into())
            ])
        );
        assert_eq!(s.children[0].id, 2);
    }

    #[test]
    fn nesting_and_counts_are_bounded() {
        let mut xml = String::from(r#"<PageContents><Shapes>"#);
        for _ in 0..70 {
            xml.push_str("<Shape ID=\"1\"><Shapes>");
        }
        for _ in 0..70 {
            xml.push_str("</Shapes></Shape>");
        }
        xml.push_str("</Shapes></PageContents>");
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let e = parse_contents(doc.root_element(), "p").unwrap_err();
        assert!(e.report().contains("nest deeper"), "{}", e.report());
    }

    #[test]
    fn theme_colors() {
        let xml = r#"<a:theme xmlns:a="x"><a:themeElements><a:clrScheme name="t">
            <a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1>
            <a:accent1><a:srgbClr val="4472C4"/></a:accent1></a:clrScheme></a:themeElements></a:theme>"#;
        let doc = roxmltree::Document::parse(xml).unwrap();
        let t = parse_theme(doc.root_element(), Some(3));
        assert_eq!(t.colors["dk1"], [0, 0, 0]);
        assert_eq!(t.colors["accent1"], [0x44, 0x72, 0xC4]);
        assert_eq!(t.number, Some(3));
    }
}
