# WYSIWYG Editing

Editing a diagram in its window: the source text in a pane beside the drawing, and edits made on
the drawing itself, written back to the source. Tests: `crates/sirene-gui/tests/editor.rs`,
`crates/sirene-gui/tests/drawing.rs`, and the unit tests in `crates/sirene-gui/src/document.rs`.

## WYSIWY-1 Source pane

Each window has a source pane to the left of the drawing: a code editor (gpui-component's
`Editor`) with line numbers, undo/redo, and search over the document's text. ToggleSource
(secondary-e) shows and hides it; it is shown by default. Hiding it keeps its text, undo history,
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
in the same directory, and the document is no longer edited. Save As (secondary-shift-s), and
Save on an Untitled document, ask for a path first; the window then shows that file. A failed
write leaves the document edited and shows the error in the status bar.

## WYSIWY-5 Changes on disk

The file poll of GUISHE-3 keeps running. When the file changes and the document is not edited, the
new text replaces the editor's text, keeping the cursor line where it still exists. When the
document is edited, the editor keeps its text and the status bar says the file changed on disk;
Reload replaces the edits with the file's text.

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
  last top-level statement. New node ids are the first unused of `A` to `Z`, then `n1`, `n2`, ...
- `delete`: removes nodes, edges, and every edge touching a removed node. A chain that loses a
  middle edge or node is split into one statement per remaining piece, on its own line. A piece
  left as a single bare node is dropped when the node was already mentioned before it in the same
  subgraph (or at the top level), so node order, subgraph membership, and what later `class` and
  `style` statements apply to stay the same. `class` lists drop
  removed ids, `style` and `click` statements for removed nodes go, `linkStyle` indexes are
  renumbered, and `id@{ ... }` statements of removed edges go. A statement that ends up empty
  is removed with its line when it was alone on it, otherwise with the `;` between it and its
  neighbor.
- Labels are written bare when they are one line of letters, digits, spaces, and `_.,!?'`;
  otherwise quoted, with `"`, `#`, `<`, and (outside markdown) `` ` `` as entity codes and line
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
a line follows the pointer while dragging. Dragging from empty space pans.

## WYSIWY-12 Deleting

Delete (backspace or delete, with the drawing focused) removes the selected nodes and edges
(WYSIWY-8) and clears the selection.

## WYSIWY-13 Drawing edits and undo

Changes made on the drawing are applied to the source pane's text as one edit each, so they mark
the document edited and the pane's own undo reverts them. Undo (secondary-z) and Redo
(secondary-shift-z) with the drawing focused step through the drawing's changes while the text
still matches; typing in the pane clears that history.
