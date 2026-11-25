//! Builds `.vsdx` packages from XML strings, for tests only (VISIO-16). This file is also
//! included by `tests/visio.rs` with `#[path]`, so it uses nothing from the crate.
#![allow(dead_code, clippy::unwrap_used)]

pub const NS: &str = "http://schemas.microsoft.com/office/visio/2012/main";
const REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PKG_REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// One entry to write: its name, its bytes, and whether to deflate it.
#[derive(Clone)]
pub struct Entry {
    pub name: String,
    pub data: Vec<u8>,
    pub deflate: bool,
}

/// A ZIP writer: stored or deflated entries, a central directory, and no zip64.
pub fn zip(entries: &[Entry]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for e in entries {
        let (method, body) = if e.deflate {
            (8u16, miniz_oxide::deflate::compress_to_vec(&e.data, 6))
        } else {
            (0u16, e.data.clone())
        };
        let offset = out.len() as u32;
        let crc = crc32(&e.data);
        let name = e.name.as_bytes();
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&[20, 0, 0, 0]);
        out.extend_from_slice(&method.to_le_bytes());
        out.extend_from_slice(&[0, 0, 0, 0]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&(e.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(&body);

        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&[20, 0, 20, 0, 0, 0]);
        central.extend_from_slice(&method.to_le_bytes());
        central.extend_from_slice(&[0, 0, 0, 0]);
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(body.len() as u32).to_le_bytes());
        central.extend_from_slice(&(e.data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name);
    }
    let cd_offset = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]);
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// A stored entry.
pub fn stored(name: &str, data: &[u8]) -> Entry {
    Entry {
        name: name.to_owned(),
        data: data.to_vec(),
        deflate: false,
    }
}

/// A page to build: extra `Page` attributes, `PageSheet` cells, and its shapes' XML.
#[derive(Clone)]
pub struct PageSpec {
    pub attrs: String,
    pub sheet: String,
    pub shapes: String,
}

/// Assembles a package: pages, masters, style sheets, palette entries, and themes.
#[derive(Clone, Default)]
pub struct Builder {
    pages: Vec<PageSpec>,
    masters: Vec<(u32, String)>,
    styles: String,
    colors: String,
    themes: Vec<String>,
    pub deflate: bool,
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
    }

    /// A page of `w` by `h` inches holding `shapes`.
    pub fn page(self, w: f64, h: f64, shapes: &str) -> Self {
        self.page_with(
            "",
            &format!(r#"<Cell N="PageWidth" V="{w}"/><Cell N="PageHeight" V="{h}"/>"#),
            shapes,
        )
    }

    /// A page with `Page` attributes `attrs` and `PageSheet` content `sheet`.
    pub fn page_with(mut self, attrs: &str, sheet: &str, shapes: &str) -> Self {
        self.pages.push(PageSpec {
            attrs: attrs.to_owned(),
            sheet: sheet.to_owned(),
            shapes: shapes.to_owned(),
        });
        self
    }

    /// A master with ID `id` holding `shapes`.
    pub fn master(mut self, id: u32, shapes: &str) -> Self {
        self.masters.push((id, shapes.to_owned()));
        self
    }

    /// `StyleSheet` elements for `document.xml`.
    pub fn styles(mut self, xml: &str) -> Self {
        self.styles = xml.to_owned();
        self
    }

    /// `ColorEntry` elements for `document.xml`.
    pub fn colors(mut self, xml: &str) -> Self {
        self.colors = xml.to_owned();
        self
    }

    /// A theme part holding `scheme` (the inside of `a:clrScheme`).
    pub fn theme(mut self, scheme: &str) -> Self {
        self.themes.push(scheme.to_owned());
        self
    }

    /// The package's parts, in order, as (name, bytes).
    pub fn parts(&self) -> Vec<(String, Vec<u8>)> {
        let xml = |s: String| format!(r#"<?xml version="1.0" encoding="utf-8"?>{s}"#).into_bytes();
        let mut parts: Vec<(String, Vec<u8>)> = Vec::new();
        parts.push((
            "[Content_Types].xml".into(),
            xml(r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/></Types>"#.into()),
        ));
        parts.push((
            "_rels/.rels".into(),
            xml(format!(r#"<Relationships xmlns="{PKG_REL_NS}"><Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/document" Target="visio/document.xml"/></Relationships>"#)),
        ));
        parts.push((
            "visio/document.xml".into(),
            xml(format!(r#"<VisioDocument xmlns="{NS}" xmlns:r="{REL_NS}"><Colors>{}</Colors><StyleSheets>{}</StyleSheets></VisioDocument>"#, self.colors, self.styles)),
        ));
        let mut doc_rels = String::from(
            r#"<Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/pages" Target="pages/pages.xml"/><Relationship Id="rId2" Type="http://schemas.microsoft.com/visio/2010/relationships/masters" Target="masters/masters.xml"/>"#,
        );
        for (i, _) in self.themes.iter().enumerate() {
            doc_rels.push_str(&format!(
                r#"<Relationship Id="rIdT{n}" Type="{REL_NS}/theme" Target="theme/theme{n}.xml"/>"#,
                n = i + 1
            ));
        }
        parts.push((
            "visio/_rels/document.xml.rels".into(),
            xml(format!(
                r#"<Relationships xmlns="{PKG_REL_NS}">{doc_rels}</Relationships>"#
            )),
        ));
        let mut pages = String::new();
        let mut page_rels = String::new();
        for (i, p) in self.pages.iter().enumerate() {
            let n = i + 1;
            pages.push_str(&format!(
                r#"<Page ID="{i}" Name="Page-{n}" NameU="Page-{n}" {attrs}><PageSheet>{sheet}</PageSheet><Rel r:id="rId{n}"/></Page>"#,
                attrs = p.attrs,
                sheet = p.sheet
            ));
            page_rels.push_str(&format!(
                r#"<Relationship Id="rId{n}" Type="http://schemas.microsoft.com/visio/2010/relationships/page" Target="page{n}.xml"/>"#
            ));
            parts.push((
                format!("visio/pages/page{n}.xml"),
                xml(format!(r#"<PageContents xmlns="{NS}" xmlns:r="{REL_NS}"><Shapes>{}</Shapes></PageContents>"#, p.shapes)),
            ));
        }
        parts.push((
            "visio/pages/pages.xml".into(),
            xml(format!(
                r#"<Pages xmlns="{NS}" xmlns:r="{REL_NS}">{pages}</Pages>"#
            )),
        ));
        parts.push((
            "visio/pages/_rels/pages.xml.rels".into(),
            xml(format!(
                r#"<Relationships xmlns="{PKG_REL_NS}">{page_rels}</Relationships>"#
            )),
        ));
        let mut masters = String::new();
        let mut master_rels = String::new();
        for (id, shapes) in &self.masters {
            masters.push_str(&format!(
                r#"<Master ID="{id}" NameU="Master{id}"><Rel r:id="rId{id}"/></Master>"#
            ));
            master_rels.push_str(&format!(
                r#"<Relationship Id="rId{id}" Type="http://schemas.microsoft.com/visio/2010/relationships/master" Target="master{id}.xml"/>"#
            ));
            parts.push((
                format!("visio/masters/master{id}.xml"),
                xml(format!(r#"<MasterContents xmlns="{NS}" xmlns:r="{REL_NS}"><Shapes>{shapes}</Shapes></MasterContents>"#)),
            ));
        }
        parts.push((
            "visio/masters/masters.xml".into(),
            xml(format!(
                r#"<Masters xmlns="{NS}" xmlns:r="{REL_NS}">{masters}</Masters>"#
            )),
        ));
        parts.push((
            "visio/masters/_rels/masters.xml.rels".into(),
            xml(format!(
                r#"<Relationships xmlns="{PKG_REL_NS}">{master_rels}</Relationships>"#
            )),
        ));
        for (i, scheme) in self.themes.iter().enumerate() {
            parts.push((
                format!("visio/theme/theme{}.xml", i + 1),
                xml(format!(r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:themeElements><a:clrScheme name="t">{scheme}</a:clrScheme></a:themeElements></a:theme>"#)),
            ));
        }
        parts
    }

    /// The package as `.vsdx` bytes.
    pub fn build(&self) -> Vec<u8> {
        self.build_with(|_| true)
    }

    /// The package without the parts `keep` rejects.
    pub fn build_with(&self, keep: impl Fn(&str) -> bool) -> Vec<u8> {
        let entries: Vec<Entry> = self
            .parts()
            .into_iter()
            .filter(|(n, _)| keep(n))
            .map(|(name, data)| Entry {
                name,
                data,
                deflate: self.deflate,
            })
            .collect();
        zip(&entries)
    }
}

/// One `Cell` element, for building shapes.
pub fn cell(name: &str, v: impl std::fmt::Display) -> String {
    format!(r#"<Cell N="{name}" V="{v}"/>"#)
}

/// The transform cells of a shape.
pub fn xform(pin_x: f64, pin_y: f64, w: f64, h: f64) -> String {
    format!(
        "{}{}{}{}{}{}",
        cell("PinX", pin_x),
        cell("PinY", pin_y),
        cell("Width", w),
        cell("Height", h),
        cell("LocPinX", w / 2.0),
        cell("LocPinY", h / 2.0)
    )
}

/// A geometry row: `row("LineTo", 3, &[("X", 1.0), ("Y", 2.0)])`.
pub fn row(kind: &str, ix: u32, cells: &[(&str, f64)]) -> String {
    let cells: String = cells.iter().map(|(n, v)| cell(n, v)).collect();
    format!(r#"<Row T="{kind}" IX="{ix}">{cells}</Row>"#)
}

/// A geometry section holding `rows`.
pub fn geometry(ix: u32, flags: &str, rows: &[String]) -> String {
    format!(
        r#"<Section N="Geometry" IX="{ix}">{flags}{}</Section>"#,
        rows.concat()
    )
}

/// A closed rectangle path `w` by `h` from the origin.
pub fn rect_geometry(w: f64, h: f64) -> String {
    geometry(
        0,
        "",
        &[
            row("MoveTo", 1, &[("X", 0.0), ("Y", 0.0)]),
            row("LineTo", 2, &[("X", w), ("Y", 0.0)]),
            row("LineTo", 3, &[("X", w), ("Y", h)]),
            row("LineTo", 4, &[("X", 0.0), ("Y", h)]),
            row("LineTo", 5, &[("X", 0.0), ("Y", 0.0)]),
        ],
    )
}

/// A `Shape` element.
pub fn shape(id: u32, attrs: &str, body: &str) -> String {
    format!(r#"<Shape ID="{id}" {attrs}>{body}</Shape>"#)
}
