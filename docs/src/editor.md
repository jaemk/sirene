# The editor

`sirene FILE` opens a window with the source on the left and a live drawing on the right.
Secondary below is Cmd on macOS and Ctrl elsewhere. Every action is also in the menus: the menu
bar on macOS, a bar at the top of each window elsewhere. [Keys](keys.md) lists them all.

## Source and drawing

- Every change to the source redraws the diagram. Errors and warnings are underlined in the
  source and the first one is shown in the status bar, with the zoom.
- Secondary-e shows or hides the source pane; the divider between the panes drags.
- Click the drawing to give it the keyboard. It shows a frame in the accent color while it has
  it; keys such as enter, backspace, the arrows, and tab go to the drawing only then.

## Editing on the drawing

Each change on the drawing is written to the source as one ordinary text edit: comments,
spacing, and statement order are kept, and secondary-z undoes it.

- Select: click a node or edge; shift-click adds or removes. Escape or a click on empty space
  clears the selection. Selecting one item highlights its source.
- Rename: double-click an item, or press enter with one selected, to edit its label in place.
  `<br>` is a line break. Enter applies, escape cancels. An empty edge label removes it.
- Add: secondary-shift-n adds a node labeled `New node`, linked from the selected node when
  there is one, and opens its label. Double-click empty space to add a node there.
- Shape bar: the buttons at the top left of the drawing add a node of that shape, linked from
  the selected node when there is one.
- Connect: drag from one node to another (or to itself). Drag from a node to empty space to add
  a new node linked from it.
- Delete: backspace or delete removes the selection, with the edges that touch removed nodes.
  A chain such as `A --> B --> C` that loses its middle is split, and `class`, `style`,
  `click`, and `linkStyle` statements are updated to match.

New nodes get the first unused id of `A` to `Z`, then `n1`, `n2`, ..., and are appended at the
end of the document (or of the subgraph they were added in).

## The right-click menu

Right-click the drawing for a menu of what applies to the selection, or to what is under the
pointer:

- Nodes: Shape, Add Node Before or After, Connect To and Connect From any other node, and the
  layout hints below.
- Edges: Line (solid, thick, dotted, invisible), Arrows (at either end, both, none, or a circle
  or cross head), Reverse, Longer and Shorter, Move Earlier and Move Later.
- A subgraph: Add Node inside it, its Direction, Ungroup.
- Empty space: Add Node, the diagram's Direction, Select All, Fit Window.
- Rename and Delete, for the selection.

With several items selected, the menu applies to all of them. An item that would change nothing
is disabled with the reason after it, such as `Move Earlier (nothing before it to swap with)`;
from a key, the status bar says `Can't move earlier: ...` instead.

## Layout hints

Mermaid has no positions, so sirene changes the layout through the source:

- Direction sets `flowchart TD`, `LR`, and so on, or a subgraph's `direction` line.
- Group (secondary-g) puts the selected nodes in a new subgraph; Ungroup removes one and keeps
  its contents. An edge to the subgraph then goes to each of its members (`A & B --> C`).
- Move To moves a node into another subgraph, or out to the top level. Dragging a node onto a
  subgraph does the same. A statement such as `A --> B` moves with the node, and B keeps a
  mention in the subgraph it was in.
- Keep Together adds an invisible `~~~` edge between two nodes, which places them next to each
  other.
- Move Earlier and Move Later (alt-up, alt-down) swap a statement with its neighbor; the layout
  follows statement order. Edge `linkStyle` numbers are updated to match.

## Styles

View > Show or Hide Styles (secondary-i) opens a panel listing the document's classes:

- Check a class to give it to the selected nodes, uncheck to take it away.
- New Class defines a class with a light fill and a blue stroke, gives it to the selected nodes,
  and opens it for editing.
- Edit shows a class's fill, stroke, and text color pickers, the stroke width, and solid or
  dashed lines.

The panel writes `classDef` and `class` statements, so one change restyles every node with that
class.

With edges selected, an Edge section sets their line color, label color, width, and dashes. It
writes one `linkStyle N ...` per edge, replacing any other `linkStyle` that named it (except
`linkStyle default`).

## Moving around

- Scroll or drag empty space to pan; the arrow keys pan by a step, shift for further.
- Secondary-scroll zooms. Secondary-= and secondary-- zoom in and out, secondary-0 is 100%,
  secondary-9 fits the window, secondary-8 fits the selection.
- Tab and shift-tab step through the nodes in source order.
- A drawing larger than the window is zoomed to fit when it is first shown.

## Files

- Secondary-s saves, secondary-shift-s saves as. A save replaces the file through a temporary
  file, keeping its permissions and owner, and a symlink stays a symlink.
- The window title says `(edited)` while there are unsaved changes, and closing or quitting asks
  whether to save them.
- A file changed on disk is reloaded when there are no unsaved edits. Otherwise the status bar
  says so; Reload (secondary-r) asks before discarding the edits, and Save asks before
  overwriting the newer file.
- File > Open Recent lists the last 10 files.

## Markdown files

`sirene notes.md` opens a Markdown file's `mermaid` code blocks one at a time. The source pane
shows the current block's code, and every edit, typed or made on the drawing, is written back
into that block; the rest of the file is not touched. Save writes the whole file.

- The status bar shows `Diagram N of M (line L)` with buttons for the previous and next block;
  secondary-shift-] and secondary-shift-[ do the same.
- In a file with no mermaid blocks, the first edit adds one at the end.
- Save As to a `.mmd` file writes the current block on its own.
- Export suggests `notes-N.png`, the name `sirene png notes.md` gives block N.

## Export

- File > Export as PNG (secondary-shift-e) and Export as SVG write the drawing in the window's
  colors, as `sirene png` and `sirene svg` would.
- Secondary-shift-c copies the drawing to the clipboard as a PNG.

## Appearance

View > Appearance follows the system's light or dark mode by default, or forces Light or Dark
for every window. See [Settings](settings.md).
