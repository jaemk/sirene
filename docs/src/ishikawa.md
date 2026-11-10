# Ishikawa diagrams

sirene draws Mermaid's ishikawa (fishbone) diagrams (`ishikawa-beta`) as images and as text. The
first line is the effect, and the indented lines under it are its causes:

```
$ sirene ascii --charset ascii delivery.mmd
Late delivery
+-- People
|   `-- Short staffed
|       `-- Holidays
+-- Methods
`-- Machines
    `-- Broken scanner
```

```
ishikawa-beta
    Late delivery
    People
        Short staffed
            Holidays
    Methods
    Machines
        Broken scanner
```

As an image, the effect sits in a head at the end of a spine, with the causes on bones above and
below it. See `crates/sirene-core/tests/fixtures/ishikawa/valid/` for more. In the editor an
ishikawa diagram is read-only on the drawing side.

## What is read

| Syntax | Drawn as |
|---|---|
| the first line | the effect, in the head |
| a line indented under another | a cause of that line |
| `<br>` | a line break |
| `%% comment`, frontmatter `title` | read; the title is drawn above |

Every line is a label, so there are no errors past the header. The first cause's indentation is
the causes' level, and a line indented at or left of it is a cause of the effect.

## Not drawn

The hand-drawn look and the `ishikawa` config options are not read.
