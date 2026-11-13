---
name: render
description: Render Mermaid, PlantUML, and Graphviz DOT diagrams to PNG, SVG, or terminal text with the sirene CLI, and check them for errors. Use to look at a diagram you wrote, show one in the terminal, make images for docs, or check the diagram blocks in a Markdown file.
---

# render

`sirene` renders Mermaid, PlantUML, and Graphviz DOT diagrams to PNG, SVG, and text. Mermaid
flowcharts and over 30 other Mermaid kinds are drawn (`sequenceDiagram`, `classDiagram`,
`stateDiagram-v2`, `erDiagram`, `gantt`, ...); a Mermaid type it does not know is reported as
unsupported. The language comes from the extension (`.mmd`, `.puml`, `.dot`, ...), the Markdown
block name (`mermaid`, `plantuml`, `puml`, `dot`, `graphviz`), or `--lang mermaid|dot|plantuml`.

## Setup

Check that `sirene` is on the PATH with `sirene --version`. If it is not, ask the user before
installing it. Any of these gives the CLI:

```
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/jaemk/sirene/main/bin/install.sh | sh
brew tap jaemk/sirene https://github.com/jaemk/sirene && brew install sirene
cargo install sirene --no-default-features
```

## Commands

Run these from the directory holding the diagram. None of them open a window. Never run
`sirene FILE` with no command: that opens the GUI editor.

```sh
sirene check diagram.mmd                     # diagnostics only; exit 1 on an error
sirene ascii diagram.mmd                     # print as box-drawing text
sirene ascii --charset ascii diagram.mmd     # print with ASCII characters only
sirene png diagram.mmd -o diagram.png        # write a PNG
sirene png diagram.mmd -o dark.png --theme dark --quality normal
sirene svg diagram.mmd -o diagram.svg        # write an SVG (stdout without -o)
sirene check notes.md docs                   # each diagram block, every diagram under docs/
sirene ascii notes.md                        # print each diagram block as text
sirene png notes.md -o out                   # out/notes-1.png, ... one per diagram block
sirene svg docs -o out                       # out/a.svg, ... one per diagram under docs/
```

With no FILE, or `-`, a command reads stdin:

```
printf 'flowchart LR\n  A --> B\n' | sirene ascii
printf 'flowchart LR\n  A --> B\n' | sirene png -o ab.png
```

`png` refuses to write to a terminal and to overwrite its input; pass `-o`. `--quality` is
`draft`, `normal`, or `fine` (the default, 4x). `sirene png notes.md --link` also adds an image
link after each block in the Markdown file; leave it off unless the user asks for that.

## Diagnostics

Problems go to stderr as `FILE:LINE:COL: error: MESSAGE` (or `warning:`), lines and columns from
1, in the Markdown file's coordinates for a block. The diagram is still drawn when there are
errors, and the exit status is 1. Fix every error before handing a diagram back.

## Looking at a diagram

To check how a diagram you wrote looks:

1. Write it to a `.mmd`, `.puml`, or `.dot` file, or a diagram block in Markdown.
2. `sirene check` it and fix the errors.
3. Render a PNG to a scratch path (`-o`) and read the image; or `sirene ascii` it to see the
   layout as text, which is also what to show the user in a terminal.
4. Adjust direction (`flowchart LR` or `TD`, `rankdir`, `left to right direction`), grouping,
   and edge lengths (`-->` vs `--->`) until it reads well, then render the final output where
   the user wants it.

What each diagram kind reads is at <https://jaemk.github.io/sirene/guide/>.
