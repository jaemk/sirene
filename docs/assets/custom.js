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
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", addHomeLink);
  } else {
    addHomeLink();
  }
})();
