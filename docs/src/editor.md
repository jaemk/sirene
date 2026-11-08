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
  there is one, and opens its label.
- Connect: drag from one node to another (or to itself).
- Delete: backspace or delete removes the selection, with the edges that touch removed nodes.
  A chain such as `A --> B --> C` that loses its middle is split, and `class`, `style`,
  `click`, and `linkStyle` statements are updated to match.

New nodes get the first unused id of `A` to `Z`, then `n1`, `n2`, ..., and are appended at the
end of the document.

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

## Export

- File > Export as PNG (secondary-shift-e) and Export as SVG write the drawing in the window's
  colors, as `sirene png` and `sirene svg` would.
- Secondary-shift-c copies the drawing to the clipboard as a PNG.

## Appearance

View > Appearance follows the system's light or dark mode by default, or forces Light or Dark
for every window. See [Settings](settings.md).
