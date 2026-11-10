# Treemaps

sirene draws Mermaid treemaps (`treemap-beta`) as images and as text. In text, the items are an
outline, largest first, with each value and a bar for its share of the total:

```
$ sirene ascii budget.mmd
Housing       $2,010.50  ██████████▎
├─ Rent       $1,800.50  █████████▏
└─ Utilities    $210.00  █▏

Food          $1,820.00  █████████▎
├─ Dining     $1,180.00  ██████
└─ Groceries    $640.00  ███▎

Savings         $900.00  ████▋
```

```
---
config:
  treemap:
    valueFormat: "$,.2f"
    padding: 6
---
treemap-beta
"Housing"
  "Rent": 1800.5
  "Utilities": 210
"Food"
  "Groceries": 640:::big
  "Dining", 1,180
"Savings": 900:::calm
classDef big fill:#f96,stroke:#333,stroke-width:2px
classDef calm fill:#9cf,color:#036
```

In the editor a treemap is read-only on the drawing side.

## What is read

| Syntax | Drawn as |
|---|---|
| `"Name"` | a section holding the items indented under it |
| `"Name": 10` or `"Name", 10` | a leaf whose area is its value; `,` and `_` in a value are ignored |
| `'Name'` | single quotes work as well |
| `"Name": 10:::cls` | a leaf or section styled by `classDef cls` (`fill`, `stroke`, `stroke-width`, `color`) |
| `title`, frontmatter `title` | the title above |
| `accTitle`, `accDescr` | read, not drawn |
| `config.treemap.valueFormat` | a d3-format string such as `",.2f"`, `"$,"`, `".1%"`, or `".2s"` |
| `config.treemap.showValues` | `false` leaves values out |
| `config.treemap` | also `padding`, `diagramPadding`, `nodeWidth`, and `nodeHeight` |

A section's value is the sum of its items. Several top-level items are allowed.

## Errors

A value that is not a number, a missing value after `:`, a name not closed on its line, and a
line that is not an item, `classDef`, or `title` are errors. A section with no items, a class
with no `classDef`, and a bad `config.treemap` value are warnings.

## Not drawn

Theme variables are ignored.
