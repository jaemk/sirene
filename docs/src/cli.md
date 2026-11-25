# Command line

```
sirene [FILE...]              open each FILE in the editor, or a new document
sirene ascii [FILE|-]         print a diagram, or a Markdown file's diagrams, as text
sirene svg [FILE|DIR|-]       print or write SVG
sirene png [FILE|DIR|-]       write PNG
sirene check [FILE|DIR|-]...  print diagnostics only
sirene update                 replace sirene with the latest release
```

`svg`, `png`, `ascii`, and `check` never open a window, and work in a build without the GUI. With
`-`, they read stdin; `svg`, `png`, and `ascii` also read stdin with no FILE.

## Languages

A diagram's language comes from its file extension (`.mmd` and `.mermaid` Mermaid, `.dot` and
`.gv` [Graphviz DOT](dot.md), `.puml`, `.plantuml`, `.pu`, and `.iuml` [PlantUML](plantuml.md))
or its Markdown block's name (`mermaid`, `dot`, `graphviz`, `plantuml`, `puml`). Stdin and other
files are DOT when they start with a DOT graph header, PlantUML when they have an `@start` line,
and Mermaid otherwise. `--lang mermaid|dot|plantuml|latex`, before or after the command, reads
every diagram as that language:

```
printf 'digraph { a -> b }\n' | sirene ascii --lang dot
```

A `.tex` or `.latex` file (or `--lang latex`, also spelled `tex`) is one formula; see
[LaTeX formulas](#latex-formulas). Directories do not pick up `.tex` files.

A `.vsdx` file (or `--lang visio`) is a [Visio drawing](visio.md): `svg` and `png` write each
page, or one with `--page N`; `ascii` is refused.

## Opening files

`sirene a.mmd b.mmd` opens a window for each file; `sirene` alone opens a new document with a
starter flowchart. A FILE named like a command needs a path prefix (`sirene ./ascii`). A FILE that
does not exist and looks like a mistyped command is refused:

```
$ sirene acsii
error: no file or command named 'acsii'; did you mean 'sirene ascii'? (to open a new file with that name, use './acsii')
```

This exits with status 2, and no window opens.

## `sirene ascii`

Prints the diagram with box-drawing characters. `--charset ascii` uses only ASCII
(`+`, `-`, `|`, `>`), including in formulas (`\/` for a radical, `alpha`, `<=`), and writes
stereotypes as `<<interface>>` rather than `«interface»`. The default `unicode` keeps formulas to
characters monospace fonts draw one cell wide; `--charset unicode-math` draws them with the full
math glyphs (radical sign, sum sign, superscript digits), which some fonts draw wider than a cell. A Markdown file prints each diagram block, with a blank line between
them.

```
$ printf 'flowchart TD\n  A --> B\n' | sirene ascii --charset ascii
+---+
| A |
+-+-+
  |
  v
+---+
| B |
+---+
```

`--width N` exits 1 with an error when a drawing is wider than N columns, still printing it,
e.g. to keep drawings in a README within 80 columns:

```
$ sirene ascii --width 80 README.md
```

`--markdown` reads stdin, or a file of any name, as Markdown; `sirene check --markdown` does the
same:

```
$ git show HEAD:README.md | sirene check --markdown -
```

## `sirene svg`

Prints a standalone SVG document. `--theme light|dark` picks the colors (default `light`), and
`-o, --output PATH` writes to PATH instead of stdout (`-o -` is stdout for a single diagram).
Directory and Markdown inputs write one image per diagram and refuse `-o -` with exit status 2.

## `sirene png`

Writes a PNG, with text drawn in the embedded Noto Sans faces so images are the same on every
machine.

| Option | Meaning |
|---|---|
| `-o, --output PATH` | where to write (`-` is stdout for a single diagram); default FILE with `.png`, or stdout when reading stdin |
| `--theme light\|dark` | the colors (default `light`) |
| `--quality draft\|normal\|fine` | the detail (default `fine`); large drawings are shrunk to fit |
| `--scale N` | pixels per diagram unit, instead of `--quality` |

sirene refuses to write a PNG to a terminal, or to overwrite the input file. Directory and
Markdown inputs refuse `-o -` with exit status 2.

## Directories

`sirene svg DIR` and `sirene png DIR` render every diagram file (by the extensions above, in any
case) under DIR, skipping hidden entries and symlinked directories. Each output is written beside its diagram, or with
`-o OUTDIR` at the same relative path under OUTDIR:

```
sirene png docs/                     # docs/a.mmd -> docs/a.png
sirene svg docs/ -o site/diagrams/   # docs/x/b.mmd -> site/diagrams/x/b.svg
```

One file's errors fail the run without stopping the others. When two diagrams would write the
same output (`a.mmd` beside `a.mermaid`), the first in path order wins and the other is reported.
On macOS and Windows, output paths compare without regard to case, so `A.mmd` and `a.mmd`
also collide.
Output directory symlinks are resolved when checking collisions. An output that would replace
any input, including the target of an input symlink, is skipped and reported.

## Markdown files

`sirene svg FILE.md` and `sirene png FILE.md` (also `.markdown`) render each `mermaid`, `dot`,
`graphviz`, `plantuml`, or `puml` code block to `STEM-N.svg` or `STEM-N.png` beside the file, N
counting blocks from 1; `-o OUTDIR` writes them there instead. Blocks inside blockquotes and
list items count, and the name may be in any case or in braces (`{.mermaid}`, `{mermaid}`).

The Markdown file is not changed unless `--link` is given. `--link` adds an image link on the
line after each rendered, closed block, and only when that line is not already the link, so
running it again changes nothing. When a new block shifts the numbering, the old
`![diagram N](...)` line is replaced. The link is relative to the Markdown file (`../img/guide-1.svg` for
`sirene svg docs/guide.md -o img --link`). The blocks themselves are never changed or removed.
An unclosed block is still rendered, but no image link is inserted for it.

```
$ sirene svg guide.md --link
wrote guide-1.svg
wrote guide-2.svg
guide.md:15:8: error: expected a node after the edge
wrote guide-3.svg
linked the images in guide.md
```

````
```mermaid
flowchart LR
  A[Write] --> B[Render]
```
![diagram 1](guide-1.svg)
````

Diagnostic positions are lines and columns in the Markdown file.

Formulas are blocks too, in source order with the diagrams, so N counts them: a fenced block
named `math`, `latex`, or `tex`, a `$$...$$` block, and an inline `$...$` (GitHub's rules: the
opening `$` is not followed by a space, the closing one is not preceded by one or followed by a
digit, `\$` is a dollar sign, and it stays on its line). Fenced and `$$` formulas are set in
display style, `$...$` in text style. Code blocks, code spans, and diagram blocks are not read for
formulas. `--link` links diagram blocks only. In a build without the `math` feature, formulas are
skipped with one warning per file.

## LaTeX formulas

`sirene svg|png|ascii|check FILE.tex` reads one formula: a bare formula, or one wrapped in
`$$...$$`, `$...$`, `\[...\]`, `\(...\)`, or a `displaymath` or `math` environment. `equation`,
`align`, `gather`, and the other math environments are passed through. `%` starts a comment. A
file with `\documentclass` or `\begin{document}` is refused with `full LaTeX documents are not
supported; give one formula` on its first line, unless the text between `\begin{document}` and
`\end{document}` is one math block. A formula that does not typeset is drawn as its source, with
one error at the offending command.

## `sirene check`

Prints diagnostics without rendering, for diagram files, directories (every diagram file under
them), Markdown files (each diagram block), or stdin. It checks every
FILE and exits 1 if any has an error, so it fits a pre-commit hook or a CI step:

```
$ sirene check guide.md docs/
guide.md:15:8: error: expected a node after the edge
```

`--json` prints the diagnostics to stdout as one JSON array, for editors and CI annotations:

```
$ sirene check --json guide.md
[
  {"file":"guide.md","line":15,"column":8,"severity":"error","message":"expected a node after the edge"}
]
```

`--markdown` reads stdin and files of any name as Markdown.

## `sirene update`

Replaces the running binary with the latest release, or the one `--to VERSION` names, after
asking (`--yes` skips the question). `--check` only reports. The download is the same
`sirene-VERSION-TARGET.tar.gz` the install script uses, for the running build's target, so a GUI
build stays a GUI build. Before anything is replaced it is checked against the release's
`SHA256SUMS` and against a signature by the project's release key, which is built into sirene.

```
$ sirene update --check
update available: 0.6.0 -> 0.6.1 (target x86_64-unknown-linux-gnu)
```

It never downgrades. A sirene installed by Homebrew or `cargo install` is refused with the
command that updates it (`brew upgrade sirene`, `brew upgrade --cask sirene`, or
`cargo install sirene`). `GH_TOKEN` or `GITHUB_TOKEN` authenticates the requests when set.
With both `--check` and `--to`, sirene compares versions locally; it does not check whether the
requested release or its archive exists.

## Watching

`-w, --watch` renders, then renders again each input that changes (and in a directory, each
diagram that appears) until interrupted. A watched Markdown file is rendered again when it
changes:

```
$ sirene png --watch docs/
watching docs/ (Ctrl-C to stop)
wrote docs/a.png
```

## Diagnostics and exit status

Problems are printed to stderr as `NAME:LINE:COL: error: MESSAGE` (or `warning:`), with LINE and
COL counting from 1. The diagram is still drawn when there are errors. The exit status is 1 when
there is an error (in the diagram, or reading or writing a file) and 0 otherwise. Input is read
as UTF-8; UTF-16 or other text is an error that says so (exit 1). Each input file or stdin stream
is limited to 64 MiB (67,108,864 bytes); larger inputs are refused with exit status 1. For a
Markdown input, the limit applies to the whole file, not each diagram block.

```
$ printf 'flowchart TD\n  A -->\n' | sirene ascii
<stdin>:2:8: error: expected a node after the edge
┌───┐
│ A │
└───┘
```

## Verbose output

`-v, --verbose`, before or after the command, prints each step (reading, parsing, layout,
rendering, writing, and in the editor opening, reloading, and saving files) with timings to
stderr:

```
$ sirene -v svg diagram.mmd -o diagram.svg
[     0.0ms] DEBUG sirene: sirene 0.6.0 (linux, x86_64): ...
[     0.1ms] DEBUG sirene: rendering 1 file(s)
[     0.1ms] DEBUG sirene: reading diagram.mmd
[     0.2ms] DEBUG sirene: read diagram.mmd: 23 bytes
[     0.4ms] DEBUG sirene_core::flowchart: parsed 23 bytes, 2 lines; 0 syntax diagnostics
[     0.5ms] DEBUG sirene_core::flowchart: lowered 2 nodes, 1 edges, 0 subgraphs, 0 class definitions; 0 diagnostics
[     0.5ms] DEBUG sirene: read as a flowchart
[     0.5ms] DEBUG sirene: parsed and lowered in 357.7µs
[     0.7ms] DEBUG sirene_core::layout: laid out 2 nodes, 0 clusters, 1 edges (1 routed) in 34x114 cells, 147.4µs
[     0.8ms] DEBUG sirene_core::scene::paint: drew a 65x145 scene of 6 elements
[     0.9ms] DEBUG sirene: rendered 731 bytes in 327.9µs
[     0.9ms] DEBUG sirene: writing diagram.svg
[     1.0ms] DEBUG sirene: exit with success
```

Repeat the flag (`-vv`) for trace logging. For GUI launches and scripts, logging can also be
enabled through the environment:

```sh
SIRENE_LOG=debug sirene diagram.mmd
SIRENE_LOG=sirene_core=trace,sirene_gui=debug sirene diagram.mmd
```

`SIRENE_LOG` takes precedence over `RUST_LOG`. Comma-separated directives accept `off`,
`error`, `warn`, `info`, `debug`, or `trace`, optionally prefixed by a module name and `=`.
The most specific module filter wins. Verbose flags override the general environment level
but keep explicit module filters. General levels apply to sirene's own crates; an explicit
module filter can enable a dependency. Invalid configuration prints an error and exits 2.
Logs go to stderr, leaving rendered stdout unchanged. No extra logs are printed without flags
or environment configuration. Operational errors include their underlying causes.

Trace logging of `sirene_core::profile` prints how long each stage of drawing took (reading,
layout ordering, edge routing, text or image output):

```sh
SIRENE_LOG=off,sirene_core::profile=trace sirene ascii diagram.mmd > /dev/null
```
