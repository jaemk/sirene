# Mermaid Tree Views

`treeView-beta`: a directory-like hierarchy of files and folders. Reference:
<https://mermaid.js.org/syntax/treeView.html> and Mermaid 12.3.0's `diagrams/treeView`
(`boxDrawingPreprocessor.ts`, `db.ts`, `renderer.ts`, `styles.ts`) with its Langium grammar.
Code: `crates/sirene-core/src/treeview/`; `diagram::lower` picks it by the header keyword.
Tests: `crates/sirene-core/tests/treeview.rs` and the corpus under
`crates/sirene-core/tests/fixtures/treeview/` (LANGS-6).

## MTREEV-1 Syntax

- Header: `treeView-beta`.
- One entry per line: a bare name (any text up to an annotation, internal spaces kept) or a
  `"quoted"` or `'quoted'` name, then in any order `:::class`, `icon(name)` (`pack:name`, or
  empty), and `## description`. A bare name stops at ` :::`, ` icon(`, or ` ##`.
- Box-drawing input: when a line other than a comment, metadata, or one holding only `│` has a
  box-drawing character (`─ ━ │ ┃ └ ┗ ├ ┣`), each line's prefix up to its branch character,
  dashes, and spaces is structure. Lines of decoration alone are skipped.
- `title text`, `accTitle`, `accDescr`, Mermaid comments and directives, and frontmatter.

The parser shares `crate::lines` with the other Mermaid parsers and is lossless. Tests: the
parser's unit tests.

## MTREEV-2 Model

`treeview::parse(src).lower()` gives a `treeview::TreeView`: the title, the `treeView`
frontmatter config (`rowIndent`, `paddingX`, `paddingY`, `lineThickness`, `showIcons`; each
size capped at 1000), and the entries. A name ending in `/` is a directory, without the `/`. As in Mermaid's db, each line's
level is its indentation in characters, or in box-drawing input four times one more than its
branch column divided by the segment width (the first branch column above 0, else 4), rounded;
a line is a child of the nearest line above it with a lower level. `icon()` is `none`; when
annotations repeat, the last wins. Tests: `model` and the unit tests in `treeview/mod.rs`.

## MTREEV-3 Problems

An unclosed quote, a branch with no name, or text after a name that is not an annotation is an
error. In box-drawing input, an indented line with no branch is an error. Tests: `problems` and
the parser's unit tests.

## MTREEV-4 Drawing

As Mermaid's renderer draws it: a root `/` and then a row per entry, depth first; each row is
the label's height plus `paddingY` above and below, indented `depth * (rowIndent + paddingX)`;
labels at 16px in the text color, directories in bold; a horizontal line from the parent's
line to each row and a vertical line under each parent down to its last child's middle, of
`lineThickness`; descriptions in italic `#6a9955` at a common column 16 right of the widest
label; `:::highlight` rows shaded `#ffc107` at 15% with a `#ffc107` border to the right edge. An
icon (an explicit `icon()`, or with `showIcons` the built-in `folder` or `file`) is 14px before
the label. The title is not drawn, as in Mermaid. Tests: `drawing`, `snapshots`, and the unit
tests in `scene/treeview.rs`.

## MTREEV-5 Text

The root `/` and the tree with `├── `, `└── `, and `│   ` (`|-- `, `` `-- ``, `|   ` in
ASCII), directories with their `/`, and descriptions lined up two columns right of the longest
row. Tests: `snapshots`.

## MTREEV-6 Not drawn

Icons from icon packs are a `?`, as Mermaid draws an unregistered icon. `defaultIconPack`,
`filenameIcons`, `extensionIcons`, the theme variables, and classes other than `highlight` are
not read. Drawing edits are read-only (LANGS-5).
