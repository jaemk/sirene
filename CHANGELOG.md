# Changelog

All notable changes to `sirene` are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Graphviz DOT: `graph`, `digraph`, and `strict` sources, `.dot` and `.gv` files in
  directories, and `dot` or `graphviz` Markdown blocks draw with sirene's layout, including
  clusters, `node` and `edge` defaults, record labels as compartments, shapes, colors, arrows,
  and end labels. Attributes that are not drawn are warnings.
- `--lang mermaid|dot` reads every diagram as that language. Without it, a file's extension
  (`.dot`, `.gv`, `.mmd`, `.mermaid`) or a Markdown block's name picks the language in the CLI
  and the editor, and other sources are detected from their first line.
- Notes on composite states in state diagrams, and class diagram notes with no class
  (`note "text"`), which draw as a note box of their own.
- Mermaid themes: a diagram's `theme` and `themeVariables` (including `fontSize`) from its
  frontmatter `config:` or an `%%{init}%%` directive set its colors in SVG, PNG, and the editor,
  for all 11 Mermaid 12.3 themes; an unknown theme name is a warning. Pie slices and the
  sections of mindmaps, timelines, radar charts, and treemaps take the theme's `pie` and
  `cScale` colors.
- `sirene update` replaces the running binary with the latest release (`--check`, `--to
  VERSION`, `--yes`), checked against `SHA256SUMS` and its build provenance; it refuses for a
  Homebrew or `cargo install` binary and names the command to use instead.
- Mermaid state diagrams (`stateDiagram`, `stateDiagram-v2`) in every command and in the
  editor, where the drawing is read-only: states, transitions, `[*]`, composite states,
  concurrent regions, fork, join, and choice, notes, `direction`, and styling.
- Mermaid class diagrams (`classDiagram`, `classDiagram-v2`), read-only in the editor: classes
  with attribute and method compartments, annotations, generics, every relation with its
  markers, cardinalities, and label, namespaces, notes, and styling.
- Mermaid ER diagrams (`erDiagram`), read-only in the editor: entities with attribute columns
  (type, name, keys, comment), aliases, relationships with crow's-foot markers in symbol or word
  form, and styling.
- Mermaid git graphs (`gitGraph`), read-only in the editor: commits, branches, checkout, merges,
  and cherry-picks on lanes in each direction (`LR`, `TB`, `BT`), with commit types and tags; in
  text, one commit per row as `git log --graph` draws them.
- Mermaid timelines (`timeline`, `LR` and `TD`), read-only in the editor: periods with events,
  continuation lines, sections with their own colors, `<br>`, and wrapping, in images and text.
- Mermaid sequence diagrams (`sequenceDiagram`), read-only in the editor: participants, actors,
  and the typed shapes, every message arrow, self-messages, activations, notes, `loop`, `alt`,
  `opt`, `par`, `critical`, `break`, and `rect` blocks, `box` groups, `create` and `destroy`,
  and `autonumber`, in images and text.
- Mermaid pie charts (`pie`), read-only in the editor: slices with percentages, a legend,
  `showData`, and a title; in text, a table with a bar per slice.
- Mermaid gantt charts (`gantt`), read-only in the editor: tasks with `after`, `until`,
  durations, and tags (`active`, `done`, `crit`, `milestone`, `vert`), sections, `excludes`,
  `includes`, `weekend`, `inclusiveEndDates`, `todayMarker`, `topAxis`, compact rows, and any
  dayjs `dateFormat` and d3 `axisFormat`, also settable in frontmatter or a directive.
- Mermaid mindmaps (`mindmap`), read-only in the editor: a tree built from indentation, every
  node shape, markdown strings, and `:::` classes, drawn out to both sides of the root with a
  color per branch; in text, an outline of the tree.
- Mermaid user journeys (`journey`), read-only in the editor: tasks in sections with a score
  from 1 to 5 drawn as a face, and actors as colored dots with a legend; in text, a score bar per
  task.
- Mermaid quadrant charts (`quadrantChart`), read-only in the editor: axis and quadrant labels,
  and points styled directly or by `classDef` (`radius`, `color`, `stroke-color`,
  `stroke-width`); in text, the points plotted by name in a box split in four.
- Mermaid XY charts (`xychart-beta`, `xychart`), read-only in the editor: bar and line series
  over category or numeric x-axes, vertical or `horizontal`, with axis titles and ranges, point
  labels, and a legend for named series; in text, a row per position with bars and markers.
- Mermaid requirement diagrams (`requirementDiagram`), read-only in the editor: every
  requirement type and elements as boxes with their fields, the seven relationship types in
  either direction, classes, styles, and `direction`.
- Mermaid kanban boards (`kanban`), read-only in the editor: columns of cards with `assigned`,
  `ticket`, and `priority` metadata, drawn with a priority stripe; in text, a box per column.
- Mindmaps read `@{ ... }` node metadata and warn that it is not used.
- Mermaid packet diagrams (`packet`, `packet-beta`), read-only in the editor: bit ranges, single
  bits, and `+count` fields in rows of `bitsPerRow`, with bit numbers and `bitOrder: descending`;
  in text, the RFC header layout.
- Mermaid radar charts (`radar-beta`), read-only in the editor: axes, curves with listed or
  keyed values, `min`, `max`, `ticks`, circle or polygon `graticule`, a legend, and the
  frontmatter `radar` config; in text, a bar per axis and curve.
- Mermaid sankey diagrams (`sankey-beta`, `sankey`), read-only in the editor: CSV rows of flows
  laid out as d3-sankey does, with `linkColor`, `nodeAlignment`, `showValues`, `prefix`, and
  `suffix`; in text, each node's outgoing flows with bars.
- Mermaid treemaps (`treemap-beta`, `treemap`), read-only in the editor: sections and leaves
  nested by indentation, laid out as d3's squarified treemap, with `classDef` styles and d3
  `valueFormat`; in text, an outline with a bar per item.
- Mermaid block diagrams (`block-beta`, `block`), read-only in the editor: blocks in a grid of
  `columns` with widths and `space`, nested `block` groups, block arrows, edges, and styles,
  placed as Mermaid places them; in text, the grid with the edges listed below.
- Mermaid architecture diagrams (`architecture-beta`), read-only in the editor: services with
  their icons (`cloud`, `database`, `disk`, `internet`, `server`), junctions, nested groups,
  edges by side with arrows, labels, and `{group}` ends, and `align`, placed on a grid from
  the edge sides; in text, boxes with the icon name.
- Mermaid C4 diagrams (`C4Context`, `C4Container`, `C4Component`, `C4Dynamic`,
  `C4Deployment`), read-only in the editor: people, systems, containers, and components with
  `Db`, `Queue`, and `_Ext` variants, nested boundaries and deployment nodes, relations with
  technology, `UpdateElementStyle`, `UpdateRelStyle`, and `UpdateLayoutConfig`, in Mermaid's
  colors and row layout; in text, the boxes with the relations listed below.
- Mermaid ZenUML diagrams (`zenuml`), drawn as sequence diagrams and read-only in the editor:
  annotated and aliased participants, async messages, sync calls with bodies, replies from
  assignments and `return`, creations with `new`, `@Starter`, and `if`/`else`, loops, `opt`,
  `par`, and `try`/`catch`/`finally` fragments; calls with no caller come in from the left.
- Mermaid info diagrams (`info`, `showInfo`), drawn as sirene's version.
- Mermaid ishikawa diagrams (`ishikawa-beta`), read-only in the editor: the effect in a head
  at the end of a spine, causes on bones above and below, and sub-causes nested by
  indentation; in text, an outline.
- Mermaid venn diagrams (`venn-beta`), read-only in the editor: sets, unions, sizes, labels,
  `text` nodes, and `style`, laid out as venn.js does for Mermaid; in text, each set and
  overlap with its text nodes.
- Mermaid Wardley maps (`wardley-beta`), read-only in the editor: anchors and components
  placed by visibility and evolution, custom `evolution` stages, `size`, links with flows and
  labels, `evolve` trends, `pipeline` blocks, `(build)`, `(buy)`, `(outsource)`, `(market)`,
  and `(inertia)`, notes, annotations, and accelerators; in text, the components plotted over
  the stages with the rest listed below.
- Mermaid tree views (`treeView-beta`), read-only in the editor: files and directories nested
  by indentation or box-drawing characters, `:::highlight`, `## descriptions`, the built-in
  `file` and `folder` icons, and the `treeView` frontmatter config; in text, the tree as `tree`
  prints it.
- Mermaid use case diagrams (`usecase-beta`), read-only in the editor: actors (stick, hollow,
  and awesome figures), ellipse and rectangle use cases, business marks, stereotypes, system
  boundaries (`rect` and `package`), associations with every marker, `include`, `extend`,
  generalization, edge ids and animation, notes, JSON tables, classes, and styles.
- Mermaid railroad diagrams in all four notations (`railroad-ebnf-beta`, `railroad-abnf-beta`,
  `railroad-peg-beta`, and the `railroad-beta` constructors), read-only in the editor:
  terminals, non-terminals, special sequences, sequences, choices, optionals, repetition,
  ABNF repeats and `%x` numbers, EBNF exceptions, and PEG predicates; in text, boxed
  elements on a track with branches, bypasses, and loops.
- Mermaid swimlane diagrams (`swimlane-beta`), read-only in the editor: flowcharts whose
  top-level subgraphs are lanes side by side across the flow, with an untitled lane for
  nodes outside every lane.
- Mermaid Cynefin diagrams (`cynefin-beta`), read-only in the editor: items in the five
  domains, transitions between them, and `showDomainDescriptions`; in text, the quadrants in a
  box with confusion in the middle and the transitions listed below.
- Mermaid event modeling diagrams (`eventmodeling`), read-only in the editor: time and reset
  frames in swimlanes by type and namespace, inferred and `->>` relations, inline data and
  data blocks; in text, the lanes as rows of boxes with vertical arrows between them.
- Collapsed flowchart subgraphs (`id@{ view: collapsed }`) are drawn as one node with the
  subgraph's title, a dashed rule, and three dots; edges into the subgraph end at that node.
- Mermaid agentflow diagrams (`agentflow-beta`), read-only in the editor: `flow` containers
  (collapsible with `@{ view: collapsed }`), `global` blocks, `connector` nodes, agentflow's
  shape aliases (`task`, `tool`, `input`, `decision`, `refdoc`, `action`), and `-->`, `--x`, and
  `-.-` edges; removed shapes are errors and other Mermaid shapes warnings, both drawn rounded.
- Markdown blocks are no longer skipped as another diagram type: every Mermaid 12.3 type is
  drawn.
- Labels decode HTML entities such as `&nbsp;` as well as Mermaid's `#nbsp;`.
- `THIRD-PARTY-NOTICES` with the licenses of the code sirene ports (venn.js, fmin, d3-sankey,
  and other d3 modules, Day.js, Mermaid) and the Noto Sans fonts. Release tarballs, the macOS
  app, and the published crates now include it along with `LICENSE-MIT` and `LICENSE-APACHE`.

### Changed

- Every flowchart shape has its own outline, as Mermaid 12.3 draws it: documents, paper tape,
  delay, side and lined cylinders, display, divided and window panes, loop limit, stacked,
  stored-data, tagged and lined rectangles, hourglass, bolt, braces, datastore, folder, bucket,
  console, browser, person, bang, and cloud were drawn as rectangles before. Hourglasses and
  bolts show no label.
- In ASCII, start and stop circles (`sm-circ`, `f-circ`, `fr-circ`) are one character, a `fork`
  is a heavy bar, and an unlabeled diamond is `◇`.

### Fixed

- `brew install --cask sirene` no longer fails with "calling `depends_on macos: :catalina` is
  disabled"; the cask drops its `depends_on macos` line.
- The labels at an edge's ends (class and ER cardinalities, DOT `headlabel` and `taillabel`) no
  longer run past the edge of an image: the drawing grows to hold them.
- A `stroke-width` over 32px is drawn 32px wide; a wider line and its arrowheads ran off the
  image.
- A blank diagram title no longer prints an empty line in text output.
- Deeply nested sources no longer overflow the stack: blocks, indented levels, and JSON values
  are read 64 deep (block diagrams 12), and a deeper one is one error.
- A gantt chart whose `excludes` names every day no longer hangs; it is an error and the
  `excludes` is ignored. Long `after` chains are dated in one visit per task, and a task waiting
  on a task with a bad date is left out without a second error.
- Text output of a very large drawing is cut off at 2^22 cells with a line saying so, in place
  of an overflow or an allocation failure.
- Architecture diagrams with many groups lay out in time proportional to their size.
- A file starting with a byte order mark draws as it does without one; before, its header or
  frontmatter was not found.

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
