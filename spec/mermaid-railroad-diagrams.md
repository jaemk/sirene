# Mermaid Railroad Diagrams

`railroad-beta`, `railroad-ebnf-beta`, `railroad-abnf-beta`, and `railroad-peg-beta`, a grammar
drawn as one syntax diagram per rule (LANGS-2, LANGS-3, MCHART-1). Reference: Mermaid 12.3.0's
`docs/syntax/railroad.md` and `railroadRenderer.ts`. Code: `crates/sirene-core/src/railroad/`;
`diagram::lower` picks it by the header keyword, which also picks the notation. Tests:
`crates/sirene-core/tests/railroad.rs` and the corpus under
`crates/sirene-core/tests/fixtures/railroad/` (LANGS-6).

## MRAIL-1 Syntax

- Header: one of the four keywords. A rule is `name <op> definition ;` and may span lines; the
  operator is `=` (EBNF also `::=`, PEG `<-`).
- `title text`, `accTitle: text`, `accDescr: text`, and `accDescr { ... }` are statements of their
  own. `%%` comments and directives are read in every notation.
- EBNF: strings in either quote with `\` escapes, names, `,` or juxtaposition for sequence, `|`,
  `[ ]`, `{ }`, `( )`, postfix `?`, `*`, `+`, `A - B`, `? special ?`, and `(* *)` and `/* */`
  comments. A `?` directly after an element is the optional operator; after whitespace it starts
  a special sequence if one closes.
- ABNF: `"..."` strings without escapes, names of `[A-Za-z][A-Za-z0-9-]*`, `/`, `[ ]`, `( )`,
  `*`, `n*`, `*m`, `n*m`, and `n` repeat prefixes, and `%x`, `%d`, `%b` numbers with ranges and
  `.` concatenation. A `;` with text after it on its line, or where a rule may start, begins a
  comment; otherwise it ends the rule.
- PEG: strings in either quote, names, `/`, `( )`, one postfix `?`, `*`, or `+`, `&` and `!`
  predicates, `.`, and `#` comments.
- `railroad-beta`: a rule's definition is one constructor call, `terminal`, `nonterminal`,
  `special` (a string argument) or `sequence`, `choice`, `optional`, `zeroOrMore`, `oneOrMore`
  (expression arguments, comma separated), with `/* */` comments.
- A syntax error reports once and skips to the next `;`.

The parser is lossless. Tests: `snapshots` and the parser's unit tests.

## MRAIL-2 Model

A grammar is a list of rules, each an expression of terminals, non-terminals, special
sequences, sequences, choices, optionals, and repeats with a minimum and an optional maximum.
Single-item sequences and choices collapse. Strings are unescaped, except in ABNF. EBNF `A - B`
is the sequence `A`, terminal `-`, `B`. PEG `&X` and `!X` are specials labelled `&X` or `!X` (a
string as `"v"`, a group as `(...)`), and `.` a special `.`. ABNF `0*1` is an optional. A
repeated rule name is a warning, and both rules are kept. Tests: `notations`, `problems`, and the
model's unit tests.

## MRAIL-3 Drawing

Mermaid's railroad layout: rules stacked top to bottom, each a name, a start marker, the track,
and an end marker. Terminals are rounded boxes in the cluster colors, non-terminals boxes in the
node colors, and specials dashed boxes. A sequence runs left to right; a choice centers its
branches; an optional bypasses over the top; a repeat loops back below. An empty diagram is 200
by 100. Tests: `drawing` and the scene's unit tests.

## MRAIL-4 ASCII

Each rule is `name = ●─...─●` with its track, rules separated by a blank line. Terminals are
rounded boxes, non-terminals boxes, and specials dotted boxes. A choice's branches hang below the
first with rails on both sides; an optional has a bypass above; a repeat has a loop below marked
`◄`. The ASCII charset draws `*` for the markers and `<` on loops. Tests: `snapshots` and the
renderer's unit tests.

## MRAIL-5 Not drawn

The title. Repeat bounds beyond zero or one, zero or more, and one or more (drawn as a loop).
Mermaid's `railroad` config and themes. In ASCII a choice is top-aligned rather than centered.
Drawing edits are read-only (LANGS-5).
