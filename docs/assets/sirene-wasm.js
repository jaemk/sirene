// Loads sirene-wasm (wasm/src/lib.rs) and draws diagrams with it. Shared by the docs site
// (custom.js) and scripts/wasm-test.mjs, so it is a plain script that sets
// `globalThis.SireneWasm`.
//
//   var sirene = await SireneWasm.load(bytesOrResponse);
//   var r = sirene.render(source, { format: "svg", lang: "mermaid", theme: "dark" });
//   // r.ok, r.output (a string, or a Uint8Array for "png"), r.error, r.diagnostics
(function (root) {
  var FORMATS = { svg: 0, ascii: 1, png: 2 };
  var LANGS = { detect: 0, mermaid: 1, dot: 2, graphviz: 2, plantuml: 3, puml: 3 };
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

  root.SireneWasm = {
    // `source` is the module's bytes (an ArrayBuffer or typed array) or a fetch Response.
    load: async function (source) {
      var bytes = typeof Response !== "undefined" && source instanceof Response
        ? await source.arrayBuffer()
        : source;
      return new Sirene(await WebAssembly.compile(bytes));
    },
  };
})(globalThis);
