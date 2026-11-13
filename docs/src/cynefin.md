# Cynefin diagrams

sirene draws Mermaid's Cynefin diagrams (`cynefin-beta`) as images and as text: items placed in
the five domains of the Cynefin framework, with transitions between domains.

```mermaid
cynefin-beta
  title Strategy Categorization

  complex
    "Market research"

  complicated
    "Competitive analysis"

  clear
    "Standard pricing"

  chaotic
    "Crisis management"

  complex --> complicated : "Pattern identified"
  complicated --> clear : "Best practice codified"
  clear --> chaotic : "Complacency"
  chaotic --> complex : "Stabilized"
```

See `crates/sirene-core/tests/fixtures/cynefin/` for more. In the editor a Cynefin diagram is
read-only on the drawing side.

## Syntax

- `cynefin-beta` (or `cynefin-beta:`) first, then `title`, `accTitle`, and `accDescr` as usual.
- A domain keyword (`complex`, `complicated`, `clear`, `chaotic`, or `confusion`) opens a
  domain; the quoted items (`"..."` or `'...'`) after it, on its line or the lines below, belong
  to it. A domain declared again replaces its items, with a warning.
- `from --> to` adds a transition, with an optional `: "label"`. A transition from a domain to
  itself is dropped, with a warning.
- `showDomainDescriptions: false` under `config: cynefin:` in the frontmatter hides each
  domain's decision model and practice.

Domains are always placed the same way: Complex top left, Complicated top right, Chaotic bottom
left, Clear bottom right, and Confusion in the middle. Confusion shows three items and a
`+N more` badge for the rest. The area grows to fit long or many items.

## Not drawn

Mermaid's `width`, `height`, `padding`, `boundaryAmplitude`, and `seed` options and its
`cynefin` theme variables. Boundaries wave the same way in every diagram.
