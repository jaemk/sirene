// Loads sirene-wasm (wasm/src/lib.rs) and draws diagrams with it. Shared by the docs site
// (custom.js), scripts/wasm-test.mjs, and pages that embed sirene (the release's
// sirene-wasm-X.Y.Z.tar.gz and the @jaemk/sirene npm package; see docs/src/embedding.md), so it
// is a plain script that sets `globalThis.SireneWasm` (and `module.exports` under CommonJS).
//
//   var sirene = await SireneWasm.load(fetch("sirene.wasm"));
//   var r = sirene.render(source, { format: "svg", lang: "mermaid", theme: "dark" });
//   // r.ok, r.output (a string, or a Uint8Array for "png"), r.error, r.diagnostics
//   sirene.renderAll(); // replaces each diagram block on the page with its drawing
(function (root) {
  var FORMATS = { svg: 0, ascii: 1, png: 2 };
  var LANGS = { detect: 0, mermaid: 1, dot: 2, graphviz: 2, plantuml: 3, puml: 3, latex: 4, tex: 4 };
  var THEMES = { light: 0, dark: 1 };

  function code(table, name, what) {
    if (!Object.prototype.hasOwnProperty.call(table, name)) {
      throw new Error("sirene: unknown " + what + " " + JSON.stringify(name));
    }
    return table[name];
  }

  function Sirene(module) {
    this.module = module;
    this.instance = null;
  }

  Sirene.prototype.exports = function () {
    if (!this.instance) this.instance = new WebAssembly.Instance(this.module, {});
    return this.instance.exports;
  };

  Sirene.prototype.render = function (source, options) {
    options = options || {};
    var format = code(FORMATS, options.format || "svg", "format");
    var lang = code(LANGS, options.lang || "detect", "language");
    var theme = code(THEMES, options.theme || "dark", "theme");
    var bytes = new TextEncoder().encode(source);
    var x = this.exports();
    var status;
    try {
      var at = x.input(bytes.length);
      new Uint8Array(x.memory.buffer, at, bytes.length).set(bytes);
      status = x.render(format, lang, theme);
    } catch (e) {
      // A panic traps and may leave the instance's state half-written: start a new one.
      this.instance = null;
      return { ok: false, output: null, error: "sirene crashed drawing this source", diagnostics: [] };
    }
    // Read after the call: memory may have grown, which detaches earlier views.
    var out = new Uint8Array(x.memory.buffer, x.output_ptr(), x.output_len()).slice();
    var diagnostics = JSON.parse(
      new TextDecoder().decode(new Uint8Array(x.memory.buffer, x.diagnostics_ptr(), x.diagnostics_len()))
    );
    var text = format === FORMATS.png && status === 0 ? null : new TextDecoder().decode(out);
    return {
      ok: status === 0,
      output: status === 0 ? (text === null ? out : text) : null,
      error: status === 0 ? null : text,
      diagnostics: diagnostics,
    };
  };

  // The diagram blocks `renderAll` draws: Markdown renderers' `<pre><code class="language-X">`,
  // and mermaid.js's `<pre class="mermaid">` and `<div class="mermaid">`.
  var BLOCKS = [
    "pre > code.language-mermaid",
    "pre > code.language-plantuml",
    "pre > code.language-puml",
    "pre > code.language-dot",
    "pre > code.language-graphviz",
    "pre.mermaid",
    "div.mermaid",
  ].join(", ");

  // Replaces each diagram block under `scope` (default: the document) with an
  // `<img class="sirene-diagram">` of its SVG, in `options.theme` (default "light"). A block
  // sirene cannot draw stays as it is, with the error in its `data-sirene-error` attribute.
  // Returns { drawn, failed: [{ element, error }] }.
  Sirene.prototype.renderAll = function (scope, options) {
    options = options || {};
    var self = this;
    var drawn = 0;
    var failed = [];
    (scope || document).querySelectorAll(BLOCKS).forEach(function (el) {
      var isCode = el.tagName === "CODE";
      var block = isCode ? el.parentNode : el;
      var lang = isCode ? el.className.match(/language-(\w+)/)[1] : "mermaid";
      var r;
      try {
        r = self.render(el.textContent, { format: "svg", lang: lang, theme: options.theme || "light" });
      } catch (e) {
        r = { ok: false, error: e.message };
      }
      if (!r.ok) {
        block.setAttribute("data-sirene-error", r.error);
        failed.push({ element: block, error: r.error });
        return;
      }
      var img = block.ownerDocument.createElement("img");
      img.className = "sirene-diagram";
      img.alt = lang + " diagram";
      img.src = "data:image/svg+xml;charset=utf-8," + encodeURIComponent(r.output);
      block.replaceWith(img);
      drawn++;
    });
    return { drawn: drawn, failed: failed };
  };

  root.SireneWasm = {
    // `source` is the module's bytes (an ArrayBuffer or typed array), a fetch Response, or a
    // promise of either.
    load: async function (source) {
      source = await source;
      var bytes = source;
      if (typeof Response !== "undefined" && source instanceof Response) {
        if (!source.ok) throw new Error("sirene: could not fetch " + source.url + ": " + source.status);
        bytes = await source.arrayBuffer();
      }
      return new Sirene(await WebAssembly.compile(bytes));
    },
  };
  // CommonJS (the npm package's `require("@jaemk/sirene")`, or its default import).
  if (typeof module === "object" && module.exports) module.exports = root.SireneWasm;
})(globalThis);
