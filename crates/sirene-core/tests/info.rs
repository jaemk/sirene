//! Info diagrams (MINFO-*): sirene's version, drawn and as text. The version changes every
//! release, so these assert on it rather than snapshot it.

use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind};
use sirene_core::info::{self, Info, VERSION};
use sirene_core::scene::{Approx, Primitive, Theme};

fn lowered(src: &str) -> (Info, Vec<String>) {
    let lowered = diagram::lower(src);
    let d = lowered.diagnostics.into_iter().map(|d| d.message).collect();
    match lowered.diagram {
        Diagram::Info(i) => (i, d),
        other => panic!("not an info diagram: {other:?}"),
    }
}

/// MINFO-1, MINFO-2: the header, `showInfo`, the title, and the accessibility lines.
#[test]
fn model() {
    for src in [
        "info",
        "info\n",
        "info showInfo\n",
        "%% c\ninfo\n  showInfo\n",
    ] {
        assert_eq!(Kind::of(src), Kind::Info, "{src:?}");
        assert_eq!(info::parse(src).syntax().to_string(), src);
        let (i, d) = lowered(src);
        assert!(d.is_empty(), "{src:?}: {d:?}");
        assert_eq!(i.show_info, src.contains("showInfo"), "{src:?}");
    }
    let (i, d) = lowered("info\n  title T\n  accTitle: a\n  accDescr: d\n");
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(i.title.as_deref(), Some("T"));
    assert_eq!(i.acc_title.as_deref(), Some("a"));
    assert_eq!(i.acc_descr.as_deref(), Some("d"));
    let (i, _) = lowered("---\ntitle: F\n---\ninfo\n");
    assert_eq!(i.title.as_deref(), Some("F"));
    assert_eq!(i.text(), format!("sirene v{VERSION}"));
}

/// MINFO-1: anything else is an error; the version is still drawn.
#[test]
fn errors() {
    let (i, d) = lowered("info extra\n  A --> B\n  title T\n");
    assert_eq!(
        d,
        [
            "expected the end of the line",
            "expected `showInfo`, `title`, `accTitle`, or `accDescr`",
        ]
    );
    assert_eq!(i.title.as_deref(), Some("T"));
}

/// MINFO-3, MINFO-4: the title over the version.
#[test]
fn drawing() {
    let src = "info\n  title T\n";
    let d = diagram::lower(src).diagram;
    let s = d.scene(&Approx, &Theme::LIGHT);
    let texts: Vec<(&str, f32)> = s
        .elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Text { text, center, .. } => Some((text.as_str(), center.y)),
            _ => None,
        })
        .collect();
    let version = format!("sirene v{VERSION}");
    assert_eq!(texts.len(), 2);
    assert_eq!(texts[0].0, "T");
    assert_eq!(texts[1].0, version);
    assert!(texts[0].1 < texts[1].1);
    assert!(s.width >= 400.0);
    for charset in [Charset::Unicode, Charset::Ascii] {
        assert_eq!(d.ascii(&Options { charset }), format!("T\n\n{version}\n"));
    }
    let plain = diagram::lower("info\n").diagram;
    assert_eq!(
        plain.ascii(&Options {
            charset: Charset::Ascii
        }),
        format!("{version}\n")
    );
}
