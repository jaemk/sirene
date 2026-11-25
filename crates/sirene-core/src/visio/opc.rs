//! Open Packaging Conventions: part names, relationships, and XML parts (VISIO-2, VISIO-3).

use super::limits;
use super::zip::safe_name;
use crate::{Error, Result};

/// One relationship of a part: its id, its type URI, and the target part's name inside the
/// package (absolute, no leading slash).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rel {
    pub id: String,
    pub ty: String,
    pub target: String,
}

impl Rel {
    /// Whether the type URI ends in `/suffix` (e.g. `document`, `pages`, `theme`).
    pub(crate) fn is(&self, suffix: &str) -> bool {
        self.ty
            .rsplit('/')
            .next()
            .is_some_and(|last| last.eq_ignore_ascii_case(suffix))
    }
}

/// The name of the relationships part of `part`: `a/b.xml` has `a/_rels/b.xml.rels`, and the
/// package itself (an empty name) `_rels/.rels`.
pub(crate) fn rels_name(part: &str) -> String {
    match part.rsplit_once('/') {
        Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
        None if part.is_empty() => "_rels/.rels".to_owned(),
        None => format!("_rels/{part}.rels"),
    }
}

/// The part `target` names, relative to the directory of the part `base`; an absolute target
/// (`/x/y`) is from the package root. `None` when the name leaves the package or is unsafe.
pub(crate) fn resolve(base: &str, target: &str) -> Option<String> {
    let target = target.split(['#', '?']).next().unwrap_or(target);
    let mut parts: Vec<&str> = if target.starts_with('/') {
        Vec::new()
    } else {
        let dir = base.rsplit_once('/').map_or("", |(d, _)| d);
        dir.split('/').filter(|s| !s.is_empty()).collect()
    };
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    let name = parts.join("/");
    safe_name(&name).then_some(name)
}

/// The relationships in the XML of `base`'s relationships part. External targets are ignored
/// (VISIO-2), as are targets that leave the package.
pub(crate) fn parse_rels(base: &str, xml: &str) -> Result<Vec<Rel>> {
    let doc = parse_xml(xml, &rels_name(base))?;
    let mut rels = Vec::new();
    for node in doc.descendants().filter(|n| n.has_tag_name("Relationship")) {
        if node
            .attribute("TargetMode")
            .is_some_and(|m| m.eq_ignore_ascii_case("external"))
        {
            continue;
        }
        let (Some(id), Some(ty), Some(target)) = (
            node.attribute("Id"),
            node.attribute("Type"),
            node.attribute("Target"),
        ) else {
            continue;
        };
        if let Some(target) = resolve(base, target) {
            rels.push(Rel {
                id: id.to_owned(),
                ty: ty.to_owned(),
                target,
            });
        }
    }
    Ok(rels)
}

/// Parses an XML part with DTDs refused and the node and depth limits applied (VISIO-13).
pub(crate) fn parse_xml<'a>(text: &'a str, part: &str) -> Result<roxmltree::Document<'a>> {
    if text.len() > limits::MAX_XML {
        return Err(Error::new(format!("{part}: over 16 MiB")));
    }
    let options = roxmltree::ParsingOptions {
        allow_dtd: false,
        nodes_limit: limits::MAX_XML_NODES,
        ..roxmltree::ParsingOptions::default()
    };
    let doc = roxmltree::Document::parse_with_options(text, options).map_err(|e| {
        let what = match e {
            roxmltree::Error::DtdDetected => "a DOCTYPE is not allowed".to_owned(),
            roxmltree::Error::NodesLimitReached => "too many nodes".to_owned(),
            other => other.to_string(),
        };
        Error::new(format!("{part}: {what}"))
    })?;
    let mut depth = vec![0usize; doc.descendants().count() + 1];
    for node in doc.descendants() {
        let d = node
            .parent()
            .and_then(|p| depth.get(p.id().get_usize()).copied())
            .map_or(0, |d| d + 1);
        if d > limits::MAX_XML_DEPTH {
            return Err(Error::new(format!(
                "{part}: elements nest deeper than {}",
                limits::MAX_XML_DEPTH
            )));
        }
        if let Some(slot) = depth.get_mut(node.id().get_usize()) {
            *slot = d;
        }
    }
    Ok(doc)
}

/// Decodes a part's bytes as UTF-8 (a leading BOM is dropped).
pub(crate) fn text<'a>(bytes: &'a [u8], part: &str) -> Result<&'a str> {
    let s =
        std::str::from_utf8(bytes).map_err(|_| Error::new(format!("{part}: not UTF-8 text")))?;
    Ok(s.strip_prefix('\u{feff}').unwrap_or(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rels_names() {
        assert_eq!(rels_name(""), "_rels/.rels");
        assert_eq!(
            rels_name("visio/document.xml"),
            "visio/_rels/document.xml.rels"
        );
        assert_eq!(rels_name("a.xml"), "_rels/a.xml.rels");
    }

    #[test]
    fn resolving() {
        assert_eq!(
            resolve("visio/document.xml", "pages/pages.xml").as_deref(),
            Some("visio/pages/pages.xml")
        );
        assert_eq!(
            resolve("visio/pages/pages.xml", "../masters/m.xml").as_deref(),
            Some("visio/masters/m.xml")
        );
        assert_eq!(
            resolve("visio/document.xml", "/docProps/app.xml").as_deref(),
            Some("docProps/app.xml")
        );
        assert_eq!(
            resolve("", "visio/document.xml").as_deref(),
            Some("visio/document.xml")
        );
        assert_eq!(resolve("a.xml", "../../etc/passwd"), None);
        assert_eq!(resolve("visio/document.xml", "../../x"), None);
        assert_eq!(resolve("a/b.xml", "c.xml#frag").as_deref(), Some("a/c.xml"));
        assert_eq!(resolve("a/b.xml", "C:\\x"), None);
    }

    #[test]
    fn rels_skip_external() {
        let xml = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
            <Relationship Id="rId1" Type="http://x/pages" Target="pages/pages.xml"/>
            <Relationship Id="rId2" Type="http://x/hyperlink" Target="http://evil.example/" TargetMode="External"/>
            <Relationship Id="rId3" Type="http://x/theme" Target="../../outside.xml"/>
        </Relationships>"#;
        let rels = parse_rels("visio/document.xml", xml).unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].target, "visio/pages/pages.xml");
        assert!(rels[0].is("pages"));
        assert!(!rels[0].is("theme"));
    }

    #[test]
    fn doctype_is_refused() {
        let e = parse_xml("<!DOCTYPE a [<!ENTITY x \"y\">]><a>&x;</a>", "p.xml").unwrap_err();
        assert!(e.report().contains("DOCTYPE"), "{}", e.report());
    }

    #[test]
    fn depth_is_bounded() {
        let deep = format!("{}{}", "<a>".repeat(300), "</a>".repeat(300));
        let e = parse_xml(&deep, "p.xml").unwrap_err();
        assert!(e.report().contains("nest deeper"), "{}", e.report());
        let ok = format!("{}{}", "<a>".repeat(200), "</a>".repeat(200));
        assert!(parse_xml(&ok, "p.xml").is_ok());
    }
}
