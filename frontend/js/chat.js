/* ============================================================
   Ventouses & Gua Sha — assistant chat (quikchat + SSE /api/chat)
   ============================================================ */
(function () {
  "use strict";

  var CHAT_ENDPOINT = "/api/chat";
  var MAX_HISTORY = 10;
  var WELCOME = "Bonjour 👋 Je suis l'assistant du cabinet. Posez-moi vos questions sur les séances : formules, prix, déroulé, marques, contre-indications, réservation…";
  var MSG_ERREUR = "Une erreur est survenue. Veuillez réessayer.";

  var qc = null;
  var busy = false;

  function mdFormat(content) {
    if (window.quikchat && typeof window.quikchat.quikdown === "function") {
      try { return window.quikchat.quikdown(content); } catch (e) { /* brut */ }
    }
    return content;
  }

  function initChat() {
    if (qc) return;
    qc = new window.quikchat(
      "#qc-chat",
      function (chat, msg) { handleSend(msg); },
      {
        theme: "quikchat-theme-light",
        titleArea: { show: false },
        sanitize: true,
        messageFormatter: function (content) { return mdFormat(content); }
      }
    );
    // Message d'accueil (rôle system : exclu de l'historique LLM)
    try { qc.messageAddNew(WELCOME, "Le cabinet", "center", "system"); } catch (e) { /* noop */ }
    var mount = document.getElementById("qc-chat");
    var input = mount && mount.querySelector("textarea, input[type='text']");
    if (input) input.placeholder = "Écrivez votre question…";
    try { qc.inputAreaSetButtonText("Envoyer"); } catch (e) { /* noop */ }
  }

  function setBusy(state) {
    busy = state;
    if (qc) qc.inputAreaSetEnabled(!state);
  }

  function handleSend(msg) {
    var text = (msg || "").trim();
    if (!text || busy) return;
    qc.messageAddNew(text, "moi", "right", "user");
    setBusy(true);
    streamAssistant(text);
  }

  function streamAssistant(userText) {
    var history = [];
    try {
      history = (qc.historyGet() || [])
        .filter(function (m) { return m.role === "user" || m.role === "assistant"; })
        .map(function (m) { return { role: m.role, content: String(m.content || "") }; });
    } catch (e) { history = []; }
    if (history.length > MAX_HISTORY) history = history.slice(-MAX_HISTORY);

    var msgId = qc.messageAddTypingIndicator("assistant");
    var first = true;

    function fail() {
      if (first) qc.messageReplaceContent(msgId, MSG_ERREUR);
      else qc.messageAppendContent(msgId, "\n\n*" + MSG_ERREUR + "*");
      setBusy(false);
    }

    fetch(CHAT_ENDPOINT, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ messages: history, lang: "fr" })
    })
      .then(function (res) {
        if (!res.ok) throw new Error("HTTP " + res.status);
        var reader = res.body && res.body.getReader ? res.body.getReader() : null;
        if (!reader) return res.text().then(function (txt) {
          var full = "";
          try { var p = JSON.parse(txt); full = p && p.reply ? p.reply : txt; }
          catch (e) { full = txt; }
          qc.messageReplaceContent(msgId, full);
          setBusy(false);
        });
        var decoder = new TextDecoder("utf-8");
        var buffer = "";
        function read() {
          return reader.read().then(function (chunk) {
            if (chunk.done) { setBusy(false); return; }
            buffer += decoder.decode(chunk.value, { stream: true });
            var events = buffer.split("\n\n");
            buffer = events.pop() || "";
            for (var i = 0; i < events.length; i++) {
              var lines = events[i].split("\n");
              for (var j = 0; j < lines.length; j++) {
                if (lines[j].indexOf("data:") !== 0) continue;
                var payload;
                try { payload = JSON.parse(lines[j].slice(5).trim()); } catch (e) { continue; }
                if (payload.error) { fail(); return; }
                if (payload.delta) {
                  if (first) { qc.messageReplaceContent(msgId, payload.delta); first = false; }
                  else qc.messageAppendContent(msgId, payload.delta);
                }
              }
            }
            return read();
          });
        }
        return read();
      })
      .catch(fail);
  }

  /* Ouverture / fermeture du widget */
  var widget = document.getElementById("chat-widget");
  var toggle = document.getElementById("chat-toggle");
  var closeBtn = document.getElementById("chat-close");
  var panel = document.getElementById("chat-panel");

  function setChatOpen(open) {
    widget.classList.toggle("open", open);
    panel.setAttribute("aria-hidden", open ? "false" : "true");
    toggle.setAttribute("aria-label", open ? "Fermer le chat" : "Ouvrir le chat");
    toggle.setAttribute("title", open ? "Fermer le chat" : "Ouvrir le chat");
    if (open) initChat();
  }

  if (toggle) toggle.addEventListener("click", function () {
    setChatOpen(!widget.classList.contains("open"));
  });
  if (closeBtn) closeBtn.addEventListener("click", function () { setChatOpen(false); });
  document.addEventListener("keydown", function (e) {
    if (e.key === "Escape" && widget.classList.contains("open")) setChatOpen(false);
  });
})();
