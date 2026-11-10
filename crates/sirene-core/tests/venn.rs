//! Venn diagrams (MVENN-*): the fixture corpus and its drawings (LANGS-6), and the model each
//! source builds.

use std::fs;
use std::path::Path;

use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind};
use sirene_core::scene::{Approx, Color, Primitive, Theme};
use sirene_core::venn::{self, Venn};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/venn");
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
        assert_eq!(venn::parse(&src).syntax().to_string(), src, "{name}");
        assert_eq!(Kind::of(&src), Kind::Venn, "{name}");
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

fn lowered(src: &str) -> Venn {
    let lowered = diagram::lower(src);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    match lowered.diagram {
        Diagram::Venn(v) => v,
        other => panic!("not a venn diagram: {other:?}"),
    }
}

fn errors(src: &str) -> Vec<String> {
    diagram::lower(src)
        .diagnostics
        .into_iter()
        .map(|d| d.message)
        .collect()
}

/// MVENN-1, MVENN-2: sets, unions, sizes, labels, text nodes, and styles.
#[test]
fn model() {
    let v = lowered(
        "venn-beta\n  title \"T\"\n  set B[\"Beta\"]:12\n  set \"A x\"\n    text a1\n  \
         union B,\"A x\"[AB]\n    text ab[\"Both\"]\n  union \"A x\",B,B\n  text B q\n  \
         style B,\"A x\" fill:#f00, color: red\n",
    );
    assert_eq!(v.title.as_deref(), Some("T"));
    let subsets: Vec<(Vec<&str>, f64, Option<&str>)> = v
        .subsets
        .iter()
        .map(|s| {
            (
                s.sets.iter().map(String::as_str).collect(),
                s.size,
                s.label.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        subsets,
        [
            (vec!["B"], 12.0, Some("Beta")),
            (vec!["A x"], 10.0, None),
            (vec!["A x", "B"], 2.5, Some("AB")),
            (vec!["A x", "B", "B"], 10.0 / 9.0, None),
        ]
    );
    let texts: Vec<(String, &str)> = v
        .texts
        .iter()
        .map(|t| (t.sets.join("|"), t.text()))
        .collect();
    assert_eq!(
        texts,
        [
            ("A x".to_owned(), "a1"),
            ("A x|B".to_owned(), "Both"),
            ("B".to_owned(), "q"),
        ]
    );
    let key = vec!["A x".to_owned(), "B".to_owned()];
    assert_eq!(v.style(&key, "fill"), Some("#f00"));
    assert_eq!(v.style(&key, "color"), Some("red"));
    assert_eq!(v.name("B"), "Beta");
    assert_eq!(v.name("A x"), "A x");
    // A three-set union implies its pairs, a quarter of the smaller set.
    let v = lowered("venn-beta\nset A:8\nset B:4\nset C\nunion A,B\nunion A,B,C\n");
    let implied: Vec<(String, f64)> = v.regions()[5..]
        .iter()
        .map(|s| (s.sets.join("|"), s.size))
        .collect();
    assert_eq!(implied, [("A|C".to_owned(), 2.0), ("B|C".to_owned(), 1.0)]);
}

/// MVENN-2: a union names sets declared before it.
#[test]
fn unknown_sets() {
    assert_eq!(
        errors("venn-beta\nset A\nunion A,B\n"),
        ["unknown set `B`: declare it with `set` first"]
    );
    assert_eq!(
        errors("venn-beta\nunion A,B\nset A\nset B\n"),
        ["unknown set `A`, `B`: declare it with `set` first"]
    );
    assert_eq!(
        errors("venn-beta\nset A\nunion A\n"),
        ["a union needs two or more sets"]
    );
}

fn texts(src: &str) -> Vec<(String, f32, f32, Color)> {
    let s = diagram::lower(src).diagram.scene(&Approx, &Theme::LIGHT);
    s.elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Text {
                text,
                center,
                color,
                ..
            } => Some((text.clone(), center.x, center.y, *color)),
            _ => None,
        })
        .collect()
}

/// MVENN-3, MVENN-4: circles side by side with the overlap's label between them, text nodes
/// below their region's label, and styles applied.
#[test]
fn drawing() {
    let t = texts(
        "venn-beta\ntitle T\nset A\n  text a1\nset B\nunion A,B[\"AB\"]\nstyle A1 color:red\n\
         style a1 color:#00ff00\nstyle A,B color:#0000ff\n",
    );
    let find = |s: &str| t.iter().find(|x| x.0 == s).cloned().unwrap();
    let (title, a, b, ab, a1) = (find("T"), find("A"), find("B"), find("AB"), find("a1"));
    assert!(title.2 < a.2, "{t:?}");
    assert!(a.1 < ab.1 && ab.1 < b.1, "{t:?}");
    assert!(
        (a.2 - b.2).abs() < 1.0 && (ab.1 - 400.0).abs() < 1.0,
        "{t:?}"
    );
    assert!(a1.2 > a.2 && (a1.1 - a.1).abs() < 1.0, "{t:?}");
    assert_eq!(a1.3, Color::rgb(0x00ff00));
    assert_eq!(ab.3, Color::rgb(0x0000ff));
    let s = diagram::lower("venn-beta\n")
        .diagram
        .scene(&Approx, &Theme::LIGHT);
    assert!(s.width > 0.0 && s.elements.is_empty());
}
