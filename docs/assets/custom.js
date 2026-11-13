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
  function init() {
    addHomeLink();
    sampleTabs();
  }
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
