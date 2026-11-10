//! Ishikawa (fishbone) diagrams (MISH-*): the fixture corpus and its drawings (LANGS-6), and the
//! tree each source builds.

use std::fs;
use std::path::Path;

use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind};
use sirene_core::ishikawa::{self, Cause, Ishikawa};
use sirene_core::scene::{Approx, Primitive, TextMetrics, Theme};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ishikawa");
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
        assert_eq!(ishikawa::parse(&src).syntax().to_string(), src, "{name}");
        assert_eq!(Kind::of(&src), Kind::Ishikawa, "{name}");
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

fn lowered(src: &str) -> Ishikawa {
    let lowered = diagram::lower(src);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    match lowered.diagram {
        Diagram::Ishikawa(i) => i,
        other => panic!("not an ishikawa diagram: {other:?}"),
    }
}

fn shape(c: &Cause) -> String {
    let label = c.lines.join("/");
    if c.children.is_empty() {
        label
    } else {
        let kids: Vec<String> = c.children.iter().map(shape).collect();
        format!("{label}({})", kids.join(","))
    }
}

/// MISH-1, MISH-2: the first line is the effect; causes nest by indentation relative to the
/// first cause; `<br>` breaks lines and entities are decoded.
#[test]
fn model() {
    let c = lowered("ishikawa-beta\n    E\n  a\n    b\n      c\n    d\n  e<br>f\n");
    assert_eq!(shape(c.effect.as_ref().unwrap()), "E(a(b(c),d),e/f)");
    let c = lowered("---\ntitle: T\n---\nishikawa X &amp; Y\nA\n");
    assert_eq!(c.title.as_deref(), Some("T"));
    assert_eq!(shape(c.effect.as_ref().unwrap()), "X & Y(A)");
    // Every line is a cause, even one that looks like a keyword.
    let c = lowered("ishikawa\nE\n  title x\n");
    assert_eq!(shape(c.effect.as_ref().unwrap()), "E(title x)");
    assert_eq!(lowered("ishikawa-beta\n").effect, None);
}

fn texts(src: &str) -> Vec<(String, f32, f32)> {
    let s = diagram::lower(src).diagram.scene(&Approx, &Theme::LIGHT);
    s.elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Text { text, center, .. } => Some((text.clone(), center.x, center.y)),
            _ => None,
        })
        .collect()
}

/// MISH-3: the head at the right, causes alternating above and below the spine, sub-causes to
/// the left of their bones, and long labels wrapped.
#[test]
fn drawing() {
    let t = texts("ishikawa\nEffect\n  Up\n    Sub cause that is long\n  Down\n");
    let find = |s: &str| t.iter().find(|x| x.0 == s).cloned().unwrap();
    let head = find("Effect");
    let up = find("Up");
    let down = find("Down");
    assert!(up.2 < head.2 && head.2 < down.2, "{t:?}");
    assert!(up.1 < head.1 && down.1 < head.1, "{t:?}");
    let sub = find("Sub cause that");
    assert!(t.iter().any(|x| x.0 == "is long"), "{t:?}");
    assert!(sub.1 < up.1 + 40.0 && sub.2 > up.2, "{t:?}");
    // A cause with no sub-causes sits beside the spine, its box clear of the head and of the
    // pair to its right.
    let t = texts("ishikawa\nEffect\n  Alpha\n  Beta gamma\n  Delta\n  Epsilon\n");
    let find = |s: &str| t.iter().find(|x| x.0 == s).cloned().unwrap();
    let right = |s: &str| find(s).1 + Approx.width(s, 14.0) / 2.0 + 20.0;
    let left = |s: &str| find(s).1 - Approx.width(s, 14.0) / 2.0 - 20.0;
    let head = find("Effect");
    let head_left = head.1 - (Approx.width("Effect", 14.0) + 6.0).max(60.0) / 2.0 - 3.0;
    assert!(
        right("Alpha") < head_left && right("Beta gamma") < head_left,
        "{t:?}"
    );
    assert!(
        right("Delta") < left("Alpha").min(left("Beta gamma")),
        "{t:?}"
    );
    assert!(
        right("Epsilon") < left("Alpha").min(left("Beta gamma")),
        "{t:?}"
    );
    assert_eq!(texts("ishikawa-beta\n"), []);
    let s = diagram::lower("ishikawa\nOnly\n")
        .diagram
        .scene(&Approx, &Theme::LIGHT);
    assert!(s.width > 0.0 && s.height > 0.0);
}
