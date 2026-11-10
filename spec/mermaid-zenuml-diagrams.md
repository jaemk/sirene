# Mermaid ZenUML Diagrams

`zenuml`, lowered to the sequence diagram model (MSEQ-2, LANGS-2). Reference:
<https://mermaid.js.org/syntax/zenuml.html>. Code: `crates/sirene-core/src/zenuml/`;
`diagram::lower` picks it by the header keyword. Tests: `crates/sirene-core/tests/zenuml.rs` and
the corpus under `crates/sirene-core/tests/fixtures/zenuml/` (LANGS-6).

## MZEN-1 Syntax

- Header: `zenuml`.
- Participants: `A`, `"A B"`, `A as Label`, an annotator such as `@Actor A`, and `@Starter(A)`.
- Async messages `A->B: text`, after an optional `@return` or `@reply` line.
- Sync calls `[Type] [x =] [C->]A.method(args)` and creations `[Type] [x =] new A(args)`, each with
  an optional `{` ... `}` body on the following lines; `return text`.
- Fragments: `if(c) {` with `} else if(c) {` and `} else {`; `while`, `for`, `forEach`,
  `foreach`, and `loop` with an optional condition; `opt {`; `par {`; `try {` with
  `} catch(e) {` and `} finally {`. A branch's `}` and the next keyword share a line.
- `title`, `// comment` lines, Mermaid comments and directives, and frontmatter `title`.

The parser shares `crate::lines` with the other Mermaid parsers and is lossless. An unclosed
`(`, a `.` without a method name, `new` without a participant, a fragment without `{`, an
unclosed or stray brace, extra text after a statement, and a line that is none of the above are
errors. Tests: `syntax_errors` and the parser's unit tests.

## MZEN-2 Model

`zenuml::parse(src).lower()` gives a `sequence::Sequence`:

- Participants in order of mention; an annotator that `ParticipantKind::from_type` knows sets the
  kind, any other is a warning; `as` sets the label.
- A sync call is a solid message with a filled head from the caller to its target that activates
  the target; its body is lowered with the target as the caller; it ends with a dotted reply
  named by the assignee (`Type x` replies `x`) that deactivates the target, or with a deactivation
  when there is no assignee or the body returned. `C->A.m()` names the caller.
- A call with no caller comes from the `@Starter` participant, or else from a hidden participant
  (`ParticipantKind::Hidden`) first in the list, dropped when nothing uses it.
- `new A()` adds `Event::Create` when `A` has not been in a message yet, then a `<<create>>` call.
- `return x` is a dotted reply from the current call's target to its caller; outside a call it is
  a warning and ignored.
- An async message has an open head; after `@return` it is dotted. A `@return` that is not
  followed by an async message is a warning.
- `if` is `alt`, the loops are `loop`, `opt`, `par`, and `try` (`BlockKind::Try`) are blocks, with
  the condition as the label; `else if(c)`, `else`, `catch(e)`, and `finally` are dividers labeled
  `c`, empty, `catch e`, and `finally`.

A statement with a syntax error on its own line is not lowered. Tests: `model`, `starters`,
`fragments`, `errors`.

## MZEN-3 Drawing

Drawn and written as text as a sequence diagram (MSEQ-3, MSEQ-4). The hidden starter has no
header or lifeline, so its messages run from the diagram's left edge. Tests: `drawing`,
`snapshots`, `ascii_charset_snapshots`.

## MZEN-4 Not drawn

Comments are not drawn above messages; participant groups, stereotypes, colors, widths, and
dividers (`==text==`) are not read; icon annotators such as `@EC2` are boxes. Drawing edits are
read-only (LANGS-5).
