# Architecture diagrams

sirene draws Mermaid architecture diagrams (`architecture-beta`) as images and as text. Each edge
names the side it leaves and enters by, and that places its ends on a grid; in text a service is
a box with its icon name:

```
$ sirene ascii --charset ascii api.mmd
+ API (cloud) -------------------------------------+
|                                                  |
| +-----------------+      +---------------------+ |
| | Server (server) +------+ Database (database) | |
| +--------+--------+      +----------+----------+ |
|          |                          |            |
|          |                          |            |
|          |                          |            |
|  +-------+--------+         +-------+--------+   |
|  | Storage (disk) |         | Storage (disk) |   |
|  +----------------+         +----------------+   |
|                                                  |
+--------------------------------------------------+
```

```
architecture-beta
    group api(cloud)[API]

    service db(database)[Database] in api
    service disk1(disk)[Storage] in api
    service disk2(disk)[Storage] in api
    service server(server)[Server] in api

    db:L -- R:server
    disk1:T -- B:server
    disk2:T -- B:db
```

See `crates/sirene-core/tests/fixtures/architecture/valid/` for junctions, arrows, labels, and
group edges. In the editor an architecture diagram is read-only on the drawing side.

## What is read

| Syntax | Drawn as |
|---|---|
| `service id(icon)[Label]` | a service: its icon with the label below |
| `group id(icon)[Label]` | a dashed box titled at the top left, with its icon |
| `junction id` | a point where edges meet |
| `... in group` | inside that group; groups nest |
| `a:R -- L:b` | an edge from `a`'s right side to `b`'s left; `b` is placed right of `a` |
| `a:R --> L:b`, `a:R <-- L:b`, `a:R <--> L:b` | an edge with arrows |
| `a:R -[label]- L:b` | an edge with a label |
| `a{group}:R -- L:b` | an edge from the side of `a`'s group |
| `align row a b`, `align column a b` | the services kept in one row or column, in order |
| `title`, frontmatter `title` | the title above |
| `accTitle`, `accDescr` | read, not drawn |

The icons `cloud`, `database`, `disk`, `internet`, and `server` are drawn; any other icon, such
as `logos:aws-s3`, is a question mark.

## Errors

A statement that is none of the above, a missing id, side, or edge, and an unclosed `(`, `[`, or
`-[` are errors. An id declared twice, a group that is not declared above, an edge to an unknown
id or to a group are errors. `{group}` on a service outside a group, or on both ends in one group,
is a warning and drawn from the service. An `align` with an unknown id or fewer than two members
is a warning.

## Not drawn

Icon packs (`iconify` names) are not loaded, and the layout is a grid rather than Mermaid's
force-directed one. Theme variables are ignored.
