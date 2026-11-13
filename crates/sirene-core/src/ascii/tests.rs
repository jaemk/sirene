use super::*;
use crate::flowchart;

fn draw(src: &str) -> String {
    render(
        &flowchart::parse(src).lower().flowchart,
        &Options::default(),
    )
}

fn draw_ascii(src: &str) -> String {
    render(
        &flowchart::parse(src).lower().flowchart,
        &Options {
            charset: Charset::Ascii,
        },
    )
}

/// Expected output from its rows.
fn rows(rows: &[&str]) -> String {
    let mut s = rows.join("\n");
    s.push('\n');
    s
}

#[test]
fn single_edge() {
    assert_eq!(
        draw("flowchart TD\nA --> B\n"),
        rows(&[
            "┌───┐", //
            "│ A │",
            "└─┬─┘",
            "  │",
            "  ▼",
            "┌───┐",
            "│ B │",
            "└───┘",
        ])
    );
}

#[test]
fn empty_diagram() {
    assert_eq!(draw("flowchart\n"), "");
    assert_eq!(draw("---\ntitle: Only\n---\nflowchart\n"), "Only\n\n");
}

/// A blank title prints no line, and a title's own padding is not printed (ASCIIR-1).
#[test]
fn blank_titles() {
    let plain = draw("flowchart LR\nA --> B\n");
    let mut c = flowchart::parse("flowchart LR\nA --> B\n")
        .lower()
        .flowchart;
    for blank in ["", " ", "\t \n"] {
        c.title = Some(blank.into());
        assert_eq!(render(&c, &Options::default()), plain, "{blank:?}");
    }
    c.title = Some("  T  ".into());
    assert_eq!(
        render(&c, &Options::default()),
        format!("      T\n\n{plain}")
    );
}

#[test]
fn title_is_centered() {
    assert_eq!(
        draw("---\ntitle: T\n---\nflowchart LR\nA --> B\n"),
        rows(&[
            "      T",
            "",
            "┌───┐    ┌───┐",
            "│ A ├───►│ B │",
            "└───┘    └───┘",
        ])
    );
}

#[test]
fn arrowheads_and_strokes() {
    let src = "flowchart TD\nA <--> B\nC x==x D\nE o-.-o F\nG --- H\n";
    assert_eq!(
        draw(src),
        rows(&[
            "┌───┐   ┌───┐   ┌───┐   ┌───┐",
            "│ A │   │ C │   │ E │   │ G │",
            "└───┘   └───┘   └───┘   └─┬─┘",
            "  ▲       x       o       │",
            "  ▼       x       o       │",
            "┌───┐   ┌───┐   ┌───┐   ┌─┴─┐",
            "│ B │   │ D │   │ F │   │ H │",
            "└───┘   └───┘   └───┘   └───┘",
        ])
    );
    assert_eq!(
        draw_ascii(src),
        rows(&[
            "+---+   +---+   +---+   +---+",
            "| A |   | C |   | E |   | G |",
            "+---+   +---+   +---+   +-+-+",
            "  ^       x       o       |",
            "  v       x       o       |",
            "+---+   +---+   +---+   +-+-+",
            "| B |   | D |   | F |   | H |",
            "+---+   +---+   +---+   +---+",
        ])
    );
}

#[test]
fn directions() {
    assert_eq!(
        draw("flowchart BT\nA --> B\n"),
        rows(&[
            "┌───┐", //
            "│ B │",
            "└───┘",
            "  ▲",
            "  │",
            "┌─┴─┐",
            "│ A │",
            "└───┘",
        ])
    );
    assert_eq!(
        draw("flowchart RL\nA --> B\n"),
        rows(&[
            "┌───┐    ┌───┐", //
            "│ B │◄───┤ A │",
            "└───┘    └───┘",
        ])
    );
}

#[test]
fn invisible_edges_are_not_drawn() {
    assert_eq!(
        draw("flowchart LR\nA ~~~|hidden| B\n"),
        rows(&[
            "┌───┐          ┌───┐", //
            "│ A │          │ B │",
            "└───┘          └───┘",
        ])
    );
}

#[test]
fn edge_labels() {
    assert_eq!(
        draw("flowchart TD\nA -->|two<br>lines| B\n"),
        rows(&[
            "┌───┐", //
            "│ A │",
            "└─┬─┘",
            "  │",
            " two",
            "lines",
            "  ▼",
            "┌───┐",
            "│ B │",
            "└───┘",
        ])
    );
    assert_eq!(
        draw("flowchart LR\nA -- yes --> B\n"),
        rows(&[
            "┌───┐       ┌───┐", //
            "│ A ├──yes─►│ B │",
            "└───┘       └───┘",
        ])
    );
}

#[test]
fn self_loops() {
    assert_eq!(
        draw("flowchart TD\nA --> A\n"),
        rows(&[
            "┌───┬─┐", //
            "│ A │ │",
            "└───┘◄┘",
        ])
    );
    assert_eq!(
        draw("flowchart LR\nA -->|again| A\n"),
        rows(&[
            "┌───┐", //
            "│ A │",
            "├───┘",
            "│   ▲",
            "└───┘",
            "",
            "again",
        ])
    );
}

#[test]
fn shapes_top_down() {
    assert_eq!(
        draw("flowchart TD\nA[[sub]] --> B[(db)] --> C@{ shape: text, label: plain }\n"),
        rows(&[
            "┌┬─────┬┐",
            "││ sub ││",
            "└┴──┬──┴┘",
            "    │",
            "    ▼",
            " ╭────╮",
            " ├────┤",
            " │ db │",
            " ╰──┬─╯",
            "    │",
            "    ▼",
            "  plain",
        ])
    );
    assert_eq!(
        draw_ascii("flowchart TD\nA(((dbl))) --> B([pill]) --> C{{hex}}\n"),
        rows(&[
            " +=====+",
            " | dbl |",
            " +==+==+",
            "    |",
            "    v",
            ".------.",
            "( pill )",
            "'---+--'",
            "    |",
            "    v",
            " /-----\\",
            " < hex >",
            " \\-----/",
        ])
    );
}

#[test]
fn markdown_and_wide_labels() {
    assert_eq!(
        draw("flowchart TD\nA[\"`**bold** *it*`\"] --> B[日本]\n"),
        rows(&[
            "┌─────────┐",
            "│ bold it │",
            "└────┬────┘",
            "     │",
            "     ▼",
            " ┌──────┐",
            " │ 日本 │",
            " └──────┘",
        ])
    );
}

/// A chain through titled subgraphs runs straight down: the titles give way to the border
/// ports, so the subgraphs do not drift sideways one after another.
#[test]
fn subgraph_chain_stays_straight() {
    let mut src = String::from("flowchart TD\n");
    for i in 0..20 {
        src.push_str(&format!(
            "subgraph s{i}\na{i} --> b{i}\nend\nb{i} --> a{}\n",
            i + 1
        ));
    }
    let out = draw(&src);
    let widest = out.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    assert!(widest <= 16, "{out}");
    // No jogs: every edge is a straight run down.
    assert!(!out.contains("└┐") && !out.contains("┌┘"), "{out}");
}

#[test]
fn clusters_and_crossing_edges() {
    // The title keeps clear of the edge crossing the top border.
    assert_eq!(
        draw("flowchart TD\nA --> B\nsubgraph S [Group]\nB --> C\nend\nC --> D\nA --> S\n"),
        rows(&[
            "  ┌───┐",
            "  │ A │",
            "  └┬─┬┘",
            "   │ │",
            "   │ │",
            "   └┐│",
            "    │▼",
            "┌───┼──── Group ─┐",
            "│   ▼            │",
            "│ ┌───┐          │",
            "│ │ B │          │",
            "│ └─┬─┘          │",
            "│   │            │",
            "│   ▼            │",
            "│ ┌───┐          │",
            "│ │ C │          │",
            "│ └─┬─┘          │",
            "│   │            │",
            "└───┼────────────┘",
            "    │",
            "    ▼",
            "  ┌───┐",
            "  │ D │",
            "  └───┘",
        ])
    );
}

#[test]
fn clusters_with_edges_inherit_direction() {
    // Edges to the subgraph itself keep its `direction LR` (LAYOUT-3).
    assert_eq!(
        draw("flowchart TD\nsubgraph S\ndirection LR\nA --> B\nend\nX --> S\nS --> Y\n"),
        rows(&[
            "        ┌───┐",
            "        │ X │",
            "        └─┬─┘",
            "          │",
            "          ▼",
            "┌─ S ──────────────┐",
            "│  ┌───┐    ┌───┐  │",
            "│  │ A ├───►│ B │  │",
            "│  └───┘    └───┘  │",
            "└─────────┬────────┘",
            "          │",
            "          ▼",
            "        ┌───┐",
            "        │ Y │",
            "        └───┘",
        ])
    );
    // `direction LR` is ignored because an edge reaches a node inside the subgraph from outside.
    let out = draw("flowchart TD\nsubgraph S\ndirection LR\nA --> B\nend\nX --> A\nB --> Y\n");
    assert!(!out.contains("│ A ├"), "{out}");
}

#[test]
fn isolated_clusters_keep_their_direction() {
    assert_eq!(
        draw("flowchart TD\nsubgraph S\ndirection LR\nA --> B\nend\n"),
        rows(&[
            "┌─ S ──────────────┐",
            "│  ┌───┐    ┌───┐  │",
            "│  │ A ├───►│ B │  │",
            "│  └───┘    └───┘  │",
            "└──────────────────┘",
        ])
    );
}

#[test]
fn edge_to_own_cluster_is_not_drawn() {
    assert_eq!(
        draw("flowchart TD\nsubgraph S\nA\nend\nA --> S\n"),
        rows(&[
            "┌── S ──┐",
            "│       │",
            "│ ┌───┐ │",
            "│ │ A │ │",
            "│ └───┘ │",
            "│       │",
            "└───────┘",
        ])
    );
}

#[test]
fn edge_paths_outside_the_layout_contract() {
    let chart = flowchart::parse("flowchart\nA --> B\n").lower().flowchart;
    let e = &chart.edges[0];
    let far = Rect::new(9, 9, 1, 1);
    let line = [Point::new(0, 0), Point::new(5, 0)];
    let mut canvas = Canvas::new(6, 1);
    // Entirely inside the source, entirely inside the target, or source and target overlapping.
    let (box_, far) = (|r| (r, false), (far, false));
    draw_edge(
        &mut canvas,
        e,
        &line,
        box_(Rect::new(0, 0, 6, 1)),
        far,
        None,
    );
    draw_edge(
        &mut canvas,
        e,
        &line,
        far,
        box_(Rect::new(0, 0, 6, 1)),
        None,
    );
    draw_edge(
        &mut canvas,
        e,
        &line,
        box_(Rect::new(0, 0, 4, 1)),
        box_(Rect::new(2, 0, 4, 1)),
        None,
    );
    assert_eq!(canvas.render(Charset::Unicode), "\n");
    // Ends outside both endpoints: drawn, without an arrowhead.
    draw_edge(&mut canvas, e, &line, far, far, None);
    assert_eq!(canvas.render(Charset::Unicode), "──────\n");
}

#[test]
fn control_characters_become_spaces() {
    let mut chart = flowchart::parse("flowchart\nA\n").lower().flowchart;
    chart.nodes[0].label = Label::plain("a\tb");
    assert_eq!(
        render(&chart, &Options::default()),
        "┌─────┐\n│ a b │\n└─────┘\n"
    );
}
