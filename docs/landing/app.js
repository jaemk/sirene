// Landing page wiring: the hero's source and ASCII text, the tab groups, and copy buttons.
// hero.svg and hero.txt are rendered from hero.mmd by sirene when the site is built.
(function () {
  function load(url, el) {
    fetch(url)
      .then(function (r) { return r.ok ? r.text() : Promise.reject(r.status); })
      .then(function (text) { el.textContent = text; })
      .catch(function () {});
  }

  load("hero.mmd", document.querySelector("#source code"));
  load("hero.txt", document.getElementById("ascii"));

  // A tab group: buttons with `data-<key>` select the element with the same `data-<target>`.
  function tabs(buttonSel, key, targetSel, target) {
    var buttons = document.querySelectorAll(buttonSel);
    buttons.forEach(function (button) {
      button.addEventListener("click", function () {
        var name = button.dataset[key];
        buttons.forEach(function (b) {
          var on = b === button;
          b.classList.toggle("is-active", on);
          b.setAttribute("aria-selected", on ? "true" : "false");
        });
        document.querySelectorAll(targetSel).forEach(function (p) {
          var on = p.dataset[target] === name;
          p.classList.toggle("is-active", on);
          p.hidden = !on;
        });
      });
    });
  }
  tabs(".tab", "tab", ".panel", "panel");
  tabs(".view", "view", ".pane", "pane");

  document.querySelectorAll("button.copy").forEach(function (button) {
    button.addEventListener("click", function () {
      if (!navigator.clipboard) return;
      navigator.clipboard.writeText(button.dataset.copy).then(function () {
        button.textContent = "Copied";
        setTimeout(function () { button.textContent = "Copy"; }, 1400);
      });
    });
  });
})();
