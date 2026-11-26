# JavaScript library

`@jaemk/sirene` is sirene's renderer compiled to WebAssembly, with a small JavaScript loader. It
draws Mermaid, PlantUML, DOT, LaTeX formulas, and Visio `.vsdx` drawings as SVG, or as text, in a
browser or in Node, with no server and no other dependencies. The [playground](playground.md)
and the `edit` button on each diagram in these docs use it.

```js
const sirene = await SireneWasm.load(fetch("sirene.wasm"));
const r = sirene.render("flowchart LR\n  A --> B\n", { theme: "light" });
document.body.innerHTML = r.output; // the SVG
```

## Install

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
| `sirene.wasm` | the module (about 4.3 MB, 1.5 MB gzipped); SVG and ASCII output |
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

## Examples

### Draw a page's diagram blocks

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

### A live preview

`render` is synchronous, so it can run on each keystroke:

```html
<textarea id="src">flowchart LR
  A --> B</textarea>
<div id="out"></div>
<pre id="problems"></pre>

<script src="sirene-wasm.js"></script>
<script>
  SireneWasm.load(fetch("sirene.wasm")).then(function (sirene) {
    var src = document.getElementById("src");
    function draw() {
      var r = sirene.render(src.value, { theme: "light" });
      if (r.ok) document.getElementById("out").innerHTML = r.output;
      document.getElementById("problems").textContent = r.diagnostics
        .map(function (d) { return d.line + ":" + d.column + ": " + d.message; })
        .join("\n");
    }
    src.addEventListener("input", draw);
    draw();
  });
</script>
```

A source with mistakes is still drawn, so `r.ok` stays true and the problems are in
`r.diagnostics`.

### A formula

```js
const r = sirene.render("\\int_0^1 x^2\\,dx = \\frac{1}{3}", { lang: "latex", theme: "light" });
```

`lang: "latex"` reads the source as one formula.

### A Visio drawing

A `.vsdx` is binary, so pass its bytes. This draws the file a user picks, one page at a time:

```html
<input type="file" id="file" accept=".vsdx">
<div id="out"></div>

<script src="sirene-wasm.js"></script>
<script>
  SireneWasm.load(fetch("sirene.wasm")).then(function (sirene) {
    document.getElementById("file").addEventListener("change", async function (e) {
      var bytes = new Uint8Array(await e.target.files[0].arrayBuffer());
      var out = document.getElementById("out");
      out.textContent = "";
      var first = sirene.render(bytes, { lang: "visio", theme: "light" });
      for (var page = 0; page < (first.pages || 0); page++) {
        var r = page === 0 ? first : sirene.render(bytes, { lang: "visio", page: page, theme: "light" });
        if (r.ok) out.insertAdjacentHTML("beforeend", r.output);
      }
      if (!first.ok) out.textContent = first.error;
    });
  });
</script>
```

`r.pages` is the drawing's page count. See [Visio drawings](visio.md) for what is drawn.

### Node

The module imports nothing, so it also runs outside a browser. This writes an SVG for each
file named on the command line:

```js
// render.mjs: node render.mjs diagram.mmd flow.dot plan.vsdx
import { readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const SireneWasm = require("@jaemk/sirene");
const sirene = await SireneWasm.load(readFileSync(require.resolve("@jaemk/sirene/sirene.wasm")));

for (const file of process.argv.slice(2)) {
  const source = file.endsWith(".vsdx") ? readFileSync(file) : readFileSync(file, "utf8");
  const r = sirene.render(source, { theme: "light" });
  for (const d of r.diagnostics) console.error(`${file}:${d.line}:${d.column}: ${d.message}`);
  if (r.ok) writeFileSync(file.replace(/\.\w+$/, ".svg"), r.output);
  else console.error(`${file}: ${r.error}`);
}
```

The language is detected from the source (a `.vsdx` by its `PK` header). With CommonJS,
`const SireneWasm = require("@jaemk/sirene")` is enough. Bundlers can import the loader as a
module and the `.wasm` as a URL or an asset; pass its bytes, or a `fetch` of it, to `load`.

### Text output

`format: "ascii"` gives what `sirene ascii` prints:

```js
console.log(sirene.render("flowchart LR\n  A --> B\n", { format: "ascii" }).output);
```

```
┌───┐    ┌───┐
│ A ├───►│ B │
└───┘    └───┘
```

## Reference

```js
const sirene = await SireneWasm.load(source);
const r = sirene.render(input, options);
const { drawn, failed } = sirene.renderAll(scope, options);
```

`load(source)` takes the module's bytes (an `ArrayBuffer` or a typed array), a `fetch`
`Response`, or a promise of either, and resolves to the renderer. A failed fetch rejects with the
URL and status.

`render(input, options)` takes the source as a string, or a `Uint8Array` or `ArrayBuffer` of
bytes.

| Option | Values |
| --- | --- |
| `format` | `svg` (default), `ascii` |
| `lang` | `detect` (default, from the source's header), `mermaid`, `plantuml` or `puml`, `dot` or `graphviz`, `latex` or `tex` (one formula), `visio` or `vsdx` (a `.vsdx` package, given as bytes; also detected by its `PK` magic) |
| `page` | For a Visio drawing, the page to draw, from 0 (default 0) |
| `theme` | `dark` (default), `light`; a diagram's own `theme` setting applies over it |

An unknown value for `format`, `lang`, or `theme` throws.

| Result | |
| --- | --- |
| `ok` | `true` when a drawing was made, also when its source has mistakes |
| `output` | the SVG document, or the text for `ascii`; `null` when `ok` is false |
| `error` | why nothing was drawn; `null` when `ok` |
| `diagnostics` | `[{ severity, line, column, message }]`, `severity` `"error"` or `"warning"`, `line` and `column` from 1 (both 1 for a Visio drawing) |
| `pages` | the page count of a Visio drawing |

`renderAll(scope, options)` draws the blocks under `scope` (default `document`); `options.theme`
defaults to `light`.

A source that crashes the module returns `ok: false` and the next call starts a fresh instance.
`format: "png"` exists only in a module built with the `png` feature (`cargo build -p sirene-wasm
--features png`); the released module does not include it. `ascii` fails for a Visio drawing,
which has no text form.

A page with a Content Security Policy needs `script-src 'wasm-unsafe-eval'` to compile the
module and `img-src data:` for `renderAll`'s images.
