# PlantUML Skinparam

Mapping of PlantUML `skinparam`, `<style>` blocks, and `!theme` to sirene's themes and element
styles (PUML-4), in `sirene_core::plantuml`. Reference: <https://plantuml.com/skinparam>. Today
each is a warning and is skipped, except `actorStyle` in use case diagrams.

## PSKIN-1 Scope

A subset is read: background, border, and font colors per element kind (`rectangleBackgroundColor`,
`classBorderColor`, `sequenceArrowColor`, ...), the global `BackgroundColor`, `ArrowColor`,
`FontColor`, `FontName`, and `FontSize`, `roundCorner`, and `shadowing`. In graph-model kinds a
parameter sets the `Style` of the nodes of its kind; in other models it sets the matching theme
field. A parameter with no equivalent is a warning naming it, as now.

## PSKIN-2 Blocks and themes

`skinparam x {` ... `}` and `skinparam x y` set the same values. A `<style>` block's selectors
for element names are read for the same subset. `!theme NAME` names a built-in or remote theme;
a name sirene has no theme for is a warning, and remote and file themes are not read (PUML-4).

## PSKIN-3 Open questions

- Which of the roughly 150 parameters real files use most; the subset above comes from the
  language reference, not usage.
- Whether `<style>` selectors beyond element names (`.stereotype`, `:hover`) are worth reading.
