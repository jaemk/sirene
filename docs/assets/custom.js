// Add a link back to the landing page at the top of the sidebar. The landing page is the site
// root, one level up from the book (/guide/).
(function () {
  function addHomeLink() {
    var scroll = document.querySelector(".sidebar .sidebar-scrollbox");
    if (!scroll || scroll.querySelector(".sirene-home")) return;
    var a = document.createElement("a");
    a.className = "sirene-home";
    a.href = "../";
    a.textContent = "<- sirene home";
    scroll.insertBefore(a, scroll.firstChild);
  }
  // The drawing / `sirene ascii` tabs samples.py adds below each diagram block.
  function sampleTabs() {
    document.querySelectorAll(".sirene-sample").forEach(function (sample) {
      var views = sample.querySelectorAll(".sirene-view");
      views.forEach(function (view) {
        view.addEventListener("click", function () {
          views.forEach(function (v) {
            var on = v === view;
            v.classList.toggle("is-active", on);
            v.setAttribute("aria-selected", on ? "true" : "false");
          });
          sample.querySelectorAll(".sirene-pane").forEach(function (pane) {
            pane.hidden = pane.dataset.pane !== view.dataset.view;
          });
        });
      });
    });
  }
  // sirene-wasm, fetched on first use: sirene-wasm.js loads it, and pages.yml builds it into the
  // book as wasm/sirene.wasm.
  var loading = null;
  var ready = false;
  function sirene() {
    if (!loading) {
      var root = typeof path_to_root === "string" ? path_to_root : "";
      loading = fetch(root + "wasm/sirene.wasm")
        .then(function (r) {
          if (!r.ok) throw new Error("the in-browser renderer is not built into this copy of the docs");
          return SireneWasm.load(r);
        })
        .then(function (s) {
          ready = true;
          return s;
        });
      loading.catch(function () {
        loading = null;
      });
    }
    return loading;
  }

  // Shows a render's error or its first diagnostics in `status`, or hides it.
  function report(status, result) {
    var lines = result.ok ? [] : [result.error];
    result.diagnostics.slice(0, 3).forEach(function (d) {
      lines.push(d.severity + ": line " + d.line + ", column " + d.column + ": " + d.message);
    });
    if (result.diagnostics.length > 3) lines.push("and " + (result.diagnostics.length - 3) + " more");
    status.textContent = lines.join("\n");
    status.hidden = lines.length === 0;
    var error = !result.ok || result.diagnostics.some(function (d) {
      return d.severity === "error";
    });
    status.classList.toggle("is-error", error);
  }

  // Redraws `sample`'s drawing and ASCII panes from `editor` as it is typed in; `lang()` names
  // the source's language. Returns the function that redraws now.
  function live(sample, editor, status, lang, changed) {
    var img = sample.querySelector(".sirene-drawing img");
    var ascii = sample.querySelector(".sirene-ascii");
    var timer = null;
    var url = null;
    function draw() {
      timer = null;
      if (!ready) {
        status.textContent = "loading sirene...";
        status.hidden = false;
        status.classList.remove("is-error");
      }
      sirene().then(
        function (s) {
          var source = editor.value;
          var svg = s.render(source, { format: "svg", lang: lang(), theme: "dark" });
          var text = s.render(source, { format: "ascii", lang: lang() });
          if (svg.ok) {
            if (url) URL.revokeObjectURL(url);
            url = URL.createObjectURL(new Blob([svg.output], { type: "image/svg+xml" }));
            img.src = url;
          }
          if (text.ok) ascii.textContent = text.output.replace(/\n+$/, "");
          report(status, svg);
          if (changed) changed(source);
        },
        function (e) {
          status.textContent = e.message;
          status.hidden = false;
          status.classList.add("is-error");
        }
      );
    }
    editor.addEventListener("input", function () {
      if (timer) clearTimeout(timer);
      timer = setTimeout(draw, 150);
    });
    return draw;
  }

  function element(tag, className, text) {
    var e = document.createElement(tag);
    if (className) e.className = className;
    if (text) e.textContent = text;
    return e;
  }

  function editor(source) {
    var e = element("textarea", "sirene-source");
    e.spellcheck = false;
    e.value = source;
    e.rows = Math.min(30, Math.max(4, source.split("\n").length + 1));
    e.setAttribute("aria-label", "Diagram source");
    return e;
  }

  // An "edit" button on each sample: swaps the code block above it for an editor and redraws the
  // sample as the source changes. Once editing, the button resets the source.
  function editableSamples() {
    document.querySelectorAll(".sirene-sample[data-lang]").forEach(function (sample) {
      var pre = sample.previousElementSibling;
      if (!pre || pre.tagName !== "PRE") return;
      var original = pre.textContent.replace(/\n$/, "");
      var button = element("button", "sirene-view sirene-edit", "edit");
      button.type = "button";
      sample.querySelector(".sirene-sample-bar").appendChild(button);
      var input = null;
      var draw = null;
      button.addEventListener("click", function () {
        if (!input) {
          input = editor(original);
          var status = element("pre", "sirene-status");
          status.hidden = true;
          pre.hidden = true;
          pre.parentNode.insertBefore(input, pre.nextSibling);
          sample.querySelector(".sirene-sample-bar").after(status);
          draw = live(sample, input, status, function () {
            return sample.dataset.lang;
          });
          button.textContent = "reset";
          input.focus();
        } else {
          input.value = original;
        }
        draw();
      });
    });
  }

  // URL-safe base64 of UTF-8, for the playground's shareable link.
  function encode(text) {
    var bytes = new TextEncoder().encode(text);
    var s = "";
    for (var i = 0; i < bytes.length; i++) s += String.fromCharCode(bytes[i]);
    return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  }
  function decode(text) {
    var s = atob(text.replace(/-/g, "+").replace(/_/g, "/"));
    var bytes = new Uint8Array(s.length);
    for (var i = 0; i < s.length; i++) bytes[i] = s.charCodeAt(i);
    return new TextDecoder().decode(bytes);
  }

  var PLAYGROUND_SOURCE = [
    "flowchart LR",
    "  edit[Edit the source] --> draw{{sirene draws it}}",
    "  draw --> svg([SVG])",
    "  draw --> ascii([sirene ascii])",
  ].join("\n");

  var LANGS = [
    ["mermaid", "Mermaid"],
    ["plantuml", "PlantUML"],
    ["dot", "Graphviz DOT"],
  ];

  // The playground page's editor: the source and its language are kept in the URL's fragment
  // (#lang:base64), so a link to the page opens the same diagram.
  function playground() {
    var mount = document.getElementById("sirene-playground");
    if (!mount) return;
    var source = PLAYGROUND_SOURCE;
    var lang = "mermaid";
    var hash = location.hash.match(/^#(mermaid|plantuml|dot):([A-Za-z0-9_-]*)$/);
    if (hash) {
      try {
        source = decode(hash[2]);
        lang = hash[1];
      } catch (e) {
        // A damaged link opens the default diagram.
      }
    }
    mount.textContent = "";
    var select = element("select", "sirene-lang");
    select.setAttribute("aria-label", "Language");
    LANGS.forEach(function (l) {
      var o = element("option", null, l[1]);
      o.value = l[0];
      select.appendChild(o);
    });
    select.value = lang;
    var input = editor(source);
    input.rows = 14;
    var sample = element("div", "sirene-sample");
    var bar = element("div", "sirene-sample-bar");
    bar.setAttribute("role", "tablist");
    bar.setAttribute("aria-label", "Output");
    [["drawing", "drawing"], ["ascii", "sirene ascii"]].forEach(function (v, i) {
      var b = element("button", "sirene-view" + (i === 0 ? " is-active" : ""), v[1]);
      b.type = "button";
      b.setAttribute("role", "tab");
      b.setAttribute("aria-selected", i === 0 ? "true" : "false");
      b.dataset.view = v[0];
      bar.appendChild(b);
    });
    bar.appendChild(select);
    var status = element("pre", "sirene-status");
    status.hidden = true;
    var drawing = element("div", "sirene-pane sirene-drawing");
    drawing.dataset.pane = "drawing";
    var img = element("img");
    img.alt = "the drawing of the source above";
    drawing.appendChild(img);
    var ascii = element("pre", "sirene-pane sirene-ascii");
    ascii.dataset.pane = "ascii";
    ascii.hidden = true;
    sample.append(bar, status, drawing, ascii);
    mount.append(input, sample);
    var draw = live(
      sample,
      input,
      status,
      function () {
        return select.value;
      },
      function (src) {
        history.replaceState(null, "", "#" + select.value + ":" + encode(src));
      }
    );
    select.addEventListener("change", draw);
    draw();
  }

  function init() {
    addHomeLink();
    playground();
    sampleTabs();
    editableSamples();
  }
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
