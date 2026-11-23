// A Node module resolver hook for the KaTeX checkout: its sources import siblings without a file
// extension, which Node's ESM loader needs spelled out. It tries `.ts`, `.js`, then an index
// file. Registered by scripts/katex-macros.mjs; used only to read KaTeX's tables.

import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";

export async function resolve(specifier, context, next) {
  if (specifier.startsWith(".") && context.parentURL) {
    const base = new URL(specifier, context.parentURL);
    for (const ext of [".ts", ".js", "/index.ts", "/index.js"]) {
      const candidate = base.href + ext;
      if (existsSync(fileURLToPath(candidate))) {
        return next(candidate, context);
      }
    }
  }
  return next(specifier, context);
}
