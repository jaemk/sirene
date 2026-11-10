//! Mermaid packet diagrams (MPKT-*): the fixture corpus with its diagnostics and drawings
//! (LANGS-6), and the model each field builds.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind, Lowered};
use sirene_core::packet::{self, Packet};
use sirene_core::scene::{Approx, Owner, Primitive, Pt, Scene, Theme};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/packet");
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

fn diagnostics(src: &str, lowered: &Lowered) -> String {
    let mut out = String::new();
    for d in &lowered.diagnostics {
        let (line, col) = d.line_col(src);
        let severity = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        writeln!(out, "{line}:{col}: {severity}: {}", d.message).unwrap();
    }
    out
}

/// Each fixture's diagnostics and drawing; the tree always holds the source exactly.
#[test]
fn snapshots() {
    for dir in ["valid", "errors"] {
        for (name, src) in fixtures(dir) {
            assert_eq!(packet::parse(&src).syntax().to_string(), src, "{name}");
            assert_eq!(Kind::of(&src), Kind::Packet, "{name}");
            let lowered = diagram::lower(&src);
            if dir == "valid" {
                assert!(
                    lowered.diagnostics.is_empty(),
                    "{name}: {:?}",
                    lowered.diagnostics
                );
            } else {
                assert!(!lowered.diagnostics.is_empty(), "{name}");
            }
            let mut out = diagnostics(&src, &lowered);
            out.push('\n');
            out.push_str(&lowered.diagram.ascii(&Options {
                charset: Charset::Unicode,
            }));
            insta::assert_snapshot!(format!("{dir}__{name}"), out, &src);
            for theme in [Theme::LIGHT, Theme::DARK] {
                let s = lowered.diagram.scene(&Approx, &theme);
                assert!(!s.elements.is_empty(), "{name}");
                assert!(s.elements.iter().all(|e| e.owner == Owner::Diagram));
            }
        }
    }
}

#[test]
fn ascii_charset_snapshots() {
    for (name, src) in fixtures("valid") {
        let out = diagram::lower(&src).diagram.ascii(&Options {
            charset: Charset::Ascii,
        });
        insta::assert_snapshot!(format!("ascii__{name}"), out, &src);
    }
}

fn lowered(src: &str) -> (Packet, Vec<(Severity, String)>) {
    let lowered = diagram::lower(src);
    let d = lowered
        .diagnostics
        .iter()
        .map(|d| (d.severity, d.message.clone()))
        .collect();
    match lowered.diagram {
        Diagram::Packet(p) => (p, d),
        other => panic!("not a packet: {other:?}"),
    }
}

fn fields(p: &Packet) -> Vec<(u32, u32, &str)> {
    p.fields
        .iter()
        .map(|f| (f.start, f.end, f.label.as_str()))
        .collect()
}

/// MPKT-1, MPKT-2: ranges, single bits, and counts running on from the field before.
#[test]
fn bits() {
    let (p, d) = lowered(
        "---\ntitle: Front\nconfig:\n  packet:\n    bitsPerRow: 8\n    showBits: false\n---\npacket\n  accTitle: at\n  +4: \"a\"\n  4-6: \"b\"\n  7: \"c\"\n  +3: \"d\"\n",
    );
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(p.title.as_deref(), Some("Front"));
    assert_eq!(p.acc_title.as_deref(), Some("at"));
    assert_eq!(
        (p.bits_per_row, p.show_bits, p.descending),
        (8, false, false)
    );
    assert_eq!(
        fields(&p),
        [(0, 3, "a"), (4, 6, "b"), (7, 7, "c"), (8, 10, "d")]
    );
    // A `title` statement wins over frontmatter.
    let (p, _) = lowered("---\ntitle: Front\n---\npacket\n  title Body\n");
    assert_eq!(p.title.as_deref(), Some("Body"));
}

/// MPKT-2: a gap is an error and drawn empty; an overlap, a backward range, and a field of no
/// bits are errors and left out; a bad config value is a warning.
#[test]
fn errors() {
    let (p, d) = lowered(
        "---\nconfig:\n  packet:\n    bitOrder: sideways\n---\npacket\n  0-3: \"a\"\n  6-7: \"gap\"\n  7-9: \"overlap\"\n  9-8: \"back\"\n  +0: \"none\"\n  10000: \"far\"\n",
    );
    assert_eq!(
        d,
        [
            (
                Severity::Warning,
                "ignoring `bitOrder: sideways`: not a valid value".to_owned()
            ),
            (
                Severity::Error,
                "`6-7` does not follow the field before it; expected it to start at bit 4"
                    .to_owned()
            ),
            (
                Severity::Error,
                "`7-9` does not follow the field before it; expected it to start at bit 8"
                    .to_owned()
            ),
            (Severity::Error, "`9-8` ends before it starts".to_owned()),
            (
                Severity::Error,
                "`+0` has no bits; a field holds at least one".to_owned()
            ),
            (
                Severity::Error,
                "`10000` runs past bit 9999, the last a packet draws".to_owned()
            ),
        ]
    );
    assert_eq!(fields(&p), [(0, 3, "a"), (6, 7, "gap")]);
}

#[test]
fn syntax_errors() {
    let errors = |src: &str| -> Vec<String> {
        diagram::lower(src)
            .diagnostics
            .into_iter()
            .map(|d| d.message)
            .collect()
    };
    assert_eq!(
        errors("packet\n  0: x\n"),
        ["expected a quoted label such as `\"Source Port\"`"]
    );
    assert!(errors("packet\n").is_empty());
}

fn texts(s: &Scene) -> Vec<(String, Pt)> {
    s.elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Text { text, center, .. } => Some((text.clone(), *center)),
            _ => None,
        })
        .collect()
}

/// MPKT-3: fields across their bits in rows, a field split where a row ends, bit numbers over
/// the ends, and each row mirrored when descending.
#[test]
fn drawing() {
    let src =
        "---\nconfig:\n  packet:\n    bitsPerRow: 8\n---\npacket\n  0-3: \"A\"\n  4-11: \"B\"\n";
    let s = diagram::lower(src).diagram.scene(&Approx, &Theme::LIGHT);
    let t = texts(&s);
    let all = |want: &str| -> Vec<Pt> {
        t.iter()
            .filter(|(text, _)| text == want)
            .map(|&(_, p)| p)
            .collect()
    };
    let (a, b) = (all("A")[0], all("B"));
    assert_eq!(b.len(), 2, "B spans two rows: {t:?}");
    assert!(a.x < b[0].x && (a.y - b[0].y).abs() < 0.01 && b[1].y > b[0].y);
    // B's second part, bits 8 to 11, sits under A's bits 0 to 3.
    assert!((b[1].x - a.x).abs() < 0.01);
    let (zero, three) = (all("0")[0], all("3")[0]);
    assert!(zero.y < a.y && zero.x < three.x && three.x < b[0].x);

    let s =
        diagram::lower(&src.replace("bitsPerRow: 8", "bitsPerRow: 8\n    bitOrder: descending"))
            .diagram
            .scene(&Approx, &Theme::LIGHT);
    let t = texts(&s);
    let at = |want: &str| t.iter().find(|(x, _)| x == want).map(|&(_, p)| p).unwrap();
    assert!(at("A").x > at("B").x && at("0").x > at("3").x);
}
