# GUI Shell

The gpui viewer (`crates/sirene-gui`): one window per diagram file. Tests:
`crates/sirene-gui/tests/viewer.rs` and the unit tests in `document.rs` and `lib.rs`.

## GUISHE-1 Launching

`sirene_gui::run(paths)` starts the app and opens one window per path, or one empty window when
there are none. Files the platform asks the app to open (`file://` URLs, e.g. from Finder) open
in new windows. The app quits when its last window closes.

## GUISHE-2 Viewer

A window shows its diagram drawn from the display list (`display-list.md`), with labels measured
by gpui's text system, over a status bar. The drawing is centered when it fits and starts at the
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
| Reload | secondary-r | re-read the file |
| Open | secondary-o | choose files; an empty window shows the first, others open new windows |
| CloseWindow | secondary-w | close the window |
| Quit | secondary-q | quit |

The same actions are in the app menus (sirene, File, View).

## GUISHE-6 Appearance

The window draws with `Theme::DARK` when the system appearance is dark and `Theme::LIGHT`
otherwise, and redraws when the appearance changes.

## GUISHE-7 Status bar

The status bar shows the read error, or the first error (else first warning) as
`LINE:COL: SEVERITY: MESSAGE` with `(+N more)` when there are others, or the file name when there
are no problems; and the zoom as a percentage.
