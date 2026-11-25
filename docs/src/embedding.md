# Web pages

`sirene-core` built for WebAssembly draws Mermaid, PlantUML, and DOT in the browser, with no
server and no other dependencies. The [playground](playground.md) and the `edit` button on each
diagram in these docs use it.

## Files

From npm, as [`@jaemk/sirene`](https://www.npmjs.com/package/@jaemk/sirene):

```
npm install @jaemk/sirene
```

It is published by the release workflow with npm provenance (`npm audit signatures` checks it).
jsDelivr and unpkg serve the package's files to any page, so a page can load them without
hosting them:

```html
<script src="https://cdn.jsdelivr.net/npm/@jaemk/sirene@X.Y.Z/sirene-wasm.js"></script>
<script>
  SireneWasm.load(fetch("https://cdn.jsdelivr.net/npm/@jaemk/sirene@X.Y.Z/sirene.wasm"))
    .then(function (sirene) { sirene.renderAll(document, { theme: "light" }); });
</script>
```

Or from GitHub: each release after 0.6.0 attaches `sirene-wasm-X.Y.Z.tar.gz`. Both hold:

| File | |
| --- | --- |
| `sirene.wasm` | the module (about 2.7 MB, 0.9 MB gzipped); SVG and ASCII output |
| `sirene-wasm.js` | the loader: a plain script that sets `window.SireneWasm` |
| `LICENSE-MIT`, `LICENSE-APACHE`, `THIRD-PARTY-NOTICES` | licenses |

```
curl -fsSLO https://github.com/jaemk/sirene/releases/download/vX.Y.Z/sirene-wasm-X.Y.Z.tar.gz
tar -xzf sirene-wasm-X.Y.Z.tar.gz
```

`SHA256SUMS` in the release lists its checksum, and
`gh attestation verify sirene-wasm-X.Y.Z.tar.gz --repo jaemk/sirene` checks its build
provenance.

Serve both files from your own site. GitHub release downloads can't be loaded from a page
directly (no CORS headers), and browsers do not fetch WebAssembly from `file://` pages.

## Drawing a page's diagram blocks

`renderAll` replaces each diagram block with an `<img class="sirene-diagram">` of its SVG:

```html
<pre><code class="language-mermaid">flowchart LR
  A --&gt; B</code></pre>

<script src="sirene-wasm.js"></script>
<script>
  SireneWasm.load(fetch("sirene.wasm")).then(function (sirene) {
    sirene.renderAll(document, { theme: "light" });
  });
</script>
```

It draws `<pre><code class="language-X">` blocks, as Markdown renderers write them, for
`mermaid`, `plantuml`, `puml`, `dot`, and `graphviz`, and mermaid.js's `<pre class="mermaid">`
and `<div class="mermaid">`. A block it cannot draw stays as source, with the error in its
`data-sirene-error` attribute; `renderAll` returns `{ drawn, failed }`. Note that a block with
errors in its source is still drawn, as `sirene svg` draws it.

[`examples/web/index.html`](https://github.com/jaemk/sirene/blob/main/examples/web/index.html)
is a complete page.

## Drawing one source

```js
const sirene = await SireneWasm.load(fetch("sirene.wasm"));
const r = sirene.render(source, { format: "svg", lang: "mermaid", theme: "dark" });
if (r.ok) {
  // r.output: the SVG document as a string ("ascii": the `sirene ascii` text)
}
for (const d of r.diagnostics) {
  console.log(`${d.severity}: ${d.line}:${d.column}: ${d.message}`);
}
```

| Option | Values |
| --- | --- |
| `format` | `svg` (default), `ascii` |
| `lang` | `detect` (default, from the source's header), `mermaid`, `plantuml` or `puml`, `dot` or `graphviz`, `latex` or `tex` (one formula), `visio` or `vsdx` (a `.vsdx` package, given as a `Uint8Array`; also detected by its `PK` magic) |
| `page` | For a Visio drawing, the page to draw, from 0 (default 0); `r.pages` is the page count |
| `theme` | `dark` (default), `light`; a diagram's own `theme` setting applies over it |

`render` is synchronous. A source that crashes the module returns `ok: false` and the next call
starts a fresh instance. The module imports nothing, so it also runs outside a browser, e.g.
Node:

```js
const SireneWasm = require("@jaemk/sirene");
const wasm = fs.readFileSync(require.resolve("@jaemk/sirene/sirene.wasm"));
const sirene = await SireneWasm.load(wasm);
```

Loaded as a plain script, `sirene-wasm.js` sets `globalThis.SireneWasm` instead.

A page with a Content Security Policy needs `script-src 'wasm-unsafe-eval'` to compile the
module and `img-src data:` for `renderAll`'s images.
