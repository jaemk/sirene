# Layout

Layered layout of a compound graph (nodes, nested clusters, edges) into integer cell
coordinates. The ASCII renderer uses one cell per character; pixel renderers scale cells.

Input: per node a size and parent cluster; per cluster a parent, an optional direction, and a
title size; per edge two endpoints (node or cluster), a minimum length, and an optional label
size; a diagram direction; spacing for vertical and horizontal directions.

Output: a rect per node and cluster, a title rect per titled cluster, and per edge an orthogonal
route plus a label rect.

## LAYOUT-1 Coordinates

Integer cells, x to the right, y down, origin at the diagram's top-left. A rect covers cells
`x..x+w` and `y..y+h`. Route points are cells. The diagram size covers every rect and route.

## LAYOUT-2 Containers

The diagram and every cluster are containers. A cluster is laid out first and then placed in its
parent as a single block, so a cluster rect contains all of its descendants and nothing else. A
cluster's border is the outermost row and column of its rect. Cluster parents that form a cycle
are treated as top level.

## LAYOUT-3 Direction

Each container is laid out top to bottom and then rotated or flipped into its direction. A
cluster with an explicit direction uses it only when no edge reaches it from outside (an edge
with one endpoint outside and the other being the cluster or inside it); otherwise it inherits
its parent's direction, as in mermaid. A cluster without a direction inherits.

## LAYOUT-4 Ranks

Within a container, the items are its direct nodes and child clusters. Each edge belongs to the
innermost container holding both endpoints and connects the items that contain them. Cycles are
broken by reversing back edges found by a depth-first search in input order. Every edge spans at
least `2 * min_len` ranks, and ranks are chosen to shorten edges. An edge label sits on the
middle rank of its edge.

## LAYOUT-5 Order

Long edges get a dummy per intermediate rank. Items in each rank are ordered by barycenter
sweeps, keeping the order with the fewest crossings. Layout is deterministic.

## LAYOUT-6 Positions

Items in a rank do not overlap and are separated by the node gap (the edge gap between two edge
dummies). Items are pulled toward the ports of their neighbors so edges run straight where
possible. A rank is as tall as its tallest item, and shorter items are centered in it.

## LAYOUT-7 Ports

An edge leaves the downstream side of its upper item and enters the upstream side of its lower
item. The ports on one side of a node are spread evenly across it, inset from the corners, and
ordered by the position of the other end; a node grows along the rank when its ports would not
fit. An edge reaching a node inside a cluster, or the cluster itself, crosses the cluster border
at its own port on the corresponding side.

## LAYOUT-8 Routes

Routes are orthogonal. Between two ranks a route runs along the main axis and turns at most once
onto a cross-axis track in the gap; gaps grow to fit their tracks. Tracks are ordered so a route
does not run along another route's segment. A route starts on its source's border, ends on its
target's border, and passes through its label rect. The points run from the edge's source to its
target.

## LAYOUT-9 Self-loops

An edge from an item to itself loops around the item's cross-axis end side (right of the item in
a top-to-bottom container), with its label beside the loop. Space is reserved for the loop.

## LAYOUT-10 Unroutable edges

An edge between a cluster and one of its descendants has no route.

## LAYOUT-11 Cluster titles

A cluster title's rect starts on the cluster's top border row and lies inside the cluster. In a
top-bottom or bottom-top cluster the title sits in the top border row with the border ports,
kept clear of them and pulled toward the middle. In a left-right or right-left cluster it starts
at the main-axis padding from the left border, and the cluster is at least wide enough for it.
No route passes through a title.

## LAYOUT-12 Lanes

A top-level cluster marked as a lane is not a block: its items join the diagram's ranks with
everyone else's. In each rank the items are ordered lane by lane, in input order, and each lane
is positioned on its own, so the lanes are bands side by side along the cross axis, sharing
their borders, each as wide as its content and title need and centering its content. Every lane
spans the diagram along the main axis, and the diagram gets border rows as a cluster does. Edge
dummies keep the upper end's lane above the edge's middle rank and the lower end's from it on.
A lane's title sits on its real top border, at the padding from its left border. An edge to or
from a lane itself has no route. Tests: `placement` and `edges_to_lanes` in
`crates/sirene-core/tests/swimlane.rs`.
