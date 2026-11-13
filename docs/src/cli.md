# Command line

```
sirene [FILE...]              open each FILE in the editor, or a new document
sirene ascii [FILE|-]         print a diagram, or a Markdown file's diagrams, as text
sirene svg [FILE|DIR|-]       print or write SVG
sirene png [FILE|DIR|-]       write PNG
sirene check [FILE|DIR|-]...  print diagnostics only
sirene update                 replace sirene with the latest release
```

`svg`, `png`, `ascii`, and `check` never open a window, and work in a build without the GUI. With no FILE,
or `-`, they read stdin.

## Languages

A diagram's language comes from its file extension (`.mmd` and `.mermaid` Mermaid, `.dot` and
`.gv` [Graphviz DOT](dot.md), `.puml`, `.plantuml`, `.pu`, and `.iuml` [PlantUML](plantuml.md))
or its Markdown block's name (`mermaid`, `dot`, `graphviz`, `plantuml`, `puml`). Stdin and other
files are DOT when they start with a DOT graph header, PlantUML when they have an `@start` line,
and Mermaid otherwise. `--lang mermaid|dot|plantuml`, before or after the command, reads every
diagram as that language:

```
printf 'digraph { a -> b }\n' | sirene ascii --lang dot
```

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
(`+`, `-`, `|`, `>`). A Markdown file prints each diagram block, with a blank line between
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

## `sirene svg`

Prints a standalone SVG document. `--theme light|dark` picks the colors (default `light`), and
`-o, --output PATH` writes to PATH instead of stdout.

## `sirene png`

Writes a PNG, with text drawn in the embedded Noto Sans faces so images are the same on every
machine.

| Option | Meaning |
|---|---|
| `-o, --output PATH` | where to write; default FILE with `.png`, or stdout when reading stdin |
| `--theme light\|dark` | the colors (default `light`) |
| `--quality draft\|normal\|fine` | the detail (default `fine`); large drawings are shrunk to fit |
| `--scale N` | pixels per diagram unit, instead of `--quality` |

sirene refuses to write a PNG to a terminal, or to overwrite the input file.

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

## Markdown files

`sirene svg FILE.md` and `sirene png FILE.md` (also `.markdown`) render each `mermaid`, `dot`,
`graphviz`, `plantuml`, or `puml` code block to `STEM-N.svg` or `STEM-N.png` beside the file, N
counting blocks from 1; `-o OUTDIR` writes them there instead.

The Markdown file is not changed unless `--link` is given. `--link` adds an image link on the
line after each rendered block, and only when that line is not already the link, so running it
again changes nothing. The link is relative to the Markdown file (`../img/guide-1.svg` for
`sirene svg docs/guide.md -o img --link`). The blocks themselves are never changed or removed.

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

## `sirene check`

Prints diagnostics without rendering, for diagram files, directories (every diagram file under
them), Markdown files (each diagram block), or stdin. It checks every
FILE and exits 1 if any has an error, so it fits a pre-commit hook or a CI step:

```
$ sirene check guide.md docs/
guide.md:15:8: error: expected a node after the edge
```

## `sirene update`

Replaces the running binary with the latest release, or the one `--to VERSION` names, after
asking (`--yes` skips the question). `--check` only reports. The download is the same
`sirene-VERSION-TARGET.tar.gz` the install script uses, for the running build's target, so a GUI
build stays a GUI build. It is checked against the release's `SHA256SUMS`, and against its build
provenance when `gh` is installed, before anything is replaced.

```
$ sirene update --check
update available: 0.2.0 -> 0.3.0 (target x86_64-unknown-linux-gnu)
```

It never downgrades. A sirene installed by Homebrew or `cargo install` is refused with the
command that updates it (`brew upgrade sirene`, `brew upgrade --cask sirene`, or
`cargo install sirene`). `GH_TOKEN` or `GITHUB_TOKEN` authenticates the requests when set.

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
there is an error (in the diagram, or reading or writing a file) and 0 otherwise.

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
[     0.0ms] sirene: sirene 0.1.1 (linux, x86_64): ...
[     0.1ms] sirene: rendering 1 file(s)
[     0.1ms] sirene: reading diagram.mmd
[     0.2ms] sirene: read diagram.mmd: 23 bytes
[     0.4ms] sirene_core::flowchart: parsed 23 bytes, 2 lines; 0 syntax diagnostics
[     0.5ms] sirene_core::flowchart: lowered 2 nodes, 1 edges, 0 subgraphs, 0 class definitions; 0 diagnostics
[     0.5ms] sirene: parsed and lowered in 357.7µs
[     0.7ms] sirene_core::layout: laid out 2 nodes, 0 clusters, 1 edges (1 routed) in 34x114 cells, 147.4µs
[     0.8ms] sirene_core::scene::paint: drew a 65x145 scene of 6 elements
[     0.9ms] sirene: rendered 731 bytes in 327.9µs
[     0.9ms] sirene: writing diagram.svg
[     1.0ms] sirene: exit with success
```
