# @jaemk/sirene

[sirene](https://github.com/jaemk/sirene)'s renderer compiled to WebAssembly: draws Mermaid,
PlantUML, and Graphviz DOT as SVG or ASCII, with no server and no dependencies.

```html
<script src="https://cdn.jsdelivr.net/npm/@jaemk/sirene@VERSION/sirene-wasm.js"></script>
<script>
  SireneWasm.load(fetch("https://cdn.jsdelivr.net/npm/@jaemk/sirene@VERSION/sirene.wasm"))
    .then((sirene) => sirene.renderAll(document, { theme: "light" }));
</script>
```

`renderAll` replaces each `mermaid`, `plantuml`, and `dot` code block, and each mermaid.js
`.mermaid` block, with its drawing. To draw one source:

```js
const { readFileSync } = require("node:fs");
const SireneWasm = require("@jaemk/sirene");
const sirene = await SireneWasm.load(readFileSync(require.resolve("@jaemk/sirene/sirene.wasm")));
const r = sirene.render("flowchart LR\n  a --> b\n", { format: "svg", theme: "light" });
// r.ok, r.output, r.error, r.diagnostics, r.pages
```

A Visio `.vsdx` is drawn from its bytes: `sirene.render(bytes, { lang: "visio", page: 0 })`,
where `bytes` is a `Uint8Array` and `page` (from 0, default 0) picks a page of `r.pages`.

See [Web pages](https://jaemk.github.io/sirene/guide/embedding.html) for every option and
[`examples/web/index.html`](https://github.com/jaemk/sirene/blob/main/examples/web/index.html)
for a complete page.

MIT or Apache-2.0.
