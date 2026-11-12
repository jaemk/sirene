# PlantUML Network Diagrams

PlantUML `nwdiag { ... }` documents (`@startnwdiag`, or `@startuml` whose first statement is
`nwdiag {`) (PUML-2), lowered to the graph model (`sirene_core::plantuml`). Reference:
<https://plantuml.com/nwdiag>. Tests: unit tests in `crates/sirene-core/src/plantuml/nwdiag.rs`,
and the fixtures `nwdiag.puml` and `errors/nwdiag.puml` with
`crates/sirene-core/tests/plantuml.rs`.

## PNW-1 Statements

Statements end at a line end or `;`, and `{` and `}` may share a line with them. The
`nwdiag { }` wrapper is optional. `title` and the shared settings apply (PSEQ-6).

## PNW-2 Networks

`network NAME { ... }` is a box with a heavy outline, labeled with its name (or its
`description`) over its `address`; `color` fills it. Each host inside it is linked to it,
the link labeled with the host's `address`. A host's first network is drawn above it and its
later networks below it, so hosts on two networks sit between them.

## PNW-3 Hosts, groups, and links

- `NAME [key = value, ...]` declares a host or sets its attributes: `shape` (the element
  shapes of PDSC-2), `description` (its label), `color`, and `address` (in a network).
- `group NAME { a; b, c }` draws a cluster around its hosts; `color` fills it and
  `description` titles it.
- `a -- b -- c` links hosts directly.

## PNW-4 Diagnostics

Unknown attributes and shapes, and an `address` outside a network, are warnings. An unclosed
`{` or `[`, a `}` that closes nothing, a block other than `nwdiag`, `network`, or `group`, and
an unknown statement are errors.
