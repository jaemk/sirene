# Railroad diagrams

sirene draws Mermaid's railroad diagrams as images and as text: one syntax diagram per grammar
rule, written in EBNF (`railroad-ebnf-beta`), ABNF (`railroad-abnf-beta`), PEG
(`railroad-peg-beta`), or Mermaid's constructor notation (`railroad-beta`).

```
$ sirene ascii identifiers.mmd
                                             ┌────────────┐
               ┌┄┄┄┄┄┄┄┄┄┄┄┄┄┄┐  ┌────────┐  │ ┌────────┐ │
Identifier = ●─┤ ? !Keyword ? ├──┤ Letter ├──┼─┤ Letter ├─┼─●
               └┄┄┄┄┄┄┄┄┄┄┄┄┄┄┘  └────────┘  │ └────────┘ │
                                             └──────◄─────┘

              ╭────╮
Keyword = ●─┬─┤ if ├────┬─●
            │ ╰────╯    │
            │ ╭──────╮  │
            ├─┤ else ├──┤
            │ ╰──────╯  │
            │ ╭───────╮ │
            └─┤ while ├─┘
              ╰───────╯

        ┌┄┄┄┄┄┄┄┄┄┄┄┄┄┐  ┌┄┄┄┄┄┄┄┐
Any = ●─┤ ? &Letter ? ├──┤ ? . ? ├─●
        └┄┄┄┄┄┄┄┄┄┄┄┄┄┘  └┄┄┄┄┄┄┄┘
```

```
railroad-peg-beta
# keywords excluded
Identifier <- !Keyword Letter Letter* ;
Keyword <- "if" / "else" / "while" ;
Any <- &Letter . ;
```

Terminals are rounded boxes, non-terminals square boxes, and special sequences dotted boxes.
Choices branch, optionals get a bypass over the top, and repetition a loop below. See
`crates/sirene-core/tests/fixtures/railroad/valid/` for more. In the editor a railroad diagram is
read-only on the drawing side.

## What is read

| Notation | Syntax |
|---|---|
| all | `name = ... ;` rules over any number of lines, `title`, `accTitle`, `accDescr`, `%% comment` |
| EBNF | `=` or `::=`, `"t"`, `'t'`, names, `,` or juxtaposition, `\|`, `[ ]`, `{ }`, `( )`, `?`, `*`, `+`, `A - B`, `? special ?`, `(* *)` and `/* */` comments |
| ABNF | `=`, `"t"`, names, `/`, `[ ]`, `( )`, `*`, `1*`, `2*4`, `3` repeats, `%x41`, `%x30-39`, `%x0D.0A` numbers, `;` comments |
| PEG | `<-`, `"t"`, `'t'`, names, `/`, `( )`, `?`, `*`, `+`, `&` and `!` predicates, `.`, `#` comments |
| constructors | `=`, `terminal("t")`, `nonterminal("n")`, `special("s")`, `sequence(...)`, `choice(...)`, `optional(x)`, `zeroOrMore(x)`, `oneOrMore(x)`, `/* */` comments |

A repeated rule name is a warning, and both rules are drawn. A syntax error skips to the end of
its rule; the other rules are still drawn.

## Not drawn

The title is read but not drawn. Repeat bounds other than optional, zero or more, and one or
more are drawn as a plain loop. Mermaid's `railroad` config and its themes are not used.
