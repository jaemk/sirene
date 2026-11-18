// Renders each file in examples/, and a gantt chart (which reads the date), as SVG, ASCII, and
// PNG with sirene-wasm built for wasm32-unknown-unknown, through the docs site's loader
// (docs/assets/sirene-wasm.js), under Node (WORKSP-11). Any failure fails the script.
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

if (failed) process.exit(1);
console.log(`sirene-wasm rendered ${sources.length} sources`);
