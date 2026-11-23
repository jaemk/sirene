// Extracts the baseline cases for sirene's LaTeX typesetter from KaTeX's tests and dumps KaTeX's
// own answers for them (spec/latex-math.md, LMATH-13). Development tool: the committed outputs
// are what the Rust tests read, and nothing here runs in CI.
//
// Setup (once):
//   git clone --depth 1 --branch v0.19.1 https://github.com/KaTeX/KaTeX.git local/katex
//   npm install --prefix local/katex-oracle katex@0.19.1 yaml --no-audit --no-fund
// Run:
//   node scripts/katex-fixtures.mjs
//
// Writes crates/sirene-core/tests/fixtures/math/katex-cases.json (inputs and what KaTeX's tests
// expect of them) and katex-oracle.json (KaTeX's parse tree, box height and depth in em, or its
// error message, for each input). Adapted from KaTeX's test suite, MIT licensed, Copyright (c)
// 2013-2020 Khan Academy and other contributors; see THIRD-PARTY-NOTICES.

import { createRequire } from "node:module";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";

const require = createRequire(new URL("../local/katex-oracle/", import.meta.url));
const katex = require("katex");
const YAML = require("yaml");

const kx = new URL("../local/katex/", import.meta.url);
const out = new URL("../crates/sirene-core/tests/fixtures/math/", import.meta.url);
mkdirSync(out, { recursive: true });

const cases = [];

// `expect`TeX`.toParse()` and friends in katex-spec.ts: the input is a raw template literal.
const spec = readFileSync(new URL("test/katex-spec.ts", kx), "utf8");
const tagged =
  /expect`((?:[^`\\]|\\.)*)`\.(not\.)?(toParse|toBuild|toParseLike|toBuildLike|toFailWithParseError)\(/g;
for (const m of spec.matchAll(tagged)) {
  const line = spec.slice(0, m.index).split("\n").length;
  const matcher = m[3];
  const expect = m[2]
    ? matcher === "toParse"
      ? "no_parse"
      : "no_build"
    : { toParse: "parse", toBuild: "build", toParseLike: "parse_like", toBuildLike: "build_like", toFailWithParseError: "fail" }[
        matcher
      ];
  const entry = { src: `katex-spec.ts:${line}`, input: m[1], expect };
  const args = callArgs(spec, m.index + m[0].length);
  if (matcher === "toParseLike" || matcher === "toBuildLike") {
    const like = /^\s*(?:"((?:[^"\\]|\\.)*)"|'((?:[^'\\]|\\.)*)'|`((?:[^`\\]|\\.)*)`)\s*(?:,\s*([\s\S]*))?$/.exec(args);
    if (like) {
      entry.like = like[1] ?? like[2] ?? like[3];
      if (like[4]?.trim()) entry.settings_raw = like[4].trim();
    }
  } else if (args.trim()) {
    entry.settings_raw = args.trim();
  }
  cases.push(entry);
}

// The text of a call's arguments, from just after its `(` to the matching `)`.
function callArgs(text, from) {
  let depth = 1;
  let i = from;
  let quote = null;
  for (; i < text.length && depth > 0; i++) {
    const ch = text[i];
    if (quote) {
      if (ch === "\\") i++;
      else if (ch === quote) quote = null;
    } else if (ch === '"' || ch === "'" || ch === "`") quote = ch;
    else if (ch === "(") depth++;
    else if (ch === ")") depth--;
  }
  return text.slice(from, i - 1);
}

// ss_data.yaml: the screenshot inputs; a string, or a map with tex, display, macros.
const ss = YAML.parse(readFileSync(new URL("test/screenshotter/ss_data.yaml", kx), "utf8"));
for (const [name, v] of Object.entries(ss)) {
  const o = typeof v === "string" ? { tex: v } : v;
  if (typeof o.tex !== "string") continue;
  const entry = { src: `ss_data.yaml:${name}`, input: o.tex, expect: o.noThrow ? "no_build" : "build" };
  if (o.display) entry.display = true;
  if (o.macros) entry.macros = o.macros;
  cases.push(entry);
}

function findClass(node, cls) {
  if (node.classes?.includes(cls)) return node;
  for (const child of node.children ?? []) {
    const found = findClass(child, cls);
    if (found) return found;
  }
  return null;
}

function stripLoc(_, v) {
  return _ === "loc" ? undefined : v;
}

const oracle = {};
for (const c of cases) {
  const key = JSON.stringify([c.input, c.display ?? false, c.macros ?? null]);
  if (key in oracle) continue;
  const settings = {
    throwOnError: true,
    strict: "ignore",
    output: "html",
    displayMode: !!c.display,
    macros: c.macros ? { ...c.macros } : undefined,
  };
  const entry = {};
  try {
    entry.tree = JSON.parse(JSON.stringify(katex.__parse(c.input, { ...settings }), stripLoc));
  } catch (e) {
    entry.parse_error = String(e.rawMessage ?? e.message);
  }
  if (entry.tree) {
    try {
      const dom = katex.__renderToDomTree(c.input, { ...settings });
      const html = findClass(dom, "katex-html");
      entry.height = html.height;
      entry.depth = html.depth;
    } catch (e) {
      entry.build_error = String(e.rawMessage ?? e.message);
    }
  }
  oracle[key] = entry;
}

// A case whose KaTeX test passes arguments this script does not reproduce (settings it could
// not read) can disagree with the oracle; mark it so the Rust tests skip it and a person reads it.
for (const c of cases) {
  if (c.settings_raw) continue;
  const v = oracle[JSON.stringify([c.input, c.display ?? false, c.macros ?? null])];
  const parsed = !!v.tree;
  const built = parsed && v.height !== undefined;
  const disagrees =
    ((c.expect === "parse" || c.expect === "parse_like") && !parsed) ||
    (c.expect === "build" && !built) ||
    (c.expect === "no_parse" && parsed);
  if (disagrees) c.disagrees = true;
}
writeFileSync(new URL("katex-cases.json", out), JSON.stringify(cases, null, 1) + "\n");
writeFileSync(new URL("katex-oracle.json", out), JSON.stringify(oracle) + "\n");
console.log(`${cases.length} cases, ${Object.keys(oracle).length} distinct inputs`);
