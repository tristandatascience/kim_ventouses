/* ============================================================
   Lumi — guide du cabinet, branché sur /api/chat (SSE + RAG)
   ============================================================ */
(function () {
  "use strict";

  var dialog = document.getElementById("concierge-dialog");
  var form = document.getElementById("concierge-form");
  var input = document.getElementById("concierge-input");
  var envoi = dialog ? dialog.querySelector(".chat-send") : null;
  var log = document.getElementById("concierge-log");
  var reset = document.getElementById("concierge-reset");
  var boiteSugg = document.getElementById("concierge-suggestions");
  var statut = document.getElementById("concierge-status");

  var SUGGESTIONS = [
    "Quelle formule choisir ?",
    "Combien de temps durent les marques ?",
    "Est-ce que ça fait mal ?",
    "Comment réserver une séance ?"
  ];
  var ACCUEIL = "Bonjour ✧ Je suis l'assistant du cabinet. Posez-moi vos " +
    "questions sur les séances : formules, prix, déroulé, marques, " +
    "contre-indications…";

  if (!dialog || !form || !input || !log) return;

  function message(classe, texte) {
    var d = document.createElement("div");
    d.className = "chat-message" + (classe ? " " + classe : "");
    var p = document.createElement("p");
    p.textContent = texte;
    d.appendChild(p);
    log.appendChild(d);
    log.scrollTop = log.scrollHeight;
    return d;
  }

  function suggestions() {
    if (!boiteSugg) return;
    boiteSugg.innerHTML = "";
    SUGGESTIONS.forEach(function (s) {
      var b = document.createElement("button");
      b.type = "button";
      b.textContent = s;
      b.addEventListener("click", function () {
        input.value = s;
        if (form.requestSubmit) form.requestSubmit();
        else form.dispatchEvent(new Event("submit", { cancelable: true }));
      });
      boiteSugg.appendChild(b);
    });
  }

  function demarrer() {
    log.innerHTML = "";
    message("accueil", ACCUEIL);
    suggestions();
  }

  /* Historique : les messages visibles, sans l'accueil ni l'indicateur */
  function historique() {
    var msgs = [];
    log.querySelectorAll(".chat-message").forEach(function (m) {
      if (m.classList.contains("pending") || m.classList.contains("accueil")) return;
      msgs.push({
        role: m.classList.contains("user") ? "user" : "assistant",
        content: m.textContent
      });
    });
    return msgs.slice(-10);
  }

  var occupe = false;
  form.addEventListener("submit", function (e) {
    e.preventDefault();
    var texte = input.value.trim();
    if (!texte || occupe) return;

    message("user", texte);
    input.value = "";
    occupe = true;
    if (envoi) envoi.disabled = true;
    if (statut) statut.textContent = "Réflexion…";

    var attente = message("pending", "…");
    var premier = true;

    function maj(t) {
      if (premier) {
        attente.classList.remove("pending");
        attente.querySelector("p").textContent = t;
        premier = false;
      } else {
        attente.querySelector("p").textContent += t;
      }
      log.scrollTop = log.scrollHeight;
    }

    function fin(erreur) {
      if (erreur && premier) maj("Une erreur est survenue. Réessayez dans un instant.");
      occupe = false;
      if (envoi) envoi.disabled = false;
      input.focus();
      if (statut) statut.textContent = "Assistant · répond en direct";
    }

    fetch("/api/chat", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ messages: historique(), lang: "fr" })
    })
      .then(function (res) {
        if (!res.ok) throw new Error("HTTP " + res.status);
        var reader = res.body && res.body.getReader ? res.body.getReader() : null;
        if (!reader) {
          return res.text().then(function (t) {
            var d = "";
            try { d = JSON.parse(t).reply || t; } catch (e) { d = t; }
            maj(d);
          });
        }
        var dec = new TextDecoder();
        var buf = "";
        function lire() {
          return reader.read().then(function (c) {
            if (c.done) { fin(false); return; }
            buf += dec.decode(c.value, { stream: true });
            var evts = buf.split("\n\n");
            buf = evts.pop() || "";
            evts.forEach(function (ev) {
              ev.split("\n").forEach(function (ligne) {
                if (ligne.indexOf("data:") !== 0) return;
                var p;
                try { p = JSON.parse(ligne.slice(5).trim()); } catch (e) { return; }
                if (p.delta) maj(p.delta);
                else if (p.error) maj("\n[erreur] " + p.error);
              });
            });
            return lire();
          });
        }
        return lire();
      })
      .catch(function () { fin(true); });
  });

  if (reset) reset.addEventListener("click", demarrer);

  /* Boutons « Demandez à l'assistant » dispersés dans la page */
  document.querySelectorAll("[data-open-lumi]").forEach(function (b) {
    b.addEventListener("click", function () {
      if (typeof dialog.showModal === "function") {
        dialog.showModal();
        document.body.classList.add("chat-open");
        input.focus();
      }
    });
  });

  demarrer();
})();
