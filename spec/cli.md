# CLI

The `sirene` binary. Tests: `crates/sirene/tests/cli.rs` and the unit tests in
`crates/sirene/src/main.rs`.

## CLI-1 No subcommand

`sirene [FILE...]` with no subcommand opens each FILE in an editor window, or one new document
with a starter flowchart (WYSIWY-7) when no FILE is given (`gui-shell.md`). It exits 0 when the
last window closes. A FILE named like a subcommand is given with a path prefix, e.g. `./ascii`.
A first FILE that does not exist, has no `/`, `\`, or `.`, and is within two edits of a
subcommand is taken as a mistyped command: `error: no file or command named 'NAME'; did you mean
'sirene CMD'? (to open a new file with that name, use './NAME')`, exit 2, and no window opens.
Tests: `suggests_a_misspelled_command`, and `misspelled_commands` in `main.rs`.

## CLI-2 `sirene ascii`

`sirene ascii [FILE]` reads a flowchart from FILE, or from stdin when FILE is omitted or `-`,
and prints it to stdout with the ASCII renderer (`ascii-renderer.md`).

`--charset unicode|ascii` picks the character set; the default is `unicode`.

## CLI-3 Diagnostics

Diagnostics are printed to stderr as `NAME:LINE:COL: error: MESSAGE` (or `warning:`), where NAME
is the file path or `<stdin>`, and LINE and COL count from 1 (COL in characters). The diagram is
printed even when there are errors. The exit status is 1 when there is an error and 0 otherwise.

## CLI-4 Read failures

When FILE cannot be read, sirene prints `error: cannot read FILE: REASON` to stderr and exits 1.

## CLI-5 `sirene svg`

`sirene svg [FILE]` reads a flowchart as `sirene ascii` does and prints it to stdout as a
standalone SVG document (`display-list.md`), with text measured by the monospace estimate.
`--theme light|dark` picks the colors; the default is `light`. `-o, --output PATH` writes the
SVG to PATH instead of stdout; a PATH that is FILE itself is refused before anything is read
(`error: refusing to overwrite the input FILE; use --output`, exit 1). Diagnostics and exit
status follow CLI-3 and CLI-4.

## CLI-6 `sirene png`

`sirene png [FILE]` reads a flowchart as `sirene ascii` does and writes it as a PNG
(DISPLA-11), with text measured by the embedded font, without opening a window. `--theme` is as
for `sirene svg`; `--quality draft|normal|fine` (default `fine`) picks the detail (DISPLA-12),
drawn at that quality's scale, shrunk to fit when the image would be too large; `--scale N` sets
pixels per diagram unit instead, and a scale too large is an error. The PNG goes to
`-o, --output PATH`; without it, to FILE with its extension replaced by `.png`, or to stdout
when reading stdin. Writing a PNG to a terminal, or over FILE (as for `sirene svg`), is
refused. Diagnostics and exit status follow CLI-3 and CLI-4; a write or render failure prints
`error: ...` and exits 1.

## CLI-7 Headless builds

The `gui` feature of the `sirene` crate is on by default. Built without it
(`--no-default-features`), the binary does not link the GUI or its system libraries, the
subcommands work as above, and running with no subcommand prints
`error: built without the GUI; see sirene --help` and exits 1.

## CLI-8 Verbose output

`-v, --verbose`, before or after the subcommand, prints a line to stderr for each step, as
`[ELAPSEDms] TARGET: MESSAGE` where TARGET is the module that logged it: the version and
parsed command; reading the input (path or `<stdin>`, size); parsing and lowering (sizes, item
and diagnostic counts, time); layout (counts, size in cells, time); the scene and PNG encoding;
the bytes rendered and where they are written; and the exit status. In the GUI it also reports
opening files and windows, a file already open, ignored `file://` URLs, changes found on disk,
reloads, saves (including writing in place when the owner cannot be kept), and the answers to
the Reload, Overwrite, Save, and Quit questions. Only sirene's own crates log; stdout is
unchanged, and without the flag nothing extra is printed. Tests: `verbose_reports_each_step`
in `crates/sirene/tests/cli.rs` and `arguments` in `crates/sirene/src/main.rs`.

## CLI-9 Directories

`sirene svg DIR` and `sirene png DIR` render every `.mmd`, `.mermaid`, `.dot`, and `.gv` file
under DIR, recursing
into subdirectories but skipping hidden entries and symlinked directories, in path order. Each is
written as `.svg` or `.png` beside it, or with `-o OUTDIR` to the same relative path under OUTDIR
(directories are created as needed). Diagnostics follow CLI-3 for each file; one file's errors
fail the run without stopping the others. When two diagrams would write the same output
(`a.mmd` beside `a.mermaid`), the first in path order is rendered and each later one is
skipped with `error: skipping LATER; FIRST also writes OUTPUT`, which fails the run (in a watch,
the line is printed again only when the set of skipped files changes). These are errors that
render nothing: a DIR with no diagrams (`error: no .mmd, .mermaid, .dot, .gv, .puml, .plantuml,
.pu, or .iuml files in DIR`), a DIR that
cannot be listed (`error: cannot read DIR: REASON`), an OUTDIR that exists and is not a directory
(`error: OUTDIR is not a directory; a DIR renders into one`), and a directory given to
`sirene ascii` (`error: DIR is a directory; ascii reads one file`). Tests: `directories`,
`directory_output_collisions`, and `unreadable_directory` in `crates/sirene/tests/cli.rs`.

## CLI-10 Watching

`-w, --watch` on `ascii`, `svg`, and `png` renders as usual, then keeps running until interrupted:
every 500ms it renders again each input whose modification time or length changed, and in a
directory, each diagram that appeared. It prints `watching PATH (Ctrl-C to stop)` to stderr at
start, then `wrote OUTPUT` (or `printed FILE` for stdout) after each render, along with any
diagnostics and read or write errors; none of these stop the watch. Watching stdin is an error
(`error: --watch needs a FILE or directory, not stdin`). Tests: `watch_needs_a_path`,
`watch_renders_changes`, and `watch_a_file` in `crates/sirene/tests/cli.rs`.

## CLI-11 `sirene check`

`sirene check FILE...` (at least one) reads each FILE, draws nothing, writes nothing to stdout,
and prints diagnostics as in CLI-3. A FILE may be a diagram, `-` for stdin, a directory (each
diagram file under it, as in CLI-9; none is `error: no .mmd, .mermaid, .dot, .gv, .puml,
.plantuml, .pu, or .iuml files in DIR`), or a Markdown file (`.md` or `.markdown`), whose diagram blocks (MARKDO-1) are checked with
positions in the Markdown file. Every FILE is checked. The exit status is 1 when any FILE cannot be read or has an error, else 0. A mistyped
`check` is suggested as in CLI-1. Tests: `check` in `crates/sirene/tests/cli.rs`.

## CLI-12 Markdown files

`sirene svg FILE.md` and `sirene png FILE.md` (or `.markdown`) write each diagram block
(MARKDO-1) of the file as its own image, `STEM-N.svg` or `STEM-N.png` beside it, where N counts
the file's diagram blocks from 1; `-o DIR` writes them into DIR instead (created as needed; an
existing file there is `error: DIR is not a directory; a Markdown file renders one image per
block into one`). Each image written is reported as `wrote PATH`; a file with no diagram blocks
prints `NAME: no diagram blocks`. Diagnostics have positions in the Markdown file. The exit status is 1 when the file cannot be
read, a block has an error, or an image cannot be written.

The Markdown file is never changed, except with `--link`: then, after each closed block that
was written, a line `![diagram N](PATH)` (PATH relative to the Markdown file's directory, with
`..` to leave it; an absolute `-o` outside that directory stays absolute) is inserted unless the
line after the block already is that line, and `linked the images in NAME` is printed. The
diagram blocks themselves are never removed or changed. `--watch` renders again whenever the file changes; the links it adds do not count as
a change. Tests: `markdown_images`, `markdown_links_from_the_file`, and `watch_markdown` in
`crates/sirene/tests/cli.rs`, and
the unit tests in `crates/sirene/src/markdown.rs`.

## CLI-13 `--lang`

`--lang mermaid|dot|plantuml`, a global option given before or after the command, reads every diagram
`check`, `ascii`, `svg`, and `png` read (files, directory entries, Markdown blocks, and stdin)
as that language. Without it, a file's extension or a block's info string picks the language,
and other sources are detected (LANGS-1, DOT-5). Tests: `forced_languages` in
`crates/sirene/tests/cli.rs`.
