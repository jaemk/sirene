# PlantUML Mindmaps and WBS

PlantUML mindmaps (`@startmindmap`) and work breakdown structures (`@startwbs`) (PUML-2): a
mindmap is lowered to the mindmap model (MMIND-2) and a WBS to the graph model, each drawn by
its layout, scene builder, and ASCII renderer (`sirene_core::plantuml`). References:
<https://plantuml.com/mindmap-diagram> and <https://plantuml.com/wbs-diagram>; the fixtures are
written for sirene. Tests: unit tests in `crates/sirene-core/src/plantuml/mindmap.rs`, the
fixtures `mindmap.puml`, `wbs.puml`, and `errors/mindmap.puml` with
`crates/sirene-core/tests/plantuml.rs`, and property tests in
`crates/sirene-core/tests/plantuml_props.rs`.

## PMIND-1 Node lines

Each node is a line of `*`, `+`, or `-` marks, as many as its depth (`* root`, `** child`);
`+` and `-` (PlantUML's right and left sides) nest alike, and `left side` and `right side`
lines are accepted, since the layout picks the sides. A single `*` on indented lines nests by
indentation (`* root`, then `  * child`). After the marks, `_` draws the node without a box,
`[#color]` colors it, and `:text` starts text that runs to a line ending in `;`. A trailing
`<<style>>` is ignored. A node needs a parent: a line with no more marks than the root is an
error and is left out with the lines under it. `title` applies; other settings are warnings,
as in PACT-9.

## PMIND-2 Mindmaps

A `@startmindmap` document is a mindmap: boxed nodes are rounded, and boxless ones are drawn
as the default shape (text over a line). A color is a warning ("node colors are not drawn in
mindmaps"), and a direction (`top to bottom direction`) is a warning.

## PMIND-3 WBS

A `@startwbs` document is a top-down tree of boxes joined by lines with no heads; boxless nodes
are text, and a color fills the box. `<` and `>` after the marks (which side a node's children
go to) are accepted and left to the layout. `***(A) text` names a node `A`, and an
`(A) -> (B)` line draws an arrow between named nodes that does not change the ranks; an alias
no node has is an error. `left to right direction` turns the tree.

## PMIND-4 Diagnostics

A line that is not a node line, a node with no parent, and an unclosed `:` text are errors;
an unknown color is a warning.
