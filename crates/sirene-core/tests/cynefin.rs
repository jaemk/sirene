//! Cynefin diagrams (MCYN-*): the fixture corpus and its drawings (LANGS-6), and the model each
//! source builds.

use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::cynefin::{self, Cynefin, Domain};
use sirene_core::diagram::{self, Diagram, Kind};
use sirene_core::scene::{Approx, Primitive, Theme};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cynefin");
    let mut out: Vec<(String, String)> = fs::read_dir(root.join(dir))
        .expect("fixture dir exists")
        .map(|e| e.expect("readable entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "mmd"))
        .map(|p| {
            let name = p
                .file_stem()
                .expect("file name")
                .to_string_lossy()
                .into_owned();
            (name, fs::read_to_string(&p).expect("readable fixture"))
        })
        .collect();
    out.sort();
    out
}

/// Each fixture's drawing as text; the tree always holds the source exactly.
#[test]
fn snapshots() {
    for (name, src) in fixtures("valid") {
        assert_eq!(cynefin::parse(&src).syntax().to_string(), src, "{name}");
        assert_eq!(Kind::of(&src), Kind::Cynefin, "{name}");
        let lowered = diagram::lower(&src);
        assert!(
            lowered.diagnostics.is_empty(),
            "{name}: {:?}",
            lowered.diagnostics
        );
        for charset in [Charset::Unicode, Charset::Ascii] {
            let out = lowered.diagram.ascii(&Options { charset });
            let kind = match charset {
                Charset::Unicode => "valid",
                Charset::Ascii => "ascii",
            };
            insta::assert_snapshot!(format!("{kind}__{name}"), out, &src);
        }
        for theme in [Theme::LIGHT, Theme::DARK] {
            let s = lowered.diagram.scene(&Approx, &theme);
            assert!(!s.elements.is_empty(), "{name}");
        }
    }
}

fn lowered(src: &str) -> Cynefin {
    let lowered = diagram::lower(src);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    match lowered.diagram {
        Diagram::Cynefin(c) => c,
        other => panic!("not a Cynefin diagram: {other:?}"),
    }
}

/// MCYN-1, MCYN-2: domains, items, transitions, and the title.
#[test]
fn model() {
    let c = lowered(
        "cynefin-beta\ntitle T\ncomplex \"a\"\n  'b' \"c\"\nclear\nconfusion \"x\"\n\
         complex --> clear : \"Codified\"\nclear-->chaotic\n",
    );
    assert_eq!(c.title.as_deref(), Some("T"));
    assert_eq!(c.items(Domain::Complex), ["a", "b", "c"]);
    assert!(c.items(Domain::Clear).is_empty());
    assert_eq!(c.items(Domain::Confusion), ["x"]);
    let t: Vec<_> = c
        .transitions
        .iter()
        .map(|t| (t.from, t.to, t.label.as_deref()))
        .collect();
    assert_eq!(
        t,
        [
            (Domain::Complex, Domain::Clear, Some("Codified")),
            (Domain::Clear, Domain::Chaotic, None),
        ]
    );
    assert!(c.descriptions);
}

/// MCYN-2: confusion shows three items and counts the rest.
#[test]
fn confusion_cap() {
    let c = lowered("cynefin-beta\nconfusion \"a\" \"b\" \"c\" \"d\" \"e\"\n");
    let (shown, more) = c.shown(Domain::Confusion);
    assert_eq!((shown.len(), more), (3, 2));
}

/// MCYN-2: a redeclared domain replaces its items and a self-loop is dropped, each with a
/// warning.
#[test]
fn warnings() {
    let l = diagram::lower("cynefin-beta\nclear \"a\"\nclear \"b\"\ncomplex --> complex\n");
    let d: Vec<_> = l
        .diagnostics
        .iter()
        .map(|d| (d.severity, d.message.as_str()))
        .collect();
    assert_eq!(
        d,
        [
            (
                Severity::Warning,
                "domain `clear` is declared again; its earlier items are dropped"
            ),
            (
                Severity::Warning,
                "a transition from `complex` to itself is not drawn"
            ),
        ]
    );
    let Diagram::Cynefin(c) = l.diagram else {
        panic!("not a Cynefin diagram")
    };
    assert_eq!(c.items(Domain::Clear), ["b"]);
    assert!(c.transitions.is_empty());
}

/// MCYN-3: every domain is named, items are drawn, and the area grows for long items.
#[test]
fn drawing() {
    let texts = |src: &str| {
        let s = diagram::lower(src).diagram.scene(&Approx, &Theme::LIGHT);
        let t: Vec<String> = s
            .elements
            .iter()
            .filter_map(|e| match &e.primitive {
                Primitive::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        (s.width, t)
    };
    let (w, t) = texts("cynefin-beta\ntitle T\ncomplex \"Probe it\"\n");
    assert_eq!(w, 880.0);
    for want in [
        "T",
        "Complex",
        "Complicated",
        "Clear",
        "Chaotic",
        "Confusion",
        "Probe it",
        "Emergent Practices",
        "Probe -> Sense -> Respond",
    ] {
        assert!(t.iter().any(|x| x == want), "{want} in {t:?}");
    }
    let long = "x".repeat(120);
    let (w, _) = texts(&format!("cynefin-beta\ncomplex \"{long}\"\n"));
    assert!(w > 880.0, "{w}");
    let (_, t) =
        texts("---\nconfig:\n  cynefin:\n    showDomainDescriptions: false\n---\ncynefin-beta\n");
    assert!(!t.iter().any(|x| x.contains("Practices")), "{t:?}");
}
