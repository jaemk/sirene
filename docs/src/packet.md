# Packet diagrams

sirene draws Mermaid packet diagrams (`packet`) as images and as text, in the layout of RFC
header diagrams:

```
$ sirene ascii udp.mmd
                              UDP Packet

    0                   1                   2                   3
    0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
   ┌───────────────────────────────┬───────────────────────────────┐
0  │          Source Port          │       Destination Port        │
   ├───────────────────────────────┼───────────────────────────────┤
32 │            Length             │           Checksum            │
   ├───────────────────────────────┴───────────────────────────────┤
64 │                    Data (variable length)                     │
   └───────────────────────────────────────────────────────────────┘
```

```
packet
title UDP Packet
+16: "Source Port"
+16: "Destination Port"
32-47: "Length"
48-63: "Checksum"
64-95: "Data (variable length)"
```

In the editor a packet diagram is read-only on the drawing side.

## What is read

| Syntax | Drawn as |
|---|---|
| `0-15: "label"` | a field across bits 0 to 15 |
| `106: "URG"` | a one-bit field |
| `+8: "label"` | a field of 8 bits after the one before |
| `title text`, frontmatter `title` | the title above |
| frontmatter `config.packet.bitsPerRow` | bits per row, 32 by default |
| frontmatter `config.packet.showBits: false` | no bit numbers |
| frontmatter `config.packet.bitOrder: descending` | each row with its highest bit on the left |
| `packet-beta` | the same as `packet` |

A field that crosses the end of a row continues on the next. In text, a label too wide for its
box wraps, and a one-bit flag reads down its box.

## Errors

Each field must start at the bit after the one before. A field after a gap is an error but is
drawn, leaving the gap empty; a field that overlaps the one before is an error and left out. A
field that ends before it starts, `+0`, a field past bit 9999, and a label without quotes are
errors.

## Not drawn

The other `config.packet` keys (`bitWidth`, `rowHeight`, paddings) and theme variables are
ignored.
