# ER diagrams

sirene draws Mermaid entity relationship diagrams (`erDiagram`) with the same layout, themes, and
outputs as flowcharts:

```
$ sirene ascii shop.mmd
    ┌──────────┐
    │ CUSTOMER │
    └──────────┘
          1
       places
          *
┌──────────────────┐
│      ORDER       │
├──────────────────┤
│ int    id     PK │
│ string status    │
└──────────────────┘
```

```
erDiagram
  CUSTOMER ||--o{ ORDER : places
  ORDER {
    int id PK
    string status
  }
```

In the editor an ER diagram is read-only on the drawing side, as state and class diagrams are.

## What is read

| Syntax | Drawn as |
|---|---|
| `A`, `"Quoted name"`, `A[Alias]`, `A["Alias"]` | a box titled with the name or alias |
| `A { type name PK, FK "comment" }` | one row per attribute, in columns of type, name, keys, and comment |
| `\|o`, `\|\|`, `}o`, `}\|` (mirrored on the right: `o\|`, `\|\|`, `o{`, `\|{`) | zero or one, exactly one, zero or more, one or more |
| `--` | a solid line (identifying) |
| `..`, `.-`, `-.` | a dotted line (non-identifying) |
| `one or more to zero or many`, `only one optionally to 1+`, ... | the same, in words; `optionally to` is dotted |
| `A \|\|--o{ B : label` | the label in the middle; it may be quoted |
| `List~T~` in a type | `List<T>` |
| `direction`, `classDef`, `class A,B name`, `A:::name`, `style` | as in flowcharts |

An edge runs from the left entity to the right one. In ASCII the markers are `?` (zero or one),
`1` (exactly one), `*` (zero or more), and `+` (one or more), next to the entity.

## Not drawn

Styling of an entity that does not exist is a warning, and the rest of the diagram is drawn.
