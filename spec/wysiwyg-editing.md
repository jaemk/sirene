# WYSIWYG Editing

Editing a diagram in its window: the source text in a pane beside the drawing, and edits made on
the drawing itself, written back to the source. Tests: `crates/sirene-gui/tests/editor.rs`,
`crates/sirene-gui/tests/drawing.rs`, and the unit tests in `crates/sirene-gui/src/document.rs`.

## WYSIWY-1 Source pane

Each window has a source pane to the left of the drawing: a code editor (gpui-component's
`Editor`) with line numbers, undo/redo, and search over the document's text. ToggleSource
(secondary-e) shows and hides it; it is shown by default, and new windows show or hide it as it
last was (GUISHE-11). Hiding it keeps its text, undo history,
and cursor.

## WYSIWY-2 Live drawing

Every change to the source text re-parses the document and redraws the diagram. Diagnostics are
shown in the editor at their source ranges, with their severity, and in the status bar as in
GUISHE-7.

## WYSIWY-3 Unsaved edits

A document is edited when its text differs from the text last read from or written to its file,
or for Untitled, from the text it opened with. An edited document shows `NAME (edited)` as its window title and in
the status bar.

## WYSIWY-4 Saving

Save (secondary-s) writes the text to the document's file, replacing it through a temporary file
in the same directory, and the document is no longer edited. The text is synced to disk before
the rename, the file keeps its permissions, owner, and group, a symlinked file stays a symlink
(its target is written), and anything already at the temporary name is replaced, not written
through. When the owner cannot be kept (another user's file), the file is overwritten in place
instead. A read-only file is not saved: `cannot write PATH: the file is read-only`. Save As (secondary-shift-s), and
Save on an Untitled document, ask for a path first (starting in the directory an export would,
GUISHE-9, with `diagram.mmd` for Untitled); the window then shows that file. A failed
write leaves the document edited and shows the error in the status bar.

## WYSIWY-5 Changes on disk

The file poll of GUISHE-3 keeps running. When the file changes and the document is not edited, the
new text replaces the editor's text, keeping the cursor line where it still exists. When the
document is edited, the editor keeps its text and the status bar says the file changed on disk;
Reload replaces the edits with the file's text. Reload on an edited document first asks
("Discard your changes to NAME?", Reload or Cancel), and Save after the file changed on disk
first asks ("NAME changed on disk. Overwrite it?", Overwrite or Cancel); Cancel changes nothing.

## WYSIWY-6 Closing with unsaved edits

CloseWindow, and Quit for each window, ask whether to save an edited document: Save writes it (as
in WYSIWY-4) and closes, Don't Save closes, and Cancel keeps the window open. Quit stops at the
first Cancel.

## WYSIWY-7 New documents

New (secondary-n), and launching with no files, open a window with an Untitled document whose
text is a starter flowchart: `flowchart TD` and one `A --> B` edge. Open (GUISHE-5) shows its
first file in such a window while it is not edited.

## WYSIWY-8 Source edits

`sirene_core::flowchart::edit` turns a change to the diagram into one `TextEdit` (a byte range of
the source and its replacement), the smallest that makes the change. Text outside the statements
it touches, including comments and formatting, is kept byte for byte. Tests:
`crates/sirene-core/tests/flowchart_edit.rs`.

- `set_node_label`: replaces the label of the node's last mention with a shape, or adds `[...]`
  after its first mention. Nodes whose label comes from `@{ label: ... }` are not edited.
- `set_edge_label`: replaces a `|...|` label in place; otherwise rewrites the edge's stroke in
  canonical form (keeping its stroke kind, arrowheads, and length) with a `|...|` label, or
  without one when the label is empty. Edges written in one statement with `&` share a label.
- `add_node` and `add_edge`: append a statement at the end of the document, indented like the
  last top-level statement. New node ids are the first of `A` to `Z`, then `n1`, `n2`, ..., that
  the source does not use or name (a `class` statement can name a node not written yet).
- `delete`: removes nodes, edges, and every edge touching a removed node. A chain that loses a
  middle edge or node is split into one statement per remaining piece, on its own line. A piece
  left as a single bare node is dropped when the node was already mentioned before it in the same
  subgraph (or at the top level), so node order, subgraph membership, and what later `class` and
  `style` statements apply to stay the same. `class` lists drop
  removed ids, `style` and `click` statements for removed nodes go, `linkStyle` indexes are
  renumbered, and the `id@{ ... }`, `class`, and `style` statements of removed edges go,
  including edges removed with a node. A statement that ends up empty
  is removed with its line when it was alone on it, otherwise with the `;` between it and its
  neighbor.
- `set_node_shape`: a shape with classic delimiters (`[...]`, `(...)`, `{...}`, and the other 12)
  replaces the delimiters of the node's last mention that has them, or adds them with the label
  after its first mention, and drops `shape` from the node's `@{ ... }` (the whole `@{ ... }` when
  nothing else is in it). Any other shape sets `shape: NAME` in the `@{ ... }` of the node's last
  mention with delimiters or metadata (or its first mention), adding `@{ shape: NAME }` when there
  is none. A label written as unquoted text is rewritten as for `set_node_label`; quoted labels
  are kept.
- `set_edge_line`: rewrites the edge's stroke in canonical form with the given stroke, arrowheads,
  and length (clamped to 1..=10), keeping its label as `|...|`.
- `reverse_edge`: an edge alone in its statement swaps its two ends in place. An edge in a chain is
  deleted (as `delete` does) and appended reversed, with its stroke and label, as its own
  statement. Edges that share a stroke (`A & B --> C`), and edges with an id in a chain, are not
  reversed.
- Labels are written bare when they are one line of letters, digits, spaces, and `_.,!?'`;
  otherwise quoted, with `"`, `#`, `&`, `<`, and (outside markdown) `` ` `` as entity codes and line
  breaks as `<br>`. Markdown labels stay markdown strings. Parsing the written label gives back
  the text with control characters as spaces and each line trimmed. Setting a label to the text
  it already has makes no edit.

## WYSIWY-9 Selection

Clicking a node or an edge (or its label) on the drawing selects it; shift-click adds it to the
selection or removes it. Clicking empty space or a subgraph clears the selection, as does
Deselect (escape). Selected items are outlined in the theme's accent color. Selecting one item
selects its source in the pane (a node's first mention, an edge's stroke and label) without
taking focus. Nodes stay selected by id and edges by index while the document changes, as long
as they exist. Clicking the drawing gives it focus.

## WYSIWY-10 Label editing

Double-clicking a node or an edge, or Rename (enter) with one item selected, opens a text field
over it holding its label, with line breaks shown as `<br>`. Enter applies the label (WYSIWY-8),
turning `<br>` back into line breaks; escape or leaving the field cancels. An empty edge label
removes the label.

## WYSIWY-11 Adding and connecting

AddNode (secondary-shift-n) adds a node labelled `New node`; with one node selected, it also adds
an edge from that node to the new one. The new node is selected and its label opens for editing.
Dragging from a node and releasing on another node (or the same one) adds an edge between them;
a line follows the pointer while dragging; releasing on empty space in the drawing adds a node
(WYSIWY-15). Dragging from empty space pans.

## WYSIWY-12 Deleting

Delete (backspace or delete, with the drawing focused) removes the selected nodes and edges
(WYSIWY-8) and clears the selection.

## WYSIWY-13 Drawing edits and undo

Changes made on the drawing are applied to the source pane's text as one edit each, so they mark
the document edited and the pane's own undo reverts them. Undo (secondary-z) and Redo
(secondary-shift-z) with the drawing focused step through the drawing's changes while the text
still matches; typing in the pane clears that history.

## WYSIWY-14 Right-click menu

A right press on the drawing selects the item under it (keeping the selection when the item is
already in it; clearing it over empty space) and opens a menu for the selection. Its items act on
every selected item of their kind, as one drawing change (WYSIWY-13):

- Nothing selected: Add Node, Select All, Fit Window.
- One item: Rename.
- Nodes: Shape (the 15 classic shapes, then More with the rest; the current shape of one selected
  node is checked), Add Node After and Add Node Before (one node: a new node with an edge from
  or to it, selected with its label open), Connect To and Connect From (an edge from or to each
  other node, named by its label's first line and id).
- Edges: Line (Solid, Thick, Dotted, Invisible), Arrows (at end, at start, at both ends, none,
  circle at end, cross at end), Reverse, Longer and Shorter (one rank more or fewer, 1 to 10;
  disabled at the limit for one edge).
- Delete.

The same edits are actions (GUISHE-5). Tests: `crates/sirene-gui/tests/context_menu.rs`.

## WYSIWY-15 Shape bar and quick adds

A bar at the top left of the drawing has a button for each of rect, rounded, stadium, diamond,
hexagon, circle, cylinder, and lean right, each showing that shape as the drawing draws it, with
an "Add SHAPE" tooltip. A button runs AddShape: like AddNode (WYSIWY-11), a node labelled
`New node`, linked from the one selected node if there is one, then selected with its label
open, but in that shape (written as `set_node_shape` writes it, WYSIWY-8). Presses on the bar
do not reach the drawing.

Double-clicking empty space on the drawing (or a subgraph) adds an unlinked node the same way.
Dragging from a node and releasing on empty space inside the drawing adds a node with an edge
from the dragged node; releasing outside the drawing, or on an edge, adds nothing. Each is one
drawing change (WYSIWY-13). Tests: `crates/sirene-gui/tests/shape_bar.rs` and
`dragging_connects_and_pans` in `crates/sirene-gui/tests/drawing.rs`.

A node added by a double-click or a drag that ends inside a subgraph is added to that subgraph
(`move_to_subgraph`, WYSIWY-16), except that a drag ending on another subgraph than the dragged
node's moves the dragged node into it instead.

## WYSIWY-16 Layout hints

Edits that steer the layout without positions, as source edits (WYSIWY-8) and actions (GUISHE-5):

- `set_direction`: replaces the header's direction, or adds one after `flowchart`/`graph`; for a
  subgraph, replaces its last `direction` statement or adds one after its `subgraph` line,
  indented like its statements. Written as `TD`, `BT`, `LR`, or `RL`; no edit when it already
  reads so.
- `keep_together`: appends `A ~~~ B`, an invisible edge.
- `group`: appends `subgraph groupN [TITLE]` with a bare mention of each node and `end`, at the
  end of the subgraph the nodes share (so it nests there) or of the document. `groupN` is the
  first unused. No edit when the nodes are in different subgraphs.
- `ungroup`: removes the `subgraph` and `end` lines, takes one indent level off the lines
  between, and removes the subgraph's own `direction`, its `style` statements, and its id from
  `class` lists. Other mentions of the subgraph in node and edge statements become its members
  joined with `&`, so `s --> C` becomes `A & B --> C`, and `linkStyle` indexes of an edge that
  became several list them all. No edit when an edge at the subgraph has an id or the subgraph
  has no members, or the `subgraph` or `end` line has another statement on it.
- `move_to_subgraph`: from the top level, adds a bare mention at the end of the target; from a
  subgraph, moves the node's statements in it to the end of the target or of the document. A
  moved statement's other nodes that would no longer be in the subgraph get a bare mention at
  its end. `linkStyle` indexes follow the moved edges. No edit when the result would change
  another node's subgraph (a target before the subgraph would take them first).
- `move_node` and `move_edge`: swap the statement with the node's first mention, or the edge,
  with the statement before or after it in the same subgraph or document, when that is a node
  or edge statement or a subgraph. `linkStyle` indexes follow their edges.

On the drawing: a right press on a subgraph (not on a node or edge inside it) opens a menu with
Add Node, Direction (for the subgraph, its current one checked), Ungroup (disabled when
`ungroup` makes no edit), Select All, and Fit Window; on empty space, Direction is the
diagram's. The node menu adds Group (secondary-g), Keep Together (two nodes), Move To (Top Level
and each subgraph, the one node's place checked), and for one node Move Earlier (alt-up) and
Move Later (alt-down), disabled when they make no edit; the edge menu adds Move Earlier and Move
Later for one edge, which stays selected. Move To acts on each selected node in turn as one
change. Tests: `crates/sirene-gui/tests/layout_hints.rs`, and `directions`, `keeping_together`,
`grouping`, `ungrouping`, `moving_into_subgraphs`, and `moving_statements` in
`crates/sirene-core/tests/flowchart_edit.rs`.

## WYSIWY-17 Styles panel

Styling from the drawing writes classes, not one-off `style` statements:

- `set_class_style`: sets or removes one property of a class in its last `classDef` that names
  only that class, keeping the other declarations (and any without `:`) in order; appends
  `classDef NAME PROPERTY:VALUE` when there is none; removes a `classDef` left empty.
- `apply_class`: appends `class IDS NAME` for the nodes that do not have the class yet.
- `remove_class`: drops the nodes from `class` statements naming the class, removing a statement
  left empty, and removes their `:::NAME` shorthands.
- `fresh_class_name`: the first `styleN` not used.

ToggleStyles (View > Show or Hide Styles, secondary-i) shows a 240px panel right of the drawing.
It lists the document's classes, each with a checkbox (checked when every selected node has the
class; checking applies it to the selected nodes, unchecking removes it), a swatch of its fill
and stroke, and Edit. New Class defines `styleN` with `fill:#e8f1fb,stroke:#3b6ea5`, applies it
to the selected nodes, and edits it, as one change. The class being edited (on showing the
panel, the first class of the selected nodes) has Fill, Stroke, and Text color pickers (written
as `#rrggbb`, or `#rrggbbaa` when translucent; clearing one removes the property), Width 1 to
4 (`stroke-width:Npx`), and Line Solid or Dashed (`stroke-dasharray:5 5`). Each change is one
drawing change (WYSIWY-13). Tests: `crates/sirene-gui/tests/styles.rs`, and `class_styles` and
`applying_classes` in `crates/sirene-core/tests/flowchart_edit.rs`.

## WYSIWY-18 Edge styles

Edges are styled with `linkStyle`, which replaces an edge's earlier `linkStyle` declarations:

- `set_link_style`: sets or removes one property of edge N, starting from the declarations in
  effect for it. It rewrites the last `linkStyle` naming only N (removing one left empty, or
  only its declarations when it has `interpolate`), or appends `linkStyle N ...`. Every other
  `linkStyle` that styles N drops N from its list (a statement naming only N loses its
  declarations, keeping `interpolate`); a curve from a shared statement moves into the new one.
  `linkStyle default` is not changed.

With edges selected, the styles panel (WYSIWY-17) has an Edge section, filled from the first
selected edge: Color (`stroke`) and Text (`color`) pickers, Width 1 to 4, and Line Solid or
Dashed. Each applies to every selected edge as one drawing change. Tests: `edges_get_link_styles`
in `crates/sirene-gui/tests/styles.rs`, and `link_styles` in
`crates/sirene-core/tests/flowchart_edit.rs`.

## WYSIWY-19 Refused edits

An edit from the menus or keys that would change nothing says why (`sirene_gui::refusal`). In
the right-click menu, Group, Ungroup, Move Earlier, Move Later, Longer, and Shorter are disabled
with the reason after the name, e.g. `Ungroup (an edge to it has an id)`. From a key or the menu
bar, the status bar shows `Can't ACTION: REASON` until the document changes. The reasons:

- Group: no nodes are selected; the nodes are in different subgraphs.
- Ungroup: an edge to it has an id; it is empty and has edges; its first or last line has
  another statement.
- Move Earlier or Later: select one node or edge; nothing before (or after) it to swap with.
- Longer or Shorter: no edges are selected; already the longest (or shortest).
- Move To, or a drag onto a subgraph: it would move other nodes too. A node already in the
  target is not refused.

Tests: `crates/sirene-gui/tests/refusals.rs`.

## WYSIWY-20 Markdown files

A `.md` or `.markdown` file opens as its mermaid blocks (MARKDO-1), one at a time. The source pane
shows the shown block's code, and the drawing draws it; every edit to it, from the pane or the
drawing, goes back into that block (MARKDO-3), so the rest of the file is never changed. The
document is edited while the whole file differs from the text last read or saved; Save writes
the whole file, and disk changes and Reload work on the whole file, keeping the block shown.

- The status bar has previous and next buttons around `Diagram N of M (line L)`, L being the
  block's first code line. View > Next Diagram (secondary-shift-]) and Previous Diagram
  (secondary-shift-[) do the same, wrapping around. Showing another block clears the selection
  and the drawing's undo history, and fits the drawing.
- A file with no mermaid blocks shows an empty source and says so in the status bar; the first
  edit adds a block at the end of the file.
- Save As to a diagram file (`.mmd`) writes the shown block alone and edits that file from then
  on; Save As of a diagram to a Markdown file writes it in a mermaid block.
- Export suggests `STEM-N.png` or `.svg`, as `sirene png FILE.md` names block N.

Tests: `crates/sirene-gui/tests/markdown.rs`.
