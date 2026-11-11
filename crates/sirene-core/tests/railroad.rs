//! Railroad diagrams (MRAIL-*): the fixture corpus and its drawings (LANGS-6), and the model
//! each notation builds.

use std::fs;
use std::path::Path;

use sirene_core::ascii::{Charset, Options};
use sirene_core::diagram::{self, Diagram, Kind};
use sirene_core::railroad::{self, Expr, Notation, Railroad};
use sirene_core::scene::{Approx, Primitive, Theme};

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/railroad");
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
        assert_eq!(railroad::parse(&src).syntax().to_string(), src, "{name}");
        assert_eq!(Kind::of(&src), Kind::Railroad, "{name}");
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

fn lowered(src: &str) -> Railroad {
    let lowered = diagram::lower(src);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    match lowered.diagram {
        Diagram::Railroad(r) => r,
        other => panic!("not a railroad diagram: {other:?}"),
    }
}

/// MRAIL-1, MRAIL-2: each header picks its notation, and all four build the same model.
#[test]
fn notations() {
    let same = [
        "railroad-ebnf-beta\nr = \"a\" ( b | c )* ;\n",
        "railroad-abnf-beta\nr = \"a\" *( b / c ) ;\n",
        "railroad-peg-beta\nr <- \"a\" ( b / c )* ;\n",
        "railroad-beta\nr = sequence(terminal(\"a\"), zeroOrMore(choice(nonterminal(\"b\"), \
         nonterminal(\"c\")))) ;\n",
    ];
    let notations = [Notation::Ebnf, Notation::Abnf, Notation::Peg, Notation::Ir];
    let first = lowered(same[0]).rules;
    for (src, notation) in same.iter().zip(notations) {
        let r = lowered(src);
        assert_eq!(r.notation, notation);
        assert_eq!(r.rules, first, "{src}");
    }
    assert!(matches!(
        &first[0].definition,
        Expr::Sequence(items) if matches!(items[1], Expr::Repeat { min: 0, .. })
    ));
}

/// MRAIL-1: errors, each reported once per rule, and duplicate rules.
#[test]
fn problems() {
    let messages = |src: &str| -> Vec<String> {
        diagram::lower(src)
            .diagnostics
            .into_iter()
            .map(|d| d.message)
            .collect()
    };
    assert_eq!(
        messages("railroad-ebnf-beta\na = b\nc = d ;\n"),
        ["expected `;` to end the rule"]
    );
    assert_eq!(
        messages("railroad-ebnf-beta\na = b ;\na = c ;\n"),
        ["rule `a` is already defined; both are drawn"]
    );
}

/// MRAIL-3: a rule's name, its boxes, and its markers.
#[test]
fn drawing() {
    let s = diagram::lower("railroad-ebnf-beta\nr = \"a\" b ;\n")
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
    assert_eq!(texts, ["a", "b", "r ="]);
    assert!(s.width > 0.0 && s.height > 0.0);
}
