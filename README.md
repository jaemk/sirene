# sirene

A local editor and viewer for Mermaid, PlantUML, and Graphviz DOT diagrams. Edit the source or
the drawing, and export SVG, PNG, or text for the terminal.

<p align="center">
  <img src="https://jaemk.github.io/sirene/hero.svg" alt="a flowchart drawn by sirene" width="640">
</p>

```
$ printf 'flowchart LR\n  A[Start] --> B{Ok?} -->|yes| C[Done]\n' | sirene ascii
┌───────┐    ╱─────╲       ┌──────┐
│ Start ├───►│ Ok? ├──yes─►│ Done │
└───────┘    ╲─────╱       └──────┘
```

* Two-way editing: the source sits beside a live drawing. Drag, connect, restyle, or relabel a
  flowchart in the drawing, and each change is written back to the source as a text edit.
* Three outputs: SVG and PNG for documents, box-drawing text for terminals, READMEs, and code
  comments.
* One binary: no browser, Node, or network needed to render. PNG text uses embedded fonts, so
  images are the same on every machine.
* Scripts and CI: `sirene check` exits 1 on errors, `svg` and `png` render the diagram blocks in
  Markdown files, and `--watch` renders again on each change.

Docs, with every diagram kind drawn: <https://jaemk.github.io/sirene/>

## Install

```
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/jaemk/sirene/main/bin/install.sh | sh
```

Or with Homebrew:

```
brew tap jaemk/sirene https://github.com/jaemk/sirene
brew trust jaemk/sirene
brew install sirene          # the binary
brew install --cask sirene   # or, on macOS, Sirene.app with `sirene` on the PATH
```

Or with Cargo: `cargo install sirene` (`--no-default-features` for the CLI only). Binaries are
also on the [releases page](https://github.com/jaemk/sirene/releases).

Note that the install script puts the CLI-only build in `~/.local/bin` on Linux; set
`SIRENE_TARGET=x86_64-unknown-linux-gnu` (or `aarch64-...`) for the GUI build, which needs the
system's X11/Wayland and Vulkan libraries. See [Install](https://jaemk.github.io/sirene/guide/install.html)
for the details.

## Usage

```
sirene diagram.mmd           # open the editor; follows changes on disk
sirene notes.md              # edit a Markdown file's mermaid blocks, one at a time
sirene ascii diagram.mmd     # print it as text (--charset ascii for plain ASCII)
sirene svg diagram.mmd       # print SVG (--theme light|dark, -o FILE)
sirene png diagram.mmd       # write diagram.png (--quality, --scale, --theme, -o FILE)
sirene png --watch docs/     # write a .png beside each diagram under docs/, again on each change
sirene svg README.md --link  # render each diagram block and link the images below it
sirene check docs/ a.md      # report errors only; exit 1 if any
sirene update                # replace sirene with the latest release
```

See [Command line](https://jaemk.github.io/sirene/guide/cli.html) for every option, and
[The editor](https://jaemk.github.io/sirene/guide/editor.html) and
[Keys](https://jaemk.github.io/sirene/guide/keys.html) for the GUI. `examples/` has diagrams to
try: `sirene examples/architecture.mmd`.

## What it draws

| Language | Diagrams |
|---|---|
| Mermaid | flowcharts (editable in the drawing), sequence, state, class, ER, git graphs, timelines, pie, gantt, mindmaps, user journeys, quadrant, XY, requirement, kanban, packet, radar, sankey, treemaps, block, architecture, C4, ZenUML, info, ishikawa, venn, Wardley maps, tree views, use case, railroad, swimlane, Cynefin, event modeling, agentflow |
| PlantUML | sequence, activity, class, object, state, use case, component, deployment, timing, network, mindmap, WBS, gantt, JSON, YAML, DOT, EBNF, regex, Chen ER, chronology, file trees |
| Graphviz | DOT graphs (`digraph`, `graph`), with records, clusters, and compass ports |

A file's language comes from its extension (`.mmd`, `.puml`, `.dot`, ...), its Markdown block
(`mermaid`, `plantuml`, `dot`), or `--lang`. Layouts are sirene's own, so drawings differ from
Mermaid's, PlantUML's, and Graphviz's. Each kind's page in the docs lists what is read and what
is not drawn; `spec/README.md` has the feature status table.

## Agents

The repo is a [mind](https://github.com/jaemk/mind) source with one skill, `sirene:render`, that
teaches agents to check and render diagrams with the CLI (`skills/render/SKILL.md`):

```
mind meld jaemk/sirene
```

## Development

```
bash scripts/ci.sh    # fmt, clippy, tests, and the coverage gate (needs cargo-llvm-cov)
```

On Linux, the GUI build needs the XCB, xkbcommon, Wayland, fontconfig, and Vulkan development
packages (see `.github/workflows/ci.yml`). Packaging and releases are in
[Development](https://jaemk.github.io/sirene/guide/development.html) and
[Releasing](https://jaemk.github.io/sirene/guide/releasing.html).

## License

MIT or Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`). Code ported from other projects and the
embedded fonts keep their own licenses; see `THIRD-PARTY-NOTICES`.
