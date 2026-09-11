/* ============================================================
   Soma & Souffle — interactions générales de la page
   ============================================================ */
(function () {
  "use strict";

  // Année du copyright
  var yearEl = document.getElementById("year");
  if (yearEl) yearEl.textContent = String(new Date().getFullYear());

  // Toggle de langue
  document.querySelectorAll(".lang-toggle button").forEach(function (btn) {
    btn.addEventListener("click", function () {
      window.SomaI18N.setLang(btn.getAttribute("data-lang"));
    });
  });

  // Ouverture / fermeture du chat
  var widget = document.getElementById("chat-widget");
  var toggle = document.getElementById("chat-toggle");
  var closeBtn = document.getElementById("chat-close");
  var panel = document.getElementById("chat-panel");

  function setChatOpen(open) {
    widget.classList.toggle("open", open);
    panel.setAttribute("aria-hidden", open ? "false" : "true");
    toggle.setAttribute(
      "aria-label",
      open ? window.SomaI18N.t("chat.close") : window.SomaI18N.t("chat.open")
    );
    if (open && window.SomaChat && window.SomaChat.onOpen) {
      window.SomaChat.onOpen();
    }
  }

  if (toggle) toggle.addEventListener("click", function () { setChatOpen(!widget.classList.contains("open")); });
  if (closeBtn) closeBtn.addEventListener("click", function () { setChatOpen(false); });
  document.addEventListener("keydown", function (e) {
    if (e.key === "Escape" && widget.classList.contains("open")) setChatOpen(false);
  });
})();
