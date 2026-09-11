/* ============================================================
   Soma & Souffle — chatbot (quikchat + SSE /api/chat)
   ============================================================ */
(function () {
  "use strict";

  var CHAT_ENDPOINT = "/api/chat";
  var MAX_HISTORY = 12; // nombre max de messages envoyés au LLM

  var qc = null;        // instance quikchat
  var busy = false;     // une requête de streaming en cours

  function t(key) { return window.SomaI18N.t(key); }

  function mdFormat(content) {
    // le build -md expose le parseur quikdown sur window.quikchat.quikdown
    if (window.quikchat && typeof window.quikchat.quikdown === "function") {
      try { return window.quikchat.quikdown(content); } catch (e) { /* fallback brut */ }
    }
    return content;
  }

  function initChat() {
    if (qc) return;

    qc = new window.quikchat(
      "#qc-chat",
      function (chat, msg) { handleSend(chat, msg); },
      {
        theme: "quikchat-theme-light",
        titleArea: { show: false },
        sanitize: true,
        messageFormatter: function (content) { return mdFormat(content); }
      }
    );

    // Message d'accueil intégré au fil (rôle "system" : exclu de l'historique LLM)
    try { qc.messageAddNew(t("chat.welcome"), "Soma & Souffle", "center", "system"); } catch (e) { /* noop */ }

    refreshLabels();
  }

  function refreshLabels() {
    if (!qc) return;
    var mount = document.getElementById("qc-chat");
    if (mount) {
      var input = mount.querySelector("textarea, input[type='text']");
      if (input) input.placeholder = t("chat.placeholder");
    }
    try { qc.inputAreaSetButtonText(t("chat.send")); } catch (e) { /* noop */ }
  }

  function setBusy(state) {
    busy = state;
    if (!qc) return;
    qc.inputAreaSetEnabled(!state);
  }

  function handleSend(chat, msg) {
    var text = (msg || "").trim();
    if (!text || busy) return;

    chat.messageAddNew(text, "me", "right", "user");

    setBusy(true);
    streamAssistant(text);
  }

  function streamAssistant(userText) {
    // Historique au format OpenAI/Ollama : [{role, content}]
    // Seuls les rôles user/assistant partent au LLM (le message d'accueil
    // "system" du fil reste côté interface).
    var history = [];
    try {
      history = (qc.historyGet() || [])
        .filter(function (m) { return m.role === "user" || m.role === "assistant"; })
        .map(function (m) {
          return { role: m.role, content: String(m.content || "") };
        });
    } catch (e) { history = []; }

    // On ne garde que les N derniers messages (le texte vient d'être ajouté)
    if (history.length > MAX_HISTORY) history = history.slice(-MAX_HISTORY);

    var msgId = qc.messageAddTypingIndicator("bot");

    fetch(CHAT_ENDPOINT, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        messages: history,
        lang: window.SomaI18N.lang
      })
    })
      .then(function (res) {
        if (!res.ok) throw new Error("HTTP " + res.status);
        if (!res.body || !res.body.getReader) {
          // Navigateur sans streaming : fallback texte
          return res.text().then(function (txt) { return { reader: null, text: txt }; });
        }
        return { reader: res.body.getReader(), text: null };
      })
      .then(function (handle) {
        if (!handle.reader) return handleSimpleText(msgId, handle.text);

        var decoder = new TextDecoder("utf-8");
        var buffer = "";
        var first = true;

        function read() {
          return handle.reader.read().then(function (chunk) {
            if (chunk.done) return finalize();
            buffer += decoder.decode(chunk.value, { stream: true });
            var events = buffer.split("\n\n");
            buffer = events.pop() || "";
            for (var i = 0; i < events.length; i++) {
              var lines = events[i].split("\n");
              for (var j = 0; j < lines.length; j++) {
                var line = lines[j];
                if (line.indexOf("data:") !== 0) continue;
                var payload;
                try { payload = JSON.parse(line.slice(5).trim()); } catch (e) { continue; }
                if (payload.error) return fail(payload.error);
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
      .catch(function (err) { fail(err && err.message ? err.message : "network"); });

    function finalize() { setBusy(false); }

    function fail(reason) {
      if (first) qc.messageReplaceContent(msgId, t("chat.error"));
      else qc.messageAppendContent(msgId, "\n\n*" + t("chat.error") + "*");
      setBusy(false);
    }
  }

  function handleSimpleText(msgId, text) {
    // Fallback : réponse non streamée (rare)
    var full = "";
    try {
      var parsed = JSON.parse(text);
      full = parsed && parsed.reply ? parsed.reply : text;
    } catch (e) { full = text; }
    qc.messageReplaceContent(msgId, full);
    setBusy(false);
  }

  // Langue : rafraîchir les libellés du chat
  document.addEventListener("soma:langchange", refreshLabels);

  // API exposée pour main.js (initialisation paresseuse à la première ouverture)
  window.SomaChat = {
    onOpen: initChat
  };
})();
