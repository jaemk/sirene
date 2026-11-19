// Renders each file in examples/, and a gantt chart (which reads the date), as SVG, ASCII, and
// PNG with sirene-wasm built for wasm32-unknown-unknown, through the docs site's loader
// (docs/assets/sirene-wasm.js), under Node (WORKSP-11), then checks the loader's `renderAll` on
// a stand-in document and `load`'s inputs. Any failure fails the script.
//
//   cargo build -p sirene-wasm --features png --target wasm32-unknown-unknown
//   node scripts/wasm-test.mjs [target/wasm32-unknown-unknown/debug/sirene_wasm.wasm]
import { readdirSync, readFileSync } from "node:fs";
import { extname, join } from "node:path";

await import("../docs/assets/sirene-wasm.js");

const path = process.argv[2] ?? "target/wasm32-unknown-unknown/debug/sirene_wasm.wasm";
const sirene = await globalThis.SireneWasm.load(readFileSync(path));

const LANGS = { ".mmd": "mermaid", ".dot": "dot", ".puml": "plantuml" };
const sources = readdirSync("examples")
  .filter((name) => LANGS[extname(name)])
  .sort()
  .map((name) => [name, LANGS[extname(name)], readFileSync(join("examples", name), "utf8")]);
sources.push(["gantt", "mermaid", "gantt\n  title Plan\n  section A\n  Task :a1, 2026-01-01, 3d\n"]);

let failed = 0;
function check(name, what, pass, detail) {
  if (!pass) {
    console.error(`${name}: ${what}: ${detail}`);
    failed++;
  }
}

for (const [name, lang, src] of sources) {
  const svg = sirene.render(src, { format: "svg", lang, theme: "light" });
  check(name, "svg", svg.ok && svg.output.startsWith("<svg"), svg.error);
  const errors = svg.diagnostics.filter((d) => d.severity === "error");
  check(name, "diagnostics", errors.length === 0, JSON.stringify(errors));
  const ascii = sirene.render(src, { format: "ascii", lang });
  check(name, "ascii", ascii.ok && ascii.output.length > 0, ascii.error);
  const png = sirene.render(src, { format: "png", lang, theme: "dark" });
  const sig = png.ok && Buffer.from(png.output.subarray(0, 4)).toString("latin1");
  check(name, "png", sig === "\x89PNG", png.error);
}

// Diagnostics carry their 1-based line and column.
const bad = sirene.render("flowchart LR\n  a --> \n", { format: "svg" });
check("bad", "diagnostics", bad.diagnostics[0]?.line === 2, JSON.stringify(bad.diagnostics));

// `renderAll` on a stand-in document: the selector it asks for, a drawn code block, a drawn
// mermaid.js block, and a block it cannot draw (an unknown language).
const doc = {
  createElement: () => ({}),
  query: null,
  querySelectorAll(selector) {
    this.query = selector;
    return this.blocks;
  },
};
function block(tagName, className, text) {
  const el = { tagName, className, textContent: text, ownerDocument: doc, attrs: {} };
  el.setAttribute = (k, v) => (el.attrs[k] = v);
  el.replaceWith = (img) => (el.replacedBy = img);
  return el;
}
const pre = block("PRE", "", "");
const code = block("CODE", "language-dot", "digraph { a -> b }");
code.parentNode = pre;
const mermaid = block("PRE", "mermaid", "flowchart LR\n  a --> b\n");
const unknown = block("CODE", "language-nope", "x");
unknown.parentNode = block("PRE", "", "");
doc.blocks = [code, mermaid];
const all = sirene.renderAll(doc);
check("renderAll", "selector", doc.query.includes("pre > code.language-plantuml") && doc.query.includes("div.mermaid"), doc.query);
check("renderAll", "drawn", all.drawn === 2 && all.failed.length === 0,
  `drawn ${all.drawn}, errors ${all.failed.map((f) => f.error)}`);
const img = pre.replacedBy;
check("renderAll", "img", img?.className === "sirene-diagram" && img.alt === "dot diagram"
  && decodeURIComponent(img.src.split(",")[1]).startsWith("<svg"), JSON.stringify(img));
check("renderAll", "mermaid", mermaid.replacedBy?.alt === "mermaid diagram", "not replaced");
// One block it cannot draw does not stop the others.
const after = block("PRE", "mermaid", "flowchart LR\n  c\n");
doc.blocks = [unknown, after];
const some = sirene.renderAll(doc);
check("renderAll", "failed", some.drawn === 1 && some.failed[0]?.element === unknown.parentNode
  && unknown.parentNode.attrs["data-sirene-error"]?.includes("unknown language"),
  `drawn ${some.drawn}, errors ${some.failed.map((f) => f.error)}`);

// `load` takes a Response or a promise of one, and names a failed fetch.
const bytes = readFileSync(path);
const fromResponse = await globalThis.SireneWasm.load(Promise.resolve(new Response(bytes)));
check("load", "response", fromResponse.render("flowchart LR\n  a\n").ok, "did not draw");
const missing = await globalThis.SireneWasm.load(new Response("", { status: 404 })).then(
  () => "loaded",
  (e) => e.message
);
check("load", "404", missing.includes("404"), missing);

if (failed) process.exit(1);
console.log(`sirene-wasm rendered ${sources.length} sources`);
