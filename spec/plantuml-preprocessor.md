# PlantUML Preprocessor

Evaluation of PlantUML's `!` preprocessor lines before a document is parsed (PUML-4), in
`sirene_core::plantuml`. Reference: <https://plantuml.com/preprocessing>. Today every such line
is a warning and is skipped, and `!include` and its variants are errors.

## PPRE-1 Scope

Evaluated: `!define NAME value` and `!define NAME(args) body`, `!definelong` ... `!enddefinelong`,
`$variable = value`, `!if`, `!ifdef`, `!ifndef`, `!elseif`, `!else`, `!endif`, `!function` and
`!procedure` with `!return` and `!endfunction`, `!while` ... `!endwhile`, and `%` builtins that
need no environment (`%strlen`, `%substr`, `%upper`, `%lower`, `%string`, `%intval`, `%not`,
`%true`, `%false`, `%newline`). Not evaluated, and still an error naming the directive: `!include`
forms that read a file, the network, or a standard library (`<C4/C4_Container>`), `!import`,
`!exec`, and `%getenv`, `%load_json`, and the other builtins that read the environment. An
`!includedef ID` and `!include !ID` of a `@startdef ... @enddef` block in the same file is
evaluated. A `@startdef` document draws nothing and has no diagnostics of its own.

## PPRE-2 Positions

The parser reads the expanded text. Every diagnostic's range is in the source the user wrote: a
problem on an expanded line is reported at the directive or call that produced it, and a line
that came from the source keeps its own range. The syntax tree stays lossless over the source.

## PPRE-3 Limits

Expansion is bounded as LANGS-8 bounds other expansion: calls nest at most 64 deep, `!while` runs
at most 10,000 iterations in total, and the expanded text is at most 16 MiB. A source past a
limit reports one error at the call that crossed it and keeps what it had expanded. The fuzz
targets cover the preprocessor (LANGS-11).

## PPRE-4 Open questions

- Real diagrams often `!include` a standard library (C4-PlantUML, AWS, Azure icons). Evaluating
  `!define` does not draw them, and bundling the libraries is a separate decision.
- Whether to keep the line-based tree (expanded lines mapped back) or add a source map to the
  parser's input type.
