# PlantUML Wire and Board

PlantUML `@startwire` (wire or block diagrams) and `@startboard` (board or story map) documents
(PUML-2). Both are beta in PlantUML and have no page in its reference: the grammar is in its
Java sources (`net.sourceforge.plantuml.wire` and `net.sourceforge.plantuml.board`) and in the
unofficial wiki. Today each kind is an error naming it.

## PWIRE-1 Status

Research: the grammar is not specified here. Before this is built, read the PlantUML
implementation and write the accepted statements, the model each draws on (graph for wire, kanban
or tree view for board), and fixtures written for sirene (PUML-5).

## PWIRE-2 Until then

`@startwire` and `@startboard` stay errors naming the kind, so no source silently draws wrong.

## PWIRE-3 Open questions

- Whether either grammar is stable enough to follow; a beta that changes between releases
  would make a spec go stale.
