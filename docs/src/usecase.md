# Use case diagrams

sirene draws Mermaid's use case diagrams (`usecase-beta`) as images and as text: actors, use
cases, system boundaries, and the relationships between them.

```
$ sirene ascii shop.mmd
                  ┌─ Storefront ──────────────────────────────────────┐
                  │                             ╭──────────────────╮  │
                ┌─┼────────────────────────────►( Browse catalogue )  │
┌──────────┐ ┌──┘ │                           ┌►(                  )  │
│          ├─┘    │  ╭──────────╮ ┌┄«include»┄┘ ╰──────────────────╯  │
│ Customer ├──────┼─►( Checkout ├┄┘                                   │
│          ├─┐    │  ╰──────────╯                                     │
└──────────┘ │    └───────────────────────────────────────────────────┘
             │
             └──┐               ┌─ Fulfilment ─────────┐
                │               │  ╭────────────────╮  │
                └───────────────┼─►( Track delivery )  │
┌─────────┐     ┌───────────────┼─►(                )  │
│ Support ├─────┘               │  ╰────────────────╯  │
└─────────┘                     └──────────────────────┘
```

```
usecase-beta
direction LR
actor Customer
actor Support
systemBoundary Storefront
  Browse("Browse catalogue")
  Checkout("Checkout")
end
systemBoundary Fulfilment
  Track("Track delivery")
end
Customer --> Browse
Customer --> Checkout
Customer --> Track
Support --> Track
Checkout ..> : include Browse
```

As an image, actors are stick figures with their names below. See
`crates/sirene-core/tests/fixtures/usecase/valid/` for more. In the editor a use case diagram is
read-only on the drawing side.

## What is read

| Syntax | Drawn as |
|---|---|
| `actor A`, `actor A("label")`, `actor A, B` | a stick figure |
| `@{ type: hollow }`, `@{ type: awesome }` | a hollow outline, or a filled silhouette |
| `U("label")`, `U[label]`, `"label"`, an undeclared endpoint | an ellipse, or a rectangle for `[...]` |
| `@{ business: true }` | the business slash on an actor or ellipse |
| `<<Stereotype>>` | `«Stereotype»` above the label |
| `systemBoundary id["title"] ... end`, `@{ type: package }` | a box around its members, with a title tab for `package` |
| `-->`, `<--`, `--`, `--o`, `o--`, `--x`, `x--`, with more dashes for a longer edge | an association with that marker |
| `-- label -->` and the other labelled forms | a labelled association |
| `..> : include`, `..> : extend` | a dotted arrow labelled `«include»` or `«extend»` |
| `--\|>` | a generalization, with a hollow triangle |
| `A id@--> B`, `id@{ animate: true }`, `id@{ animation: fast }` | an edge that `class`, `style`, and metadata can name |
| `note for U "text"` | a note linked to its actor or use case |
| `json T@{ ... }` | a table of the object's keys and values, nested keys as `a.b` and `items[0].name` |
| `:::a,b`, `class a,b c,d`, `classDef`, `style` | classes and styles |
| `direction`, `accTitle`, `accDescr`, `%% comment` | read |

Mermaid's rules (an icon with a non-normal `type`, `include` between non-use-cases, a JSON table
with a circle or cross marker, and the like) are warnings, and the diagram is still drawn.

## Not drawn

Icon actors (`@{ icon: "fa:user" }`) are drawn as stick figures. Mermaid 12's `redux-color`
theme, `neo` look, ELK layout, and the `usecase` frontmatter config are not used.
