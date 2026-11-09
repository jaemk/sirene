# Changelog

All notable changes to `sirene` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `sirene update` replaces the running binary with the latest release (`--check`, `--to
  VERSION`, `--yes`), checked against `SHA256SUMS` and its build provenance; it refuses for a
  Homebrew or `cargo install` binary and names the command to use instead.
- Mermaid state diagrams (`stateDiagram`, `stateDiagram-v2`) in every command and in the
  editor, where the drawing is read-only: states, transitions, `[*]`, composite states,
  concurrent regions, fork, join, and choice, notes, `direction`, and styling.

### Changed

- In ASCII, start and stop circles (`sm-circ`, `f-circ`, `fr-circ`) are one character, a `fork`
  is a heavy bar, and an unlabeled diamond is `◇`.

## [0.2.0] - 2026-11-08

### Added

- A documentation site at <https://jaemk.github.io/sirene/>: a landing page and an mdBook
  guide, with the examples rendered by sirene at build time.
- `sirene png --quality draft|normal|fine` (default `fine`, 4x); the default scale shrinks
  to fit the largest image.
- `sirene svg DIR` and `sirene png DIR` render every `.mmd` and `.mermaid` file under a
  directory, beside each or under `-o OUTDIR`.
- `-w`/`--watch` on `ascii`, `svg`, and `png` renders again when an input changes.
- `-v`/`--verbose` logs each step, with timings, to stderr.
- Markdown bold and italic in labels; long labels wrap.
- `examples/` with diagrams to try.
- Editor: fit large drawings on open, zoom to selection, arrow-key panning, select all, and
  tab/shift-tab through nodes.
- Editor: File > Export as PNG or SVG, and Edit > Copy Image.
- Editor: View > Appearance (Follow System, Light, Dark), File > Open Recent, and a resizable
  source pane, kept in a settings file with the window size.
- Editor: the menus are drawn in each window on Linux and Windows, where there is no system
  menu bar.
- Editor: Edit > Cut, Copy, and Paste; Undo, Redo, and Select All act on the source text when
  it has the keyboard.
- Editor: a focus ring shows when the drawing has the keyboard.
- Editor: a right-click menu on the drawing to set node shapes, edge lines, arrowheads, and
  lengths, reverse edges, add a node before or after, and connect to or from any node.
- `flowchart::edit::set_node_shape`, `set_edge_line`, and `reverse_edge`.
- Editor: a shape bar on the drawing that adds a node of a shape; double-click empty space to
  add a node, or drag from a node to empty space to add a linked one.
- Editor: layout hints: set the diagram's or a subgraph's direction, group and ungroup nodes,
  move nodes into or out of subgraphs (also by dragging onto one), keep two nodes together, and
  move statements earlier or later.
- `flowchart::edit::set_direction`, `keep_together`, `group`, `ungroup`, `move_to_subgraph`,
  `move_node`, and `move_edge`. Ungroup sends edges at the subgraph to its members, and moving
  a node out of a subgraph takes its chains with it.
- Editor: a disabled right-click item says why it is disabled, and a refused key shows the reason
  in the status bar.
- Editor: a styles panel (View > Show or Hide Styles) that applies classes to the selected
  nodes and edits a class's colors, width, and dashes.
- `flowchart::edit::set_class_style`, `apply_class`, `remove_class`, and `fresh_class_name`.
- Editor: the styles panel sets the selected edges' color, label color, width, and dashes with
  `linkStyle`; `flowchart::edit::set_link_style`.
- `sirene check FILE...` prints diagnostics only, for diagrams, directories, and Markdown files.
- `sirene svg FILE.md` and `sirene png FILE.md` write one image per mermaid block; `--link`
  adds an image link after each block, keeping the blocks.
- `sirene_core::markdown::mermaid_blocks` and `flowchart::Parse::other_diagram`.
- Editor: `sirene FILE.md` edits a Markdown file's mermaid blocks one at a time, writing each
  edit back into its block; `markdown::Block::replace_code`.
- The repo is a mind source (`mind meld jaemk/sirene`) offering the `sirene:render` skill, which
  teaches agents to check and render diagrams with the CLI.

### Changed

- `sirene png` writes at 4x by default (was 2x); `--quality normal` gives the old size.
- The source pane starts 480px wide and keeps its width when the window resizes.
- New windows open offset from the last one instead of on top of it.
- Other Mermaid diagram types (`sequenceDiagram`, `classDiagram`, ...) are one
  "not supported" error and draw nothing, instead of drawing their text as flowchart nodes.
- A mistyped subcommand (`sirene asci FILE`) is an error with a suggestion instead of opening
  windows.
- Save As and Export start in the home directory when the app runs from `/`.

### Fixed

- Readable label colors on styled fills; straighter edges between near-aligned nodes.
- `svg` and `png` refuse to overwrite their input.
- New windows fit the screen when the remembered size came from a larger one, and a remembered
  source pane leaves the drawing at least 240px.
- Settings are saved through a temporary file, so a crash cannot leave them half written.
- `svg DIR` and `png DIR` skip a diagram whose output another already writes (`a.mmd` beside
  `a.mermaid`) instead of overwriting it, and report an unreadable DIR or an `-o` that is a file
  once instead of per diagram.
- Saving keeps file permissions, owner, group, and symlinks; a read-only file is not saved.
- Reload asks before dropping unsaved edits; Save asks before replacing changes made on disk.
- Opening a file that is already open brings its window forward.
- Deleting a node drops the `id@{ ... }` and `class` statements of its edges.
- The label field covers the label it edits, with its text centered, instead of showing it
  through a translucent background in the dark theme.

## [0.1.1] - 2026-11-07

### Added

- Release binaries for macOS (aarch64, x86_64) and Linux (aarch64, x86_64; gnu
  with the GUI, musl without), with `SHA256SUMS` and build provenance
  attestations.
- `bin/install.sh`, a one-line installer for the release binaries.
- Homebrew: this repository is a tap, with a `sirene` formula for the binaries
  and a `sirene` cask for `Sirene.app`.

### Changed

- The macOS bundle identifier is `io.github.jaemk.sirene`, and the Mermaid
  document type is `io.github.jaemk.sirene.mermaid`.

## [0.1.0] - 2026-11-07

### Added

- Flowchart parsing, layout, and rendering to ASCII, SVG, and PNG
  (`sirene ascii`, `sirene svg`, `sirene png`).
- A WYSIWYG editor: the source beside a live drawing, with edits on the drawing
  written back to the source.
- `Sirene.app` for macOS and a Homebrew cask.
