use super::testpkg::*;
use super::*;

fn rect() -> String {
    shape(
        1,
        r#"Type="Shape""#,
        &format!("{}{}", xform(2.0, 1.0, 2.0, 1.0), rect_geometry(2.0, 1.0)),
    )
}

fn read_bytes(bytes: &[u8]) -> Result<(Visio, Vec<Diagnostic>)> {
    read(bytes)
}

#[test]
fn reads_a_minimal_package() {
    let bytes = Builder::new().page(8.5, 11.0, &rect()).build();
    let (visio, diagnostics) = read_bytes(&bytes).unwrap();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(visio.page_count(), 1);
    let page = visio.current().unwrap();
    assert_eq!(page.sheet.cells["PageWidth"].v, "8.5");
    assert_eq!(page.shapes.len(), 1);
    assert_eq!(page.shapes[0].cells["PinX"].v, "2");
    assert_eq!(page.shapes[0].sections[0].rows.len(), 5);
}

#[test]
fn reads_a_deflated_package() {
    let mut b = Builder::new().page(4.0, 3.0, &rect());
    b.deflate = true;
    let (visio, _) = read_bytes(&b.build()).unwrap();
    assert_eq!(visio.current().unwrap().shapes.len(), 1);
}

#[test]
fn reads_pages_masters_styles_palette_and_themes() {
    let b = Builder::new()
        .page(4.0, 3.0, &rect())
        .page_with(r#"Background="1""#, "", "")
        .page(5.0, 5.0, &rect())
        .master(7, &rect())
        .styles(r#"<StyleSheet ID="0" LineStyle="0" FillStyle="0" TextStyle="0"><Cell N="LineWeight" V="0.02"/></StyleSheet>"#)
        .colors(r##"<ColorEntry IX="24" RGB="#102030"/>"##)
        .theme(r#"<a:accent1><a:srgbClr val="FF0000"/></a:accent1>"#);
    let (visio, diagnostics) = read_bytes(&b.build()).unwrap();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let doc = visio.document();
    assert_eq!(doc.pages.len(), 3);
    assert_eq!(visio.pages(), [0, 2]);
    assert_eq!(visio.page_count(), 2);
    assert_eq!(
        visio.with_page(1).unwrap().current().unwrap().sheet.cells["PageWidth"].v,
        "5"
    );
    assert!(visio.with_page(2).is_none());
    assert_eq!(doc.masters[&7].shapes.len(), 1);
    assert_eq!(doc.styles[&0].sheet.cells["LineWeight"].v, "0.02");
    assert_eq!(doc.palette[&24], [0x10, 0x20, 0x30]);
    assert_eq!(doc.themes[0].number, Some(1));
    assert_eq!(doc.themes[0].colors["accent1"], [255, 0, 0]);
}

#[test]
fn a_missing_masters_part_is_a_warning() {
    let b = Builder::new().page(4.0, 3.0, &rect()).master(7, &rect());
    let bytes = b.build_with(|n| n != "visio/masters/masters.xml");
    let (visio, diagnostics) = read_bytes(&bytes).unwrap();
    assert_eq!(visio.page_count(), 1);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].message.contains("masters not read"));
}

#[test]
fn a_missing_required_part_is_an_error() {
    let b = Builder::new().page(4.0, 3.0, &rect());
    for (gone, expect) in [
        ("[Content_Types].xml", "Content_Types"),
        ("_rels/.rels", "document relationship"),
        ("visio/document.xml", "missing visio/document.xml"),
        ("visio/pages/pages.xml", "pages.xml"),
        ("visio/pages/page1.xml", "page1.xml"),
        ("visio/_rels/document.xml.rels", "no pages relationship"),
        ("visio/pages/_rels/pages.xml.rels", "no page part"),
    ] {
        let bytes = b.build_with(|n| n != gone);
        let e = read_bytes(&bytes).unwrap_err();
        assert!(
            e.report().contains(expect),
            "without {gone}: {}",
            e.report()
        );
    }
}

#[test]
fn not_a_package() {
    assert!(read_bytes(b"hello").is_err());
    let bytes = zip(&[stored("a.txt", b"x")]);
    assert!(
        read_bytes(&bytes)
            .unwrap_err()
            .report()
            .contains("Content_Types")
    );
}

#[test]
fn bad_xml_is_an_error_naming_the_part() {
    let bytes = {
        let b = Builder::new().page(4.0, 3.0, &rect());
        let entries: Vec<Entry> = b
            .parts()
            .into_iter()
            .map(|(name, data)| Entry {
                data: if name == "visio/pages/page1.xml" {
                    b"<PageContents><Shapes>".to_vec()
                } else {
                    data
                },
                name,
                deflate: false,
            })
            .collect();
        zip(&entries)
    };
    let e = read_bytes(&bytes).unwrap_err();
    assert!(
        e.report().contains("visio/pages/page1.xml"),
        "{}",
        e.report()
    );
}

#[test]
fn a_doctype_in_any_part_is_an_error() {
    let b = Builder::new().page(4.0, 3.0, &rect());
    let entries: Vec<Entry> = b
        .parts()
        .into_iter()
        .map(|(name, data)| Entry {
            data: if name == "visio/document.xml" {
                b"<!DOCTYPE d [<!ENTITY e \"x\">]><VisioDocument/>".to_vec()
            } else {
                data
            },
            name,
            deflate: false,
        })
        .collect();
    let e = read_bytes(&zip(&entries)).unwrap_err();
    assert!(e.report().contains("DOCTYPE"), "{}", e.report());
}

#[test]
fn external_relationship_targets_are_not_followed() {
    let b = Builder::new().page(4.0, 3.0, &rect());
    let entries: Vec<Entry> = b
        .parts()
        .into_iter()
        .map(|(name, data)| {
            let data = if name == "visio/_rels/document.xml.rels" {
                String::from_utf8(data)
                    .unwrap()
                    .replace(
                        "</Relationships>",
                        r#"<Relationship Id="x" Type="http://x/theme" Target="file:///etc/passwd" TargetMode="External"/></Relationships>"#,
                    )
                    .into_bytes()
            } else {
                data
            };
            Entry { name, data, deflate: false }
        })
        .collect();
    let (visio, diagnostics) = read_bytes(&zip(&entries)).unwrap();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(visio.document().themes.is_empty());
}

#[test]
fn too_many_pages_is_an_error() {
    let mut b = Builder::new();
    for _ in 0..=limits::MAX_PAGES {
        b = b.page(1.0, 1.0, "");
    }
    let e = read_bytes(&b.build()).unwrap_err();
    assert!(e.report().contains("pages"), "{}", e.report());
}

#[test]
fn too_many_shapes_is_an_error() {
    let many = r#"<Shape ID="1"/>"#.repeat(limits::MAX_SHAPES + 1);
    let e = read_bytes(&Builder::new().page(1.0, 1.0, &many).build()).unwrap_err();
    assert!(e.report().contains("shapes"), "{}", e.report());
}

#[test]
fn theme_numbers() {
    assert_eq!(theme_number("visio/theme/theme12.xml"), Some(12));
    assert_eq!(theme_number("visio/theme/other.xml"), None);
}
