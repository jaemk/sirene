# Mermaid Ishikawa Diagrams

`ishikawa-beta` (and `ishikawa`), fishbone diagrams: an effect and the causes that lead to it.
Reference: <https://mermaid.js.org/syntax/ishikawa.html> and Mermaid 12.3.0's
`diagrams/ishikawa`. Code: `crates/sirene-core/src/ishikawa/`; `diagram::lower` picks it by the
header keyword. Tests: `crates/sirene-core/tests/ishikawa.rs` and the corpus under
`crates/sirene-core/tests/fixtures/ishikawa/` (LANGS-6).

## MISH-1 Syntax

- Header: `ishikawa-beta` or `ishikawa`; the first line of text may follow on the same line.
- Every other line is one label, its indentation giving its depth. `<br>` breaks a label, and
  HTML entities are decoded.
- Mermaid comments and directives, and frontmatter `title`.

There are no keywords: `title x` is a cause. The parser shares `crate::lines` with the other
Mermaid parsers and is lossless; it reports no errors past the header. Tests: the parser's unit
tests.

## MISH-2 Model

`ishikawa::parse(src).lower()` gives an `ishikawa::Ishikawa`: the frontmatter title and the
effect, a `Cause` tree. As in Mermaid, the first line is the effect whatever its indentation; the
first cause's indentation is level 1, a line at or left of it is a cause of the effect, and each
other line is a cause of the nearest line above it with less indentation. A diagram with no lines
has no effect. Tests: `model`, and `trees` in `ishikawa/mod.rs`.

## MISH-3 Drawing

Mermaid's fishbone layout: the effect, bold and wrapped at 13 characters, in a head at the right
end of a horizontal spine; causes alternate above and below, in boxed labels at the ends of
bones angled 82 degrees back from the spine, each pair left of the one before. Sub-causes sit on
bones alternating flat and angled, spaced evenly along their cause's bone, with labels wrapped at
15 characters to the left of their bone ends. The spine's length above and below follows how many
sub-causes each side has. Every bone has an arrowhead where it meets its parent.

Unlike Mermaid, a cause with no sub-causes starts its short bone a label's width left of where its
pair starts, so its box clears the head and the pair to its right; the frontmatter title is drawn
above. Tests: `drawing`, `snapshots`.

## MISH-4 Text

The effect, then an outline of the causes as in mindmaps (MMIND-4); the title centered above.
Tests: `snapshots`.

## MISH-5 Not drawn

The hand-drawn look and `ishikawa` config (`diagramPadding`, `useMaxWidth`) are not read. Drawing
edits are read-only (LANGS-5).
