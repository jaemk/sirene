//! Wardley maps (MWARD-*): the fixture corpus and its drawings (LANGS-6), and the model each
//! source builds.

use std::fs;
use std::path::Path;

use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind};
use sirene_core::scene::{Approx, Primitive, Theme};
use sirene_core::wardley::{self, Flow, NodeKind, Strategy, Wardley};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/wardley");
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
        assert_eq!(wardley::parse(&src).syntax().to_string(), src, "{name}");
        assert_eq!(Kind::of(&src), Kind::Wardley, "{name}");
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

fn lowered(src: &str) -> Wardley {
    let lowered = diagram::lower(src);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    match lowered.diagram {
        Diagram::Wardley(w) => w,
        other => panic!("not a Wardley map: {other:?}"),
    }
}

fn errors(src: &str) -> Vec<String> {
    diagram::lower(src)
        .diagnostics
        .into_iter()
        .map(|d| d.message)
        .collect()
}

/// MWARD-1, MWARD-2: components, decorators, stages, links, and evolve trends.
#[test]
fn model() {
    let w = lowered(
        "wardley-beta\ntitle T\nsize [400, 300]\n\
         evolution Genesis / Concept@0.2 -> Custom@0.6 -> Product\n\
         anchor User [0.9, 0.5]\ncomponent \"Web App\" [80, 30] label [5, -5] (buy) (inertia)\n\
         component B [0.5, 0.5] (market)\nUser -> Web App\nB +<> \"Web App\"; feeds\n\
         User -.-> B\nevolve B 0.75\n",
    );
    assert_eq!(w.title.as_deref(), Some("T"));
    assert_eq!(w.size, Some((400.0, 300.0)));
    assert_eq!(w.stage_names(), ["Genesis / Concept", "Custom", "Product"]);
    let nodes: Vec<_> = w
        .nodes
        .iter()
        .map(|n| (n.label.as_str(), n.x, n.y, n.kind, n.strategy, n.inertia))
        .collect();
    assert_eq!(
        nodes,
        [
            ("User", 50.0, 90.0, NodeKind::Anchor, None, false),
            (
                "Web App",
                30.0,
                80.0,
                NodeKind::Component,
                Some(Strategy::Buy),
                true
            ),
            (
                "B",
                50.0,
                50.0,
                NodeKind::Component,
                Some(Strategy::Market),
                false
            ),
        ]
    );
    assert_eq!(w.nodes[1].label_offset, Some((5.0, -5.0)));
    let links: Vec<_> = w
        .links
        .iter()
        .map(|l| (l.source, l.target, l.dashed, l.label.as_deref(), l.flow))
        .collect();
    assert_eq!(
        links,
        [
            (0, 1, false, None, None),
            (2, 1, false, Some("feeds"), Some(Flow::Both)),
            (0, 2, true, None, None),
        ]
    );
    assert_eq!(w.trends.len(), 1);
    assert_eq!((w.trends[0].node, w.trends[0].x), (2, 75.0));
}

/// MWARD-2: pipelines hang components off a parent at its visibility.
#[test]
fn pipelines() {
    let w = lowered(
        "wardley-beta\ncomponent P [0.6, 0.4]\npipeline P {\n  component X [0.2]\n  \
         component \"Y z\" [0.8]\n}\n",
    );
    assert_eq!(w.pipelines.len(), 1);
    let p = &w.pipelines[0];
    assert!(w.nodes[p.parent].pipeline_parent);
    let parts: Vec<(&str, f64, f64)> = p
        .components
        .iter()
        .map(|&i| (w.nodes[i].label.as_str(), w.nodes[i].x, w.nodes[i].y))
        .collect();
    assert_eq!(parts, [("X", 20.0, 60.0), ("Y z", 80.0, 60.0)]);
}

/// MWARD-3: out-of-range coordinates and unknown names.
#[test]
fn problems() {
    assert_eq!(
        errors("wardley-beta\ncomponent A [150, 0.5]\n"),
        ["component's visibility must be from 0 to 1, or a percentage up to 100; got 150"]
    );
    // Above 1 is already a percentage.
    assert_eq!(
        lowered("wardley-beta\ncomponent A [1.5, 0.5]\n").nodes[0].y,
        1.5
    );
    assert_eq!(
        errors("wardley-beta\ncomponent A [0.5, 0.5]\nA -> B\nevolve C 0.9\n"),
        [
            "unknown component `B`; the link is not drawn",
            "unknown component `C`; ignoring the `evolve`",
        ]
    );
    assert_eq!(
        errors("wardley-beta\npipeline P {\n}\n"),
        ["unknown component `P`: declare it before its pipeline"]
    );
}

/// MWARD-4: a component's circle sits at its evolution across and visibility up.
#[test]
fn drawing() {
    let s = diagram::lower("wardley-beta\ntitle T\ncomponent A [0.5, 0.25]\n")
        .diagram
        .scene(&Approx, &Theme::LIGHT);
    assert_eq!((s.width, s.height), (900.0, 600.0));
    let texts: Vec<&str> = s
        .elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    for want in ["T", "Evolution", "Visibility", "Genesis", "Commodity", "A"] {
        assert!(texts.contains(&want), "{want} in {texts:?}");
    }
}
