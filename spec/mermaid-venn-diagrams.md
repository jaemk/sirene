# Mermaid Venn Diagrams

`venn-beta`: sets drawn as circles, sized and overlapped by the sizes given. Reference:
<https://mermaid.js.org/syntax/venn.html>, Mermaid 12.3.0's `diagrams/venn`, and venn.js
(`@upsetjs/venn.js`), which lays it out. Code: `crates/sirene-core/src/venn/`; `diagram::lower`
picks it by the header keyword. Tests: `crates/sirene-core/tests/venn.rs` and the corpus under
`crates/sirene-core/tests/fixtures/venn/` (LANGS-6).

## MVENN-1 Syntax

- Header: `venn-beta`. Keywords are case-insensitive.
- `title text`, up to a `#` or `;`; surrounding quotes are dropped.
- `set id`, `set id["Label"]`, `set id[Label]`, each with an optional `:size`.
- `union a,b,...` with the same label and size suffixes.
- `text sets id`, `text sets id["Label"]`: a text node in the region of `sets`. A `text` line
  indented under a `set` or `union` names only its id and takes that statement's sets; a line at
  column 0 ends this, as in Mermaid.
- `style ids name:value, ...` for `fill`, `color`, `stroke`, `stroke-width`, and `fill-opacity`.
- Ids are words (`A`, `Set_1`, `a-b`) or `"quoted strings"`; text ids may also be numbers.
- Mermaid comments and directives, and frontmatter `title`.

The parser shares `crate::lines` with the other Mermaid parsers and is lossless. Tests: the
parser's unit tests.

## MVENN-2 Model

`venn::parse(src).lower()` gives a `venn::Venn`: the title (the `title` statement over
frontmatter), the subsets with sorted set names, sizes, and labels, the text nodes, and the
styles. A size defaults to 10 divided by the square of the number of sets. A union needs two or
more sets, each declared by an earlier `set`; otherwise it is an error and left out. Tests:
`model`, `unknown_sets`.

## MVENN-3 Layout

venn.js's layout, ported: each set is a circle with its size as area; circles are placed greedily,
most overlapped first, at candidate points where their pairwise overlaps match, then refined with
Nelder-Mead against the overlap sizes, rotated so the second largest circle is right of the
largest, mirrored so the third is below, disjoint groups laid out in a grid, and scaled into
800 by 450 with 15 padding. A region's label goes at the point inside its circles and outside the
others farthest from their edges. As in Mermaid, a union of three or more sets adds each of its
pairs that is not declared, sized a quarter of the smaller set, and sets of size 0 are left out.

Unlike venn.js, the constrained-MDS start it tries (with random restarts) for eight or more areas
is not used, so the layout is the same on every run. Tests: the unit tests in
`venn/layout.rs`, `drawing`.

## MVENN-4 Drawing

As Mermaid draws it at its default 800 by 450: the title at the top; each set's circle in the
default theme's `venn1` to `venn8` colors at 10% fill opacity with a stroke of 2.5, and its label
(the set's label or id) in the color darkened by 30% lightness, or lightened on a dark background;
an overlap's label in the text color; labels at 24px, wrapped as venn.js wraps them. A styled
overlap is filled with its `fill`. Text nodes are 20px in a grid inside their region, below its
label, wrapped to their cells; `style id color:...` colors one. Tests: `drawing`, `snapshots`.

## MVENN-5 Text

The title centered above, then each set (by its label) and overlap (`A & B: label`) on a line,
with its text nodes under it as `  - text`. Tests: `snapshots`.

## MVENN-6 Not drawn

The hand-drawn look, Mermaid 12's `neo` look and `redux-color` theme, and `venn` config are not
read. Drawing edits are read-only (LANGS-5).
