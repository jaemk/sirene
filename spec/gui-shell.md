# GUI Shell

The app window (`crates/sirene-gui`): one window per diagram file. Tests:
`crates/sirene-gui/tests/viewer.rs` and the unit tests in `document.rs` and `lib.rs`.

## GUISHE-1 Launching

`sirene_gui::run(paths)` starts the app and opens one window per path, or one new document
(WYSIWY-7) when there are none. Files the platform asks the app to open (`file://` URLs, e.g. from Finder) open
in new windows; a URL naming a host other than `localhost` is ignored. A file that a window
already shows (compared after resolving links and `..`) brings that window forward instead of
opening another, however it is opened. The app quits when its last window closes.

## GUISHE-2 Viewer

A window shows its diagram drawn from the display list (`display-list.md`), with labels measured
by gpui's text system, right of the source pane (WYSIWY-1) and over a status bar. The drawing is centered when it fits and starts at the
top left when it does not. Scrolling pans; scrolling with the secondary modifier zooms. Zoom is
clamped to 10%-800%. The window title is the file name, or `Untitled`.

## GUISHE-3 Documents and reload

A document is a file's source, its lowered flowchart, and its diagnostics with 1-based line and
column. A file that cannot be read shows as an empty diagram with the read error. The file is
re-read every 500ms and on the Reload action, and the drawing updates when the contents change.

## GUISHE-4 Painting

Paths are filled and stroked with gpui paths (arcs and cubics included, dashes kept); text is
shaped with the window's default font and centered on its point. Painting scales every length by
the zoom.

## GUISHE-5 Actions and keys

`secondary` is Cmd on macOS and Ctrl elsewhere.

| Action | Keys | Effect |
|--------|------|--------|
| ZoomIn | secondary-= or secondary-+ | zoom by 1.25 |
| ZoomOut | secondary-- | zoom by 1/1.25 |
| ActualSize | secondary-0 | 100%, no pan |
| FitWindow | secondary-9 | zoom so the drawing fits, no pan |
| Reload | secondary-r | re-read the file, asking first when there are unsaved edits (WYSIWY-5) |
| New | secondary-n | open a new document (WYSIWY-7) |
| Open | secondary-o | choose files; an unedited Untitled window shows the first, others open new windows |
| Save | secondary-s | save (WYSIWY-4) |
| SaveAs | secondary-shift-s | save to a chosen path (WYSIWY-4) |
| CloseWindow | secondary-w | close the window (WYSIWY-6) |
| Quit | secondary-q | quit (WYSIWY-6) |
| ToggleSource | secondary-e | show or hide the source pane (WYSIWY-1) |
| AddNode | secondary-shift-n | add a node (WYSIWY-11) |
| Rename | enter on the drawing | edit the selected item's label (WYSIWY-10) |
| Delete | backspace or delete on the drawing | delete the selection (WYSIWY-12) |
| Deselect | escape on the drawing | clear the selection (WYSIWY-9) |
| Undo | secondary-z on the drawing | undo a drawing change (WYSIWY-13) |
| Redo | secondary-shift-z on the drawing | redo a drawing change (WYSIWY-13) |
| ZoomToSelection | secondary-8 | fit the selection (GUISHE-8) |
| Pan | arrow keys on the drawing, shift for further | move the drawing (GUISHE-8) |
| SelectAll | secondary-a on the drawing | select every node and edge (GUISHE-8) |
| SelectNext, SelectPrevious | tab, shift-tab on the drawing | select the next or previous node (GUISHE-8) |
| ExportPng | secondary-shift-e | export as PNG (GUISHE-9) |
| ExportSvg | (menu) | export as SVG (GUISHE-9) |
| CopyImage | secondary-shift-c | copy the drawing as an image (GUISHE-9) |

The same actions are in the app menus (sirene, File, Edit, View). The window is built on GPUI Kit
(`gpui-kit`: gpui-pre, gpui-base, gpui-component), whose light or dark theme follows GUISHE-6.

## GUISHE-6 Appearance

The window draws with `Theme::DARK` when the system appearance is dark and `Theme::LIGHT`
otherwise, and redraws when the appearance changes.

## GUISHE-7 Status bar

The status bar shows the first of: a save error (WYSIWY-4), a change on disk under unsaved edits
(WYSIWY-5), the read error, the first error (else first warning) as `LINE:COL: SEVERITY: MESSAGE`
with `(+N more)` when there are others, or the window title (WYSIWY-3); and the zoom as a
percentage.

## GUISHE-8 Navigation

The first time a document is drawn in a window (on opening, or shown in a blank window), a
drawing larger than the drawing area is zoomed to fit it; smaller drawings stay at 100%.
ZoomToSelection zooms (up to 200%, and no lower than 10%) so the selection's bounds fit the
drawing area with a 40px margin, centered; it does nothing with no selection. The arrow keys pan
by 40px, with shift by 200px, showing more of the drawing on the side the arrow points to.
SelectAll selects every node, then every edge. SelectNext and SelectPrevious select the node
after or before the last selected node in source order, wrapping around (the first or last node
when no node is selected), and pan to center it when any of it is outside the drawing area.
While a label is being edited these keys go to its field. Tests:
`crates/sirene-gui/tests/navigation.rs`.

## GUISHE-9 Export and copy

ExportPng and ExportSvg ask for a path (in the document's directory, or the working directory for
Untitled, named after the document with `.png` or `.svg`, else `diagram`) and write the drawing
there in the window's theme: PNG as `sirene png` writes it at fine quality (DISPLA-12), SVG as
`sirene svg` does. CopyImage puts the drawing on the clipboard as a PNG at normal quality. The
status bar shows `exported PATH`, `copied the drawing as an image`, or the error (`cannot write
PATH: REASON`) until the document next changes; a cancelled dialog changes nothing. Tests:
`crates/sirene-gui/tests/export.rs`.
