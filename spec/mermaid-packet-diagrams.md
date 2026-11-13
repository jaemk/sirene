# Mermaid Packet Diagrams

`packet` and `packet-beta`, lowered to their own model (LANGS-2, MCHART-1). Reference:
<https://mermaid.js.org/syntax/packet.html>. Code: `crates/sirene-core/src/packet/`;
`diagram::lower` picks it by the header keyword. Tests: `crates/sirene-core/tests/packet.rs` and
the corpus under `crates/sirene-core/tests/fixtures/packet/` (LANGS-6).

## MPKT-1 Syntax

- Header: `packet` or `packet-beta`.
- A field: `START: "label"` (one bit), `START-END: "label"`, or `+BITS: "label"` (starting after
  the field before). The label is in double or single quotes, where `\` escapes the next
  character.
- `title TEXT` to the end of the line or a `%%` comment.
- Comments, directives, frontmatter `title` and `config.packet` (`bitsPerRow`, `showBits`,
  `bitOrder`), `accTitle`, `accDescr`.

The parser shares `crate::lines` with the other Mermaid parsers and is lossless. A field missing
its bits, `:`, or quoted label is an error. Tests: `snapshots`, `syntax_errors`, and the parser's
unit tests.

## MPKT-2 Model

`packet::Packet`: the title (the `title` statement, else frontmatter `title`), the accessibility
title and description, `bits_per_row` (32 unless set, 1 to 256), `show_bits` (on unless
`false`), `descending` (`bitOrder: descending`), and the fields in order, each a `Field` of a first
and last bit and a label. A bad config value is a warning and ignored.

Each field must start at the bit after the last one, as in Mermaid; one that does not is an
error. A field after a gap is still drawn, leaving the gap empty; one that overlaps the field
before is left out. A field ending before it starts, one of `+0` bits, and one past bit 9999
(`packet::MAX_BIT`) are errors and left out. `Packet::rows` splits the fields into rows of
`bits_per_row`, a field crossing a row's end continuing on the next row, and `Packet::column`
gives a bit's place in its row, mirrored when descending. Tests: `bits`, `errors`, and the
module's unit tests.

## MPKT-3 Drawing

Each row is `bits_per_row` bits of 32 pixels by 32, 5 pixels apart. Each field's part in a row is
a box in the node colors across its bits, with its label wrapped inside. With `show_bits`, the
first and last bit numbers sit over their bits above the box. The title is centered above. Every
element is owned by the diagram. Tests: `drawing`.

## MPKT-4 ASCII

The RFC layout: two columns per bit, a ruler of bit positions on two lines (tens over units,
mirrored when descending) when `show_bits` is on, each row's first bit number at its left, and
each field's part a box across its bits. A label wraps at spaces to fit the box, and a word wider
than the box breaks anywhere, so a one-bit flag reads down the box (`U`, `R`, `G`). A row is as
tall as its tallest label, and each label is centered in its box. The title is centered above.
Tests: `snapshots`, `ascii_charset_snapshots`, and the renderer's unit tests.

## MPKT-5 Not drawn

Theme variables and the other `config.packet` keys (`bitWidth`, `rowHeight`, paddings) are
ignored. Drawing edits are read-only (LANGS-5).
