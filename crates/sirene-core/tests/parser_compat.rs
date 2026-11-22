//! Characterization of parser syntax, diagnostics and lowering before shared scanner changes.

use std::fmt::Write;

use sirene_core::Diagnostic;
use sirene_core::syntax::SyntaxNode;
use sirene_core::{c4, flowchart, usecase, zenuml};

fn record(
    out: &mut String,
    name: &str,
    src: &str,
    tree: SyntaxNode,
    parse: &[Diagnostic],
    lower: &[Diagnostic],
) {
    assert_eq!(tree.to_string(), src, "{name}");
    writeln!(out, "=== {name} ===\nsource: {src:?}\nsyntax:").unwrap();
    for element in tree.descendants_with_tokens() {
        match element {
            sirene_core::syntax::SyntaxElement::Node(node) => {
                writeln!(out, "node {:?} {:?}", node.kind(), node.text_range()).unwrap();
            }
            sirene_core::syntax::SyntaxElement::Token(token) => {
                writeln!(
                    out,
                    "token {:?} {:?} {:?}",
                    token.kind(),
                    token.text_range(),
                    token.text()
                )
                .unwrap();
            }
        }
    }
    writeln!(
        out,
        "parse diagnostics: {parse:#?}\nlower diagnostics: {lower:#?}\n"
    )
    .unwrap();
}

#[test]
fn parser_and_lowering_compatibility() {
    let mut out = String::new();
    for (name, src) in [
        (
            "c4_keywords",
            "C4Context\ntitleé(x)\ntitle_name(x)\ntitle-name(x)\naccTitleX(x)\nPerson(a, \"A\")\nBad(x)\n",
        ),
        (
            "c4_titles",
            "C4Context\r\ntitle\r\ntitle  spaced  \r\ntitle hello %% comment\r\ntitle a; Person(x, \"X\")\r\ntitle",
        ),
        (
            "c4_recovery",
            "C4Context\nPerson(a, \"A\")\nPerson(b, $foo)\nPerson(c, \"C\")\nRel(c, missing, \"uses\")\n",
        ),
    ] {
        let p = c4::parse(src);
        let (model, diagnostics) = p.lower();
        record(
            &mut out,
            name,
            src,
            p.syntax(),
            p.diagnostics(),
            &diagnostics,
        );
        writeln!(out, "model: {model:#?}\n").unwrap();
    }
    for (name, src) in [
        (
            "zen_keywords",
            "zenuml\nifx\nif_2\nifé\nif-x\nnewbie\nreturnValue\nA.call()\n",
        ),
        (
            "zen_recovery",
            "zenuml\nif(x) {\nA.call(\nB->C: later\n}\nA.call()\n",
        ),
    ] {
        let p = zenuml::parse(src);
        let (model, diagnostics) = p.lower();
        record(
            &mut out,
            name,
            src,
            p.syntax(),
            p.diagnostics(),
            &diagnostics,
        );
        writeln!(out, "model: {model:#?}\n").unwrap();
    }
    for (name, src) in [
        (
            "flow_metadata",
            "flowchart TD\nA@{ label: \"café } { x\", shape: rect }\nB@{ label: \"unclosed\nC@{ label: \"ok }\" }\nD --> E\n",
        ),
        (
            "flow_metadata_repeated",
            "flowchart TD\nA@{ label: \"x }\" }\nB@{ label: \"y }\" }\nC@{ bad\nD@{ bad\nE --> F\n",
        ),
    ] {
        let p = flowchart::parse(src);
        let lowered = p.lower();
        record(
            &mut out,
            name,
            src,
            p.syntax(),
            p.diagnostics(),
            &lowered.diagnostics,
        );
        writeln!(out, "model: {:#?}\n", lowered.flowchart).unwrap();
    }
    for (name, src) in [
        (
            "usecase_metadata",
            "usecase-beta\nactor A@{ label: \"café }\", type: hollow }\njson J@{ \"a\": {\"b\": \"}\"} }\nU(\"Use\")\n",
        ),
        (
            "usecase_unclosed",
            "usecase-beta\nactor A@{ type: \"unclosed\nactor B@{ type: \"hollow\" }\nU(\"later\")\n",
        ),
    ] {
        let p = usecase::parse(src);
        let lowered = p.lower();
        record(
            &mut out,
            name,
            src,
            p.syntax(),
            p.diagnostics(),
            &lowered.diagnostics,
        );
        writeln!(out, "model: {:#?}\n", lowered.flowchart).unwrap();
    }
    insta::assert_snapshot!(out);
}
