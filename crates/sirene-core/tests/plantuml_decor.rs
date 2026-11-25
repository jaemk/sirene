//! `header`, `footer`, `caption`, and `legend` (PCOM-1): what the parser reads, where the scene
//! and the text renderer draw them, and the formulas in them (LEMB-2, LEMB-4, LEMB-5).

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sirene_core::Severity;
use sirene_core::ascii::{Charset, Options};
use sirene_core::decor::{Banner, Decorations, HAlign, Legend, VAlign};
use sirene_core::diagram::{self, Diagram};
use sirene_core::scene::{Approx, Primitive, Scene, Theme};

fn fixture(dir: &str, name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/plantuml")
        .join(dir)
        .join(format!("{name}.puml"));
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// The decorations of `src`, which must have some.
fn decor(src: &str) -> Decorations {
    match diagram::lower(src).diagram {
        Diagram::Decorated(d) => d.decor,
        other => panic!("not decorated: {other:?}"),
    }
}

fn banner(lines: &[&str], align: HAlign) -> Option<Banner> {
    Some(Banner {
        lines: lines.iter().map(|s| (*s).to_owned()).collect(),
        align,
    })
}

fn scene(src: &str) -> Scene {
    diagram::lower(src)
        .diagram
        .scene(&Approx, &Theme::LIGHT)
        .unwrap()
}

fn ascii(src: &str, charset: Charset) -> String {
    diagram::lower(src)
        .diagram
        .ascii(&Options { charset })
        .unwrap()
}

/// The text runs of a scene with their centers.
fn texts(scene: &Scene) -> Vec<(&str, f32, f32)> {
    scene
        .elements
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Text { text, center, .. } => Some((text.as_str(), center.x, center.y)),
            _ => None,
        })
        .collect()
}

fn at(scene: &Scene, text: &str) -> (f32, f32) {
    let found: Vec<_> = texts(scene)
        .into_iter()
        .filter(|t| t.0 == text)
        .map(|t| (t.1, t.2))
        .collect();
    assert_eq!(found.len(), 1, "{text:?} in {:?}", texts(scene));
    found[0]
}

const SEQ: &str = "A -> B : hi\n";

fn wrap(decorations: &str) -> String {
    format!("@startuml\n{decorations}\n{SEQ}@enduml\n")
}

#[test]
fn single_line_forms_have_the_documented_default_positions() {
    let d = decor(&wrap("header H\nfooter F\ncaption C"));
    assert_eq!(d.header, banner(&["H"], HAlign::Right));
    assert_eq!(d.footer, banner(&["F"], HAlign::Center));
    assert_eq!(d.caption, banner(&["C"], HAlign::Center));
    assert_eq!(d.legend, None);
}

#[test]
fn a_colon_after_the_keyword_is_not_part_of_the_text() {
    let d = decor(&wrap("header: H\nfooter : F\ncaption: C"));
    assert_eq!(d.header, banner(&["H"], HAlign::Right));
    assert_eq!(d.footer, banner(&["F"], HAlign::Center));
    assert_eq!(d.caption, banner(&["C"], HAlign::Center));
}

#[test]
fn header_and_footer_take_a_position_before_their_keyword() {
    let d = decor(&wrap("left header H\nRight Footer F"));
    assert_eq!(d.header, banner(&["H"], HAlign::Left));
    assert_eq!(d.footer, banner(&["F"], HAlign::Right));
    let d = decor(&wrap("center header H\nleft footer F"));
    assert_eq!(d.header, banner(&["H"], HAlign::Center));
    assert_eq!(d.footer, banner(&["F"], HAlign::Left));
}

#[test]
fn blocks_run_to_their_closer() {
    for (open, close) in [
        ("header", "end header"),
        ("header", "endheader"),
        ("left header", "End  Header"),
    ] {
        let d = decor(&wrap(&format!("{open}\none\n**two**\n{close}")));
        let align = if open.starts_with("left") {
            HAlign::Left
        } else {
            HAlign::Right
        };
        assert_eq!(
            d.header,
            banner(&["one", "**two**"], align),
            "{open}/{close}"
        );
    }
    let d = decor(&wrap("center footer\nx\nend footer"));
    assert_eq!(d.footer, banner(&["x"], HAlign::Center));
    let d = decor(&wrap("footer left\nx\nendfooter"));
    assert_eq!(d.footer, banner(&["x"], HAlign::Left));
}

#[test]
fn backslash_n_breaks_a_line_outside_a_formula() {
    let d = decor(&wrap("header a\\nb\ncaption <latex>x \\neq y</latex>\\nz"));
    assert_eq!(d.header, banner(&["a", "b"], HAlign::Right));
    assert_eq!(
        d.caption,
        banner(&["<latex>x \\neq y</latex>", "z"], HAlign::Center)
    );
}

#[test]
fn legend_positions() {
    for (open, h, v) in [
        ("legend", HAlign::Center, VAlign::Bottom),
        ("legend left", HAlign::Left, VAlign::Bottom),
        ("legend right", HAlign::Right, VAlign::Bottom),
        ("legend center", HAlign::Center, VAlign::Bottom),
        ("legend top", HAlign::Center, VAlign::Top),
        ("legend bottom", HAlign::Center, VAlign::Bottom),
        ("legend top left", HAlign::Left, VAlign::Top),
        ("legend top right", HAlign::Right, VAlign::Top),
        ("legend bottom left", HAlign::Left, VAlign::Bottom),
        ("LEGEND Bottom Right", HAlign::Right, VAlign::Bottom),
    ] {
        for close in ["endlegend", "end legend"] {
            let d = decor(&wrap(&format!("{open}\na\nb\n{close}")));
            assert_eq!(
                d.legend,
                Some(Legend {
                    lines: vec!["a".into(), "b".into()],
                    horizontal: h,
                    vertical: v,
                }),
                "{open}/{close}"
            );
        }
    }
}

#[test]
fn the_last_of_each_kind_is_kept() {
    let d = decor(&wrap("header one\nheader two\nfooter a\nfooter b"));
    assert_eq!(d.header, banner(&["two"], HAlign::Right));
    assert_eq!(d.footer, banner(&["b"], HAlign::Center));
}

#[test]
fn line_read_kinds_take_blocks_and_positions_too() {
    let src =
        "@startuml\nclass A\nright header\nx\nend header\nfooter left\ny\nendfooter\n@enduml\n";
    let d = decor(src);
    assert_eq!(d.header, banner(&["x"], HAlign::Right));
    assert_eq!(d.footer, banner(&["y"], HAlign::Left));
}

/// The timing kind reads the shared statements, so it is wrapped like the others.
#[test]
fn a_timing_diagram_is_wrapped() {
    let src = fixture("valid", "timing").replace("@startuml\n", "@startuml\nheader H\n");
    let lowered = diagram::lower(&src);
    let Diagram::Decorated(d) = &lowered.diagram else {
        panic!("{:?}", lowered.diagram);
    };
    assert!(matches!(d.diagram, Diagram::Timing(_)));
    assert_eq!(d.decor.header, banner(&["H"], HAlign::Right));
}

#[test]
fn a_diagram_without_decorations_is_not_wrapped() {
    let lowered = diagram::lower(&wrap(""));
    assert!(matches!(lowered.diagram, Diagram::Sequence(_)));
    let lowered = diagram::lower("@startuml\nclass A\n@enduml\n");
    assert!(matches!(lowered.diagram, Diagram::Graph(_)));
}

#[test]
fn a_drawn_decoration_is_not_reported() {
    let d = diagram::lower(&wrap("header H\nlegend\nx\nend legend")).diagnostics;
    assert!(d.is_empty(), "{d:?}");
}

#[test]
fn an_unclosed_block_is_an_error_and_a_legend_with_text_is_not_drawn() {
    let lowered = diagram::lower("@startuml\nclass A\nlegend\nx\n@enduml\n");
    assert!(
        lowered
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error && d.message.contains("unclosed `legend`")),
        "{:?}",
        lowered.diagnostics
    );
    let lowered = diagram::lower("@startuml\nclass A\nlegend hello\n@enduml\n");
    assert!(matches!(lowered.diagram, Diagram::Graph(_)));
    let [d] = lowered.diagnostics.as_slice() else {
        panic!("{:?}", lowered.diagnostics);
    };
    assert_eq!(d.severity, Severity::Warning);
    assert!(d.message.contains("not drawn"), "{d:?}");
}

/// The names that stay members and links are not decorations.
#[test]
fn elements_named_like_decorations_are_not_read() {
    let lowered = diagram::lower("@startuml\nclass Header\nHeader --> Footer\n@enduml\n");
    assert!(matches!(lowered.diagram, Diagram::Graph(_)), "{lowered:?}");
}

/// Every kind that reads the shared statements takes them: the drawing is the diagram's own,
/// wrapped.
#[test]
fn every_kind_takes_decorations() {
    let kinds = [
        ("decor_sequence", "Sequence"),
        ("decor_class", "Graph"),
        ("decor_activity", "Graph"),
        ("decor_state", "Graph"),
        ("decor_component", "Graph"),
        ("decor_mindmap", "Mindmap"),
        ("decor_wbs", "Graph"),
        ("decor_gantt", "Gantt"),
        ("decor_chen", "Graph"),
        ("decor_nwdiag", "Graph"),
        ("decor_files", "TreeView"),
        ("decor_chronology", "Timeline"),
    ];
    for (name, inner) in kinds {
        let src = fixture("valid", name);
        let lowered = diagram::lower(&src);
        let Diagram::Decorated(d) = &lowered.diagram else {
            panic!("{name}: {:?}", lowered.diagram);
        };
        let shown = format!("{:?}", d.diagram);
        assert!(shown.starts_with(inner), "{name}: {inner}");
        assert!(d.decor.header.is_some(), "{name}");
        assert!(d.decor.footer.is_some(), "{name}");
        assert!(d.decor.caption.is_some(), "{name}");
        assert!(d.decor.legend.is_some(), "{name}");
        assert!(
            lowered.diagnostics.is_empty(),
            "{name}: {:?}",
            lowered.diagnostics
        );
        // The scene is the diagram's own with the decorations around it, in the same theme.
        let whole = lowered.diagram.scene(&Approx, &Theme::LIGHT).unwrap();
        let own = d.diagram.scene(&Approx, &Theme::LIGHT).unwrap();
        assert!(
            whole.height > own.height + 60.0,
            "{name}: {} {}",
            whole.height,
            own.height
        );
        assert!(whole.width >= own.width, "{name}");
        assert!(whole.elements.len() > own.elements.len(), "{name}");
        let rows = lowered.diagram.ascii(&Options::default()).unwrap();
        let rows_own = d.diagram.ascii(&Options::default()).unwrap();
        assert!(
            rows.lines().count() > rows_own.lines().count() + 6,
            "{name}:\n{rows}"
        );
        assert!(
            rows.contains('\u{250c}'),
            "{name}: the legend is boxed\n{rows}"
        );
    }
}

mod drawing {
    use super::*;

    const LAYOUT: &str = "@startuml\nheader HEAD\nfooter FOOT\ncaption CAP\nlegend\nLEG\nend legend\nA -> B : msg\n@enduml\n";

    #[test]
    fn the_scene_is_header_diagram_legend_caption_footer_from_the_top() {
        let s = scene(LAYOUT);
        let (_, head) = at(&s, "HEAD");
        let (_, leg) = at(&s, "LEG");
        let (_, cap) = at(&s, "CAP");
        let (_, foot) = at(&s, "FOOT");
        let (_, msg) = at(&s, "msg");
        assert!(
            head < msg && msg < leg && leg < cap && cap < foot,
            "{head} {msg} {leg} {cap} {foot}"
        );
        assert!(foot < s.height && head > 0.0);
    }

    #[test]
    fn a_top_legend_is_above_the_diagram() {
        let s = scene(&LAYOUT.replace("legend\nLEG", "legend top\nLEG"));
        let (_, head) = at(&s, "HEAD");
        let (_, leg) = at(&s, "LEG");
        let (_, msg) = at(&s, "msg");
        assert!(head < leg && leg < msg, "{head} {leg} {msg}");
    }

    #[test]
    fn decorations_grow_the_scene_and_leave_the_diagram_whole() {
        let bare = scene("@startuml\nA -> B : msg\n@enduml\n");
        let s = scene(LAYOUT);
        assert!(
            s.height > bare.height + 4.0 * 20.0,
            "{} {}",
            s.height,
            bare.height
        );
        assert!(s.width >= bare.width);
        // The diagram's own text is where it was across, and lower by what is above it.
        let (bx, by) = at(&bare, "msg");
        let (x, y) = at(&s, "msg");
        assert!(
            (x - bx - (s.width - bare.width) / 2.0).abs() < 0.01,
            "{x} {bx}"
        );
        assert!(y > by);
        for e in &s.elements {
            if let Primitive::Text { center, .. } = &e.primitive {
                assert!(center.x > 0.0 && center.x < s.width, "{center:?}");
                assert!(center.y > 0.0 && center.y < s.height, "{center:?}");
            }
        }
    }

    #[test]
    fn positions_set_the_side() {
        let src =
            "@startuml\nleft header L\nright header R\ncenter footer C\nA -> B : msg\n@enduml\n";
        // The last header is the one drawn.
        let s = scene(src);
        let (hx, _) = at(&s, "R");
        let (fx, _) = at(&s, "C");
        assert!(hx > s.width / 2.0, "{hx}");
        assert!((fx - s.width / 2.0).abs() < 0.01, "{fx}");
        let s = scene("@startuml\nleft header L\nA -> B : msg\n@enduml\n");
        let (lx, _) = at(&s, "L");
        assert!(lx < s.width / 2.0, "{lx}");
    }

    #[test]
    fn a_legend_is_a_bordered_box_at_its_corner() {
        let legend = |open: &str| {
            let s = scene(&format!(
                "@startuml\n{open}\nLEGEND TEXT\nend legend\nA -> B : msg\n@enduml\n"
            ));
            let (x, y) = at(&s, "LEGEND TEXT");
            (s, x, y)
        };
        let (s, lx, ly) = legend("legend top left");
        let (_, rx, ry) = legend("legend top right");
        let (_, _, by) = legend("legend bottom right");
        assert!(lx < rx, "{lx} {rx}");
        assert_eq!(ly, ry);
        assert!(by > ly);
        // A filled, stroked box around the text.
        let boxed = s.elements.iter().any(|e| match &e.primitive {
            Primitive::Path { fill: Some(_), stroke: Some(_), segments, .. } => {
                segments.len() == 5
                    && matches!(segments[0], sirene_core::scene::Segment::Move(p) if p.x < lx && p.y < ly)
            }
            _ => false,
        });
        assert!(boxed);
    }

    #[test]
    fn creole_sets_style_color_and_lists() {
        let s = scene(
            "@startuml\nlegend\n**b** //i// <color:red>r</color> plain\n* one\n** two\n# n\nend legend\nA -> B : msg\n@enduml\n",
        );
        let style = |text: &str| {
            s.elements
                .iter()
                .find_map(|e| match &e.primitive {
                    Primitive::Text {
                        text: t,
                        style,
                        color,
                        ..
                    } if t == text => Some((*style, *color)),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{text:?} in {:?}", texts(&s)))
        };
        assert!(style("b").0.bold && !style("b").0.italic);
        assert!(style("i").0.italic && !style("i").0.bold);
        assert_ne!(style("r").1, Theme::LIGHT.text);
        assert_eq!(style("plain").1, Theme::LIGHT.text);
        let shown: Vec<&str> = texts(&s).into_iter().map(|t| t.0).collect();
        assert!(shown.contains(&"\u{2022} one"), "{shown:?}");
        assert!(shown.contains(&"\u{2022} two"), "{shown:?}");
        assert!(shown.contains(&"1. n"), "{shown:?}");
    }

    #[test]
    fn text_has_the_rows_in_place() {
        let out = ascii(LAYOUT, Charset::Unicode);
        let rows: Vec<&str> = out.lines().collect();
        let find = |t: &str| {
            rows.iter()
                .position(|r| r.contains(t))
                .unwrap_or_else(|| panic!("{t} in\n{out}"))
        };
        let (head, msg, leg, cap, foot) = (
            find("HEAD"),
            find("msg"),
            find("LEG"),
            find("CAP"),
            find("FOOT"),
        );
        assert!(
            head < msg && msg < leg && leg < cap && cap < foot,
            "\n{out}"
        );
        // The header is at the right, the footer and caption centered, the legend boxed.
        let width = rows.iter().map(|r| r.chars().count()).max().unwrap();
        assert!(rows[head].trim_end().chars().count() == width, "\n{out}");
        let mid = |r: &str| {
            let lead = r.chars().take_while(|c| *c == ' ').count();
            lead * 2 + r.trim().chars().count()
        };
        assert!(mid(rows[foot]).abs_diff(width) <= 1, "\n{out}");
        assert!(
            rows[leg - 1].contains('\u{250c}') && rows[leg + 1].contains('\u{2514}'),
            "\n{out}"
        );
        let plain = ascii(LAYOUT, Charset::Ascii);
        assert!(plain.contains("+---") && plain.is_ascii(), "\n{plain}");
    }

    #[test]
    fn a_legend_at_the_top_left_of_text_is_the_first_block() {
        let out = ascii(
            &LAYOUT.replace("legend\nLEG", "legend top left\nLEG"),
            Charset::Unicode,
        );
        let rows: Vec<&str> = out.lines().collect();
        let find = |t: &str| rows.iter().position(|r| r.contains(t)).unwrap();
        assert!(
            find("HEAD") < find("LEG") && find("LEG") < find("msg"),
            "\n{out}"
        );
        let leg = rows[find("LEG")];
        assert!(leg.starts_with('\u{2502}'), "\n{out}");
    }

    #[test]
    fn lists_in_text_use_the_charset() {
        let src = "@startuml\nlegend\n* one\n# two\nend legend\nA -> B\n@enduml\n";
        let u = ascii(src, Charset::Unicode);
        assert!(u.contains("\u{2022} one") && u.contains("1. two"), "\n{u}");
        let a = ascii(src, Charset::Ascii);
        assert!(a.contains("* one") && a.contains("1. two"), "\n{a}");
    }

    /// The scene of each fixture, as the elements in order.
    #[test]
    fn scene_snapshots() {
        let mut names: Vec<String> = fs::read_dir(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plantuml/valid"),
        )
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("decor_") && n.ends_with(".puml"))
        .map(|n| n.trim_end_matches(".puml").to_owned())
        .collect();
        names.sort();
        assert!(names.len() == 12, "{names:?}");
        for name in names {
            let src = fixture("valid", &name);
            let s = scene(&src);
            insta::assert_snapshot!(format!("scene__{name}"), dump(&s), &src);
        }
    }

    /// A scene as lines: its size, then each text with its place and each path with its box.
    fn dump(s: &Scene) -> String {
        let mut out = format!("{:.1} x {:.1}\n", s.width, s.height);
        for e in &s.elements {
            match &e.primitive {
                Primitive::Text {
                    text,
                    center,
                    style,
                    color,
                    ..
                } => {
                    let _ = writeln!(
                        out,
                        "text {text:?} ({:.1}, {:.1}){}{} #{:02x}{:02x}{:02x}",
                        center.x,
                        center.y,
                        if style.bold { " bold" } else { "" },
                        if style.italic { " italic" } else { "" },
                        color.r,
                        color.g,
                        color.b
                    );
                }
                Primitive::Path {
                    segments,
                    fill,
                    stroke,
                    ..
                } => {
                    let mut pts = Vec::new();
                    for seg in segments {
                        match seg {
                            sirene_core::scene::Segment::Move(p)
                            | sirene_core::scene::Segment::Line(p) => pts.push(*p),
                            sirene_core::scene::Segment::Arc { to, .. }
                            | sirene_core::scene::Segment::Cubic { to, .. } => pts.push(*to),
                            sirene_core::scene::Segment::Close => {}
                        }
                    }
                    let (x0, x1) = pts
                        .iter()
                        .fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.x), a.1.max(p.x)));
                    let (y0, y1) = pts
                        .iter()
                        .fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.y), a.1.max(p.y)));
                    let _ = writeln!(
                        out,
                        "path {} points ({x0:.1}, {y0:.1})-({x1:.1}, {y1:.1}){}{}",
                        pts.len(),
                        if fill.is_some() { " fill" } else { "" },
                        if stroke.is_some() { " stroke" } else { "" }
                    );
                }
            }
        }
        out
    }
}

#[cfg(feature = "math")]
mod math {
    use super::*;

    const VALID: [&str; 2] = ["latex_inline_decor_seq", "latex_inline_decor_class"];

    /// How many filled paths of the scene are in the text color: a formula's glyphs and rules.
    fn formula_paths(scene: &Scene) -> usize {
        scene
            .elements
            .iter()
            .filter(|e| {
                matches!(
                    &e.primitive,
                    Primitive::Path { fill: Some(f), stroke: None, .. } if *f == Theme::LIGHT.text
                )
            })
            .count()
    }

    #[test]
    fn well_formed_fixtures_have_no_diagnostics() {
        for name in VALID {
            let d = diagram::lower(&fixture("valid", name)).diagnostics;
            assert!(d.is_empty(), "{name}: {d:?}");
        }
    }

    #[test]
    fn formulas_are_drawn_in_both_renderers_without_markup() {
        for name in VALID {
            let src = fixture("valid", name);
            let s = scene(&src);
            assert!(formula_paths(&s) >= 4, "{name}: {}", formula_paths(&s));
            assert!(
                texts(&s)
                    .iter()
                    .all(|t| !t.0.contains("latex>") && !t.0.contains("math>")),
                "{name}: {:?}",
                texts(&s)
            );
            for charset in [Charset::Unicode, Charset::Ascii] {
                let rows = ascii(&src, charset);
                assert!(
                    !rows.contains("latex>") && !rows.contains("math>"),
                    "{name}:\n{rows}"
                );
            }
        }
    }

    /// A formula in a line sits on the line's baseline with the text beside it, and a tall one
    /// makes its line, and so the scene, taller.
    #[test]
    fn a_tall_formula_makes_its_line_taller() {
        let plain = scene("@startuml\ncaption a b\nA -> B\n@enduml\n");
        let tall = scene("@startuml\ncaption a <latex>\\frac{x}{y}</latex> b\nA -> B\n@enduml\n");
        assert!(tall.height > plain.height);
        assert!(texts(&tall).iter().any(|t| t.0 == "a"));
        assert!(formula_paths(&tall) > 0);
        let rows = ascii(
            "@startuml\ncaption a <latex>\\frac{x}{y}</latex> b\nA -> B\n@enduml\n",
            Charset::Unicode,
        );
        let plain = ascii(
            "@startuml\ncaption a b\nA -> B\n@enduml\n",
            Charset::Unicode,
        );
        assert!(rows.lines().count() > plain.lines().count(), "\n{rows}");
    }

    #[test]
    fn several_lines_and_backslash_n_each_hold_formulas() {
        let src = fixture("valid", "latex_inline_decor_seq");
        let s = scene(&src);
        let shown: Vec<&str> = texts(&s).into_iter().map(|t| t.0).collect();
        // The caption is two lines: `\n` outside a formula breaks it.
        assert!(shown.contains(&"over two"), "{shown:?}");
        assert!(shown.contains(&"lines of"), "{shown:?}");
    }

    /// A failing formula is drawn as its source and has one error on its body.
    #[test]
    fn each_failing_formula_has_one_error_on_its_body() {
        let src = fixture("errors", "latex_inline_fail_decor");
        let lowered = diagram::lower(&src);
        let d = &lowered.diagnostics;
        assert_eq!(d.len(), 4, "{d:?}");
        assert!(d.iter().all(|d| d.severity == Severity::Error), "{d:?}");
        for (d, want) in d.iter().zip([r"\foo", r"\bar", r"\baz", r"\qux"]) {
            let at = &src[usize::from(d.range.start())..usize::from(d.range.end())];
            // An error at the end of the body is an empty range there.
            assert!(
                want.contains(at) && (at == want || at.is_empty()),
                "{want}: {at:?}"
            );
        }
        let lines: Vec<usize> = d.iter().map(|d| d.line_col(&src).0).collect();
        assert_eq!(lines, [2, 3, 4, 6]);
        let s = lowered.diagram.scene(&Approx, &Theme::LIGHT).unwrap();
        for source in [
            r"<latex>\foo</latex>",
            r"<latex>\baz{x}</latex>",
            r"<latex>\qux</latex>",
        ] {
            assert!(
                texts(&s).iter().any(|t| t.0.contains(source)),
                "{source}: {:?}",
                texts(&s)
            );
        }
        assert!(formula_paths(&s) > 0, "the good formula is set");
    }

    #[test]
    fn a_replaced_decoration_is_not_checked() {
        let src = "@startuml\nheader <latex>\\foo</latex>\nheader fine\nA -> B\n@enduml\n";
        let lowered = diagram::lower(src);
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    }

    #[test]
    fn a_formula_equal_to_one_in_a_label_is_diagnosed_on_its_own_line() {
        let src = "@startuml\nclass A\nnote right of A : <latex>\\foo</latex>\nheader <latex>\\foo</latex>\n@enduml\n";
        let d = diagram::lower(src).diagnostics;
        assert_eq!(d.len(), 2, "{d:?}");
        let mut lines: Vec<usize> = d.iter().map(|d| d.line_col(src).0).collect();
        lines.sort_unstable();
        assert_eq!(lines, [3, 4], "{d:?}");
    }
}

#[cfg(not(feature = "math"))]
mod no_math {
    use super::*;

    /// One warning per formula, and the tags stay text.
    #[test]
    fn tags_stay_literal_with_one_warning_per_formula() {
        for (dir, name) in [
            ("valid", "latex_inline_decor_seq"),
            ("valid", "latex_inline_decor_class"),
            ("errors", "latex_inline_fail_decor"),
        ] {
            let src = fixture(dir, name);
            let formulas = src.matches("<latex>").count() + src.matches("<math>").count();
            let lowered = diagram::lower(&src);
            assert_eq!(lowered.diagnostics.len(), formulas, "{name}");
            assert!(
                lowered
                    .diagnostics
                    .iter()
                    .all(|d| d.severity == Severity::Warning),
                "{name}"
            );
            let s = lowered.diagram.scene(&Approx, &Theme::LIGHT).unwrap();
            assert!(
                texts(&s)
                    .iter()
                    .any(|t| t.0.contains("latex>") || t.0.contains("math>")),
                "{name}: {:?}",
                texts(&s)
            );
        }
    }
}
