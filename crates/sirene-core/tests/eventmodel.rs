//! Event modeling diagrams (MEVM-*): the fixture corpus and its drawings (LANGS-6), and the
//! model each source builds.

use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind};
use sirene_core::eventmodel::{self, EntityType, EventModel, Group};
use sirene_core::scene::{Approx, Primitive, Segment, Theme};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/eventmodel");
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
        assert_eq!(eventmodel::parse(&src).syntax().to_string(), src, "{name}");
        assert_eq!(Kind::of(&src), Kind::EventModel, "{name}");
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

fn lowered(src: &str) -> EventModel {
    let lowered = diagram::lower(src);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    match lowered.diagram {
        Diagram::EventModel(c) => c,
        other => panic!("not an event modeling diagram: {other:?}"),
    }
}

fn relations(c: &EventModel) -> Vec<(&str, &str)> {
    c.relations
        .iter()
        .map(|r| (c.frames[r.from].id.as_str(), c.frames[r.to].id.as_str()))
        .collect()
}

/// MEVM-2: frames, data, lanes, and inferred relations.
#[test]
fn model() {
    let c = lowered(
        "eventmodeling\ntitle T\ntf 01 ui CartUI\ntimeframe 02 command AddItem [[D]]\n\
         tf 03 evt Cart.ItemAdded { a: 1 }\ntf 04 evt Cart.ItemAdded\ntf 05 pcr P\n\
         data D {\n  x: 1\n    y: 2\n}\n",
    );
    assert_eq!(c.title.as_deref(), Some("T"));
    let f: Vec<_> = c
        .frames
        .iter()
        .map(|f| (f.id.as_str(), f.kind, f.name.as_str(), f.data.clone()))
        .collect();
    assert_eq!(
        f,
        [
            ("01", EntityType::Ui, "CartUI", vec![]),
            (
                "02",
                EntityType::Command,
                "AddItem",
                vec!["x: 1".to_owned(), "  y: 2".to_owned()]
            ),
            (
                "03",
                EntityType::Event,
                "ItemAdded",
                vec!["a: 1".to_owned()]
            ),
            ("04", EntityType::Event, "ItemAdded", vec![]),
            ("05", EntityType::Processor, "P", vec![]),
        ]
    );
    let lanes: Vec<String> = c.lanes.iter().map(|l| l.label()).collect();
    assert_eq!(
        lanes,
        ["UI/Automation", "Command/Read Model", "Stream: Cart"]
    );
    assert_eq!(c.lanes[2].group, Group::Events);
    // 04 is in 03's lane, so its source is the latest frame in another lane, 02.
    assert_eq!(
        relations(&c),
        [("01", "02"), ("02", "03"), ("02", "04"), ("04", "05")]
    );
}

/// MEVM-2: lanes are grouped, each group's own lane first, then namespaces as they appear.
#[test]
fn lane_order() {
    let c = lowered(
        "eventmodeling\nrf 01 evt B.E\nrf 02 evt A.E\nrf 03 evt E\nrf 04 ui X.U\nrf 05 cmd C\n\
         rf 06 evt B.F\nrf 07 rmo A.B.C\n",
    );
    let lanes: Vec<String> = c.lanes.iter().map(|l| l.label()).collect();
    assert_eq!(
        lanes,
        [
            "UI/A: X",
            "Command/Read Model",
            "Events",
            "Stream: B",
            "Stream: A"
        ]
    );
    // A name with two dots has no namespace.
    assert_eq!(c.frames[6].name, "A.B.C");
    assert_eq!(c.frames[0].lane, c.frames[5].lane);
    assert!(c.relations.is_empty(), "reset frames infer nothing");
}

/// MEVM-2: explicit sources replace the inferred one, in frame order, on reset frames too.
#[test]
fn explicit_relations() {
    let c = lowered(
        "eventmodeling\nrf 02 evt A\nrf 03 evt B\ntf 01 rmo M ->> 03 ->> 02 ->> 03\n\
         tf 04 ui U\nrf 05 rmo N ->> 02\n",
    );
    assert_eq!(
        relations(&c),
        [("02", "01"), ("03", "01"), ("01", "04"), ("02", "05")]
    );
}

/// MEVM-2: repeated frame numbers, undefined references, and sources of the wrong type are
/// errors; a frame's relation to itself is a warning.
#[test]
fn diagnostics() {
    let l = diagram::lower(
        "eventmodeling\ntf 01 ui A\ntf 01 cmd B\ntf 02 cmd C [[Missing]]\ntf 03 evt D ->> 01\n\
         tf 04 rmo E ->> 09 ->> 04 ->> 03\nnote 08 {\n}\ngwt 07 given evt D then evt D\n",
    );
    let d: Vec<_> = l
        .diagnostics
        .iter()
        .map(|d| (d.severity, d.message.as_str()))
        .collect();
    assert_eq!(
        d,
        [
            (
                Severity::Error,
                "frame `01` is defined again; each frame needs its own number"
            ),
            (Severity::Error, "frame `08` is not defined"),
            (Severity::Error, "frame `07` is not defined"),
            (Severity::Error, "data block `Missing` is not defined"),
            (
                Severity::Error,
                "an event can only receive input from a command, not from `ui`"
            ),
            (Severity::Error, "frame `09` is not defined"),
            (
                Severity::Warning,
                "a relation from frame `04` to itself is not drawn"
            ),
        ]
    );
    let Diagram::EventModel(c) = l.diagram else {
        panic!("not an event modeling diagram")
    };
    assert_eq!(c.frames.len(), 4);
    assert_eq!(relations(&c), [("01", "02"), ("01", "03"), ("03", "04")]);
}

/// MEVM-3: lanes and boxes are drawn with their text, boxes advance along the timeline, and
/// each relation has an arrowhead.
#[test]
fn drawing() {
    let s = diagram::lower(
        "eventmodeling\ntitle Cart\ntf 01 ui CartUI\ntf 02 cmd AddItem { description: string }\n\
         tf 03 evt ItemAdded\ntf 04 evt Second\n",
    )
    .diagram
    .scene(&Approx, &Theme::LIGHT);
    let texts: Vec<&str> = s
        .elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    for want in [
        "Cart",
        "UI/Automation",
        "Command/Read Model",
        "Events",
        "CartUI",
        "AddItem",
        "description: string",
        "ItemAdded",
    ] {
        assert!(texts.contains(&want), "{want} in {texts:?}");
    }
    // Box rectangles: the filled paths after the three lanes, by their left edges.
    let lefts: Vec<f32> = s
        .elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Path {
                segments,
                fill: Some(_),
                stroke: Some(_),
            } => segments.iter().find_map(|g| match g {
                Segment::Move(p) => Some(p.x),
                _ => None,
            }),
            _ => None,
        })
        .skip(3)
        .collect();
    assert_eq!(lefts.len(), 4);
    assert!(lefts.windows(2).all(|w| w[0] < w[1]), "{lefts:?}");
    let arrowheads = s
        .elements
        .iter()
        .filter(|e| {
            matches!(
                &e.primitive,
                Primitive::Path {
                    fill: Some(_),
                    stroke: None,
                    ..
                }
            )
        })
        .count();
    assert_eq!(arrowheads, 3);
}
