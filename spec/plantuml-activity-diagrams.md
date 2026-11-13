# PlantUML Activity Diagrams

PlantUML activity diagrams in the newer syntax (PUML-2), lowered to the graph model and drawn by
the layered layout, scene builder, and ASCII renderer (`sirene_core::plantuml`). Reference:
<https://plantuml.com/activity-diagram-beta>; the fixtures are written for sirene. Tests: unit
tests in `crates/sirene-core/src/plantuml/activity.rs`, the fixtures `activity.puml` and
`swimlanes.puml` with `crates/sirene-core/tests/plantuml.rs`, and property tests in
`crates/sirene-core/tests/plantuml_props.rs`.

## PACT-1 Documents and detection

A document (PSEQ-1) is an activity diagram when its first deciding line is `start`, `stop`, an
`:action;`, `if (...)`, `while (...)`, `repeat`, `fork`, `split`, `switch (...)`, a `|lane|`, or
the like (PUML-1). The flow runs top to bottom; `left to right direction` turns it. `title`
(inline or a block to `end title`) is drawn above the diagram.

## PACT-2 Actions and terminals

- `:text;` is a rounded action; the text may span lines up to the `;`, and `\n` breaks a line.
  The other ends set the shape: `|` a framed box, `<` and `>` leaning boxes, `/` a lean, `]`
  and `}` a rectangle. An action with no closing end is an error.
- `#color:text;` fills the action with a named or hex color.
- `start` is a small filled circle; `stop` and `end` a ringed circle. `stop`, `end`, `kill`,
  and `detach` close the flow, so the next node is not joined to it.
- `(A)` is a connector circle.

## PACT-3 Decisions

- `if (c) then (yes)` ... `elseif (d) then (yes)` ... `else (no)` ... `endif` (or `end if`)
  draws a diamond per test with the branch labels on its edges; every open branch end joins the
  next node. Without `else`, the test itself is an open end.
- `switch (s)` ... `case (x)` ... `endswitch` draws one diamond with an edge per case.

## PACT-4 Loops

- `while (c) is (y)` ... `endwhile (n)` draws the test as a diamond, a back edge from the body
  to it, and the exit labeled `n`.
- `repeat [:action;]` ... `[backward :action;]` ... `repeat while (c) is (y) not (n)` draws the
  test after the body with a back edge (through the backward action) to the loop's start.
- `break` ends the body; its end leaves the loop.

## PACT-5 Concurrency

`fork` ... `fork again` ... `end fork` draws a bar before and after the branches; `end merge`
joins them in a diamond instead. `split` ... `split again` ... `end split` branches without the
first bar.

## PACT-6 Arrows

`-> text;` labels the edges into the next node, over a branch label. `-[#color,dashed]->` sets
their color and stroke (`dashed`, `dotted`, `bold`, `hidden`); an unknown color or style is a
warning.

## PACT-7 Grouping and lanes

- `partition`, `package`, `rectangle`, `card`, `folder`, and `frame` name `{` ... `}` and
  `group name` ... `end group` draw the nodes inside them in a titled cluster.
- `|Lane|` (or `|#color|Lane|`) puts the following nodes in a swimlane; lanes are columns in
  first-use order. Nodes and groups before the first `|Lane|` go in that lane.
- `if (c) is (red) then`: the `is` label names the yes branch when `then` has no label; the
  same for `elseif`.

## PACT-8 Notes

`note left` or `note right` attaches a note to the last node, either inline after `:` or as a
block to `end note`; `floating note` does the same.

## PACT-9 Diagnostics

`else`, `endif`, `endwhile`, `break`, and the other closers outside their block are errors, as is
a block left open at the end ("unclosed `while`: expected `endwhile`") and any line that is not
an activity statement. The legacy syntax (`(*)`, `-->` between quoted labels) is an error.
Settings and decoration that are not drawn (`skinparam`, `header`, `footer`, `legend`,
`caption`, `scale`, `hide`, and the like) are warnings; `!include` and the other file-reading
directives are errors (PUML-4).
