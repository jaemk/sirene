//! Microsoft Visio `.vsdx` drawings (VISIO-1 to VISIO-16): the package is read here, the drawing
//! is built in `draw`, and `scene::visio` hands it to the backends.

pub(crate) mod limits;
pub mod model;
pub(crate) mod opc;
#[cfg(test)]
pub(crate) mod testpkg;
mod zip;

use std::sync::Arc;

use rowan::TextRange;

use crate::diagnostic::Diagnostic;
use crate::{Error, Result};
use model::{Document, Master, Page, Theme};
use opc::{parse_rels, parse_xml, rels_name, text};
use zip::Archive;

/// Pixels per inch: a page's size in the scene (VISIO-5).
pub const PX_PER_INCH: f32 = 96.0;

/// A Visio document and the page drawn from it (VISIO-11).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Visio {
    doc: Arc<Document>,
    /// The index among [`Visio::pages`] of the page drawn.
    page: usize,
}

impl Visio {
    /// The parsed document.
    pub fn document(&self) -> &Document {
        &self.doc
    }

    /// The indexes into `document().pages` of the pages that are outputs: every page that is not
    /// a background page, in file order.
    pub fn pages(&self) -> Vec<usize> {
        self.doc
            .pages
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.background)
            .map(|(i, _)| i)
            .collect()
    }

    /// The number of output pages.
    pub fn page_count(&self) -> usize {
        self.pages().len()
    }

    /// The 0-based number, among the output pages, of the page drawn.
    pub fn page(&self) -> usize {
        self.page
    }

    /// This document drawing output page `n` (0-based), or `None` when there is no such page.
    pub fn with_page(&self, n: usize) -> Option<Visio> {
        (n < self.page_count()).then(|| Visio {
            doc: Arc::clone(&self.doc),
            page: n,
        })
    }

    /// The page drawn.
    pub fn current(&self) -> Option<&Page> {
        let index = *self.pages().get(self.page)?;
        self.doc.pages.get(index)
    }
}

fn warn(diagnostics: &mut Vec<Diagnostic>, message: String) {
    diagnostics.push(Diagnostic::warning(TextRange::default(), message));
}

fn part(zip: &mut Archive, name: &str) -> Result<String> {
    let bytes = zip.read(name).map_err(|e| e.context(name.to_owned()))?;
    text(&bytes, name).map(str::to_owned)
}

/// Reads a `.vsdx` package (VISIO-2, VISIO-3): the document with its pages and masters, and the
/// warnings found on the way. An error is a corrupt package, a missing required part, or a limit
/// exceeded (VISIO-13).
pub fn read(bytes: &[u8]) -> Result<(Visio, Vec<Diagnostic>)> {
    let (doc, diagnostics) = read_document(bytes)?;
    Ok((
        Visio {
            doc: Arc::new(doc),
            page: 0,
        },
        diagnostics,
    ))
}

fn read_document(bytes: &[u8]) -> Result<(Document, Vec<Diagnostic>)> {
    let mut zip = Archive::open(bytes)?;
    let mut diagnostics = Vec::new();
    if !zip.contains("[Content_Types].xml") {
        return Err(Error::new("not a Visio package: no [Content_Types].xml"));
    }
    let types = part(&mut zip, "[Content_Types].xml")?;
    parse_xml(&types, "[Content_Types].xml")?;

    let package_rels = if zip.contains(&rels_name("")) {
        let xml = part(&mut zip, &rels_name(""))?;
        parse_rels("", &xml)?
    } else {
        Vec::new()
    };
    let doc_part = package_rels
        .iter()
        .find(|r| r.is("document"))
        .map(|r| r.target.clone())
        .ok_or_else(|| {
            Error::new("not a Visio package: no document relationship in _rels/.rels")
        })?;
    if !zip.contains(&doc_part) {
        return Err(Error::new(format!(
            "not a Visio package: missing {doc_part}"
        )));
    }

    let mut document = Document::default();
    let xml = part(&mut zip, &doc_part)?;
    {
        let parsed = parse_xml(&xml, &doc_part)?;
        model::parse_document(parsed.root_element(), &mut document)
            .map_err(|e| e.context(doc_part.clone()))?;
    }
    let doc_rels_name = rels_name(&doc_part);
    let doc_rels = if zip.contains(&doc_rels_name) {
        let xml = part(&mut zip, &doc_rels_name)?;
        parse_rels(&doc_part, &xml)?
    } else {
        Vec::new()
    };

    let pages_part = doc_rels
        .iter()
        .find(|r| r.is("pages"))
        .map(|r| r.target.clone())
        .ok_or_else(|| Error::new(format!("{doc_part}: no pages relationship")))?;
    read_pages(&mut zip, &pages_part, &mut document)?;

    if let Some(r) = doc_rels.iter().find(|r| r.is("masters")) {
        read_masters(&mut zip, &r.target, &mut document, &mut diagnostics)?;
    }

    for rel in doc_rels.iter().filter(|r| r.is("theme")) {
        match part(&mut zip, &rel.target) {
            Ok(xml) => {
                let parsed = parse_xml(&xml, &rel.target)?;
                let number = theme_number(&rel.target);
                document
                    .themes
                    .push(model::parse_theme(parsed.root_element(), number));
            }
            Err(e) => warn(&mut diagnostics, e.report()),
        }
    }
    Ok((document, diagnostics))
}

fn theme_number(name: &str) -> Option<u32> {
    let file = name.rsplit('/').next()?;
    let digits = file.strip_prefix("theme")?.strip_suffix(".xml")?;
    digits.parse().ok()
}

fn part_rels(zip: &mut Archive, name: &str) -> Result<Vec<opc::Rel>> {
    let rels = rels_name(name);
    if !zip.contains(&rels) {
        return Ok(Vec::new());
    }
    let xml = part(zip, &rels)?;
    parse_rels(name, &xml)
}

fn read_pages(zip: &mut Archive, pages_part: &str, document: &mut Document) -> Result<()> {
    let xml = part(zip, pages_part)?;
    let rels = part_rels(zip, pages_part)?;
    let parsed = parse_xml(&xml, pages_part)?;
    for node in parsed
        .root_element()
        .children()
        .filter(|n| n.has_tag_name("Page"))
    {
        if document.pages.len() >= limits::MAX_PAGES {
            return Err(Error::new(format!(
                "the document has more than {} pages",
                limits::MAX_PAGES
            )));
        }
        let mut page =
            model::parse_page_entry(node).map_err(|e| e.context(pages_part.to_owned()))?;
        let target = node
            .children()
            .find(|c| c.has_tag_name("Rel"))
            .and_then(model::rel_id)
            .and_then(|id| rels.iter().find(|r| r.id == id))
            .map(|r| r.target.clone())
            .ok_or_else(|| {
                Error::new(format!(
                    "{pages_part}: page {:?} has no page part",
                    page.name
                ))
            })?;
        let xml = part(zip, &target)?;
        let contents = parse_xml(&xml, &target)?;
        page.shapes = model::parse_contents(contents.root_element(), &target)?;
        document.pages.push(page);
    }
    if document.pages.iter().all(|p| p.background) {
        return Err(Error::new(format!(
            "{pages_part}: the document has no pages"
        )));
    }
    Ok(())
}

fn read_masters(
    zip: &mut Archive,
    masters_part: &str,
    document: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<()> {
    let xml = match part(zip, masters_part) {
        Ok(xml) => xml,
        Err(e) => {
            warn(diagnostics, format!("masters not read: {}", e.report()));
            return Ok(());
        }
    };
    let rels = part_rels(zip, masters_part)?;
    let parsed = parse_xml(&xml, masters_part)?;
    for node in parsed
        .root_element()
        .children()
        .filter(|n| n.has_tag_name("Master"))
    {
        if document.masters.len() >= limits::MAX_MASTERS {
            return Err(Error::new(format!(
                "the document has more than {} masters",
                limits::MAX_MASTERS
            )));
        }
        let mut master: Master = model::parse_master_entry(node);
        let target = node
            .children()
            .find(|c| c.has_tag_name("Rel"))
            .and_then(model::rel_id)
            .and_then(|id| rels.iter().find(|r| r.id == id))
            .map(|r| r.target.clone());
        let Some(target) = target else {
            warn(diagnostics, format!("master {:?} has no part", master.name));
            continue;
        };
        match part(zip, &target) {
            Ok(xml) => {
                let contents = parse_xml(&xml, &target)?;
                master.shapes = model::parse_contents(contents.root_element(), &target)?;
                document.masters.insert(master.id, master);
            }
            Err(e) => warn(
                diagnostics,
                format!("master {:?} not read: {}", master.name, e.report()),
            ),
        }
    }
    Ok(())
}

/// The theme `index` names (`themeN.xml`), else the first.
pub(crate) fn theme_for(document: &Document, index: Option<u32>) -> Option<&Theme> {
    index
        .and_then(|i| document.themes.iter().find(|t| t.number == Some(i)))
        .or_else(|| document.themes.first())
}

#[cfg(test)]
mod tests;
