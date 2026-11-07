# GUI Shell

The app window (`crates/sirene-gui`): one window per diagram file. Tests:
`crates/sirene-gui/tests/viewer.rs` and the unit tests in `document.rs` and `lib.rs`.

## GUISHE-1 Launching

`sirene_gui::run(paths)` starts the app and opens one window per path, or one new document
(WYSIWY-7) when there are none. Files the platform asks the app to open (`file://` URLs, e.g. from Finder) open
in new windows. The app quits when its last window closes.

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
| Reload | secondary-r | re-read the file |
| New | secondary-n | open a new document (WYSIWY-7) |
| Open | secondary-o | choose files; an unedited Untitled window shows the first, others open new windows |
| Save | secondary-s | save (WYSIWY-4) |
| SaveAs | secondary-shift-s | save to a chosen path (WYSIWY-4) |
| CloseWindow | secondary-w | close the window (WYSIWY-6) |
| Quit | secondary-q | quit (WYSIWY-6) |
| ToggleSource | secondary-e | show or hide the source pane (WYSIWY-1) |

The same actions are in the app menus (sirene, File, View). The window is built on GPUI Kit
(`gpui-kit`: gpui-pre, gpui-base, gpui-component), whose light or dark theme follows GUISHE-6.

## GUISHE-6 Appearance

The window draws with `Theme::DARK` when the system appearance is dark and `Theme::LIGHT`
otherwise, and redraws when the appearance changes.

## GUISHE-7 Status bar

The status bar shows the first of: a save error (WYSIWY-4), a change on disk under unsaved edits
(WYSIWY-5), the read error, the first error (else first warning) as `LINE:COL: SEVERITY: MESSAGE`
with `(+N more)` when there are others, or the window title (WYSIWY-3); and the zoom as a
percentage.
