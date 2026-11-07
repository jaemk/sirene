# WYSIWYG Editing

Editing a diagram in its window: the source text in a pane beside the drawing, and later edits
made on the drawing itself, written back to the source. Tests: `crates/sirene-gui/tests/editor.rs`
and the unit tests in `crates/sirene-gui/src/document.rs`.

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
