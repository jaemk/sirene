# Info diagrams

sirene draws Mermaid's `info` diagram as its own version, where Mermaid draws Mermaid's:

```
$ echo info | sirene ascii
sirene v0.2.1
```

`showInfo` (after `info` or on its own line) is read and draws the same. A `title` line or
frontmatter `title` is drawn above the version. Any other line is an error.
