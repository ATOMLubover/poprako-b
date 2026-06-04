const state = {
  sessions: [],
  checkpoints: [],
  activeSessionId: null,
  // Each exchange: { userContent, assistantContent (null = loading), afterCpId }
  exchanges: [],
  sending: false,
};

const els = {
  sessions: document.getElementById("session-list"),
  checkpoints: document.getElementById("checkpoint-list"),
  messages: document.getElementById("messages"),
  form: document.getElementById("message-form"),
  input: document.getElementById("message-input"),
  newSession: document.getElementById("new-session"),
  refresh: document.getElementById("refresh"),
  activeSession: document.getElementById("active-session"),
  persistDebug: document.getElementById("persist-debug"),
  contextCount: document.getElementById("context-count"),
  contextView: document.getElementById("context-view"),
};

async function api(path, options = {}) {
  const response = await fetch(path, {
    headers: { "Content-Type": "application/json" },
    ...options,
  });
  const data = await response.json().catch(() => ({}));
  if (!response.ok) {
    throw new Error(data.details || data.error || response.statusText);
  }
  return data;
}

function shortId(id) {
  return id ? id.slice(0, 8) : "none";
}

function truncateTitle(text) {
  if (text.length <= 32) return text;
  const cut = text.slice(0, 32);
  const lastSpace = cut.lastIndexOf(" ");
  return (lastSpace > 12 ? cut.slice(0, lastSpace) : cut) + "\u2026";
}

// Build exchanges from checkpoint list + context messages.
// The last solutionPairs.length exchanges map to before/after checkpoint pairs.
// Exchanges before that (inherited from a fork) have no fork buttons.
function buildExchanges(checkpoints, messages) {
  const solutionPairs = [];
  const pending = {};
  for (const item of checkpoints) {
    const cp = item.checkpoint;
    if (!cp.solution_id) continue;
    if (cp.kind === "before_solution") {
      pending[cp.solution_id] = cp.id;
    } else if (cp.kind === "after_solution") {
      solutionPairs.push({ afterCpId: cp.id });
      delete pending[cp.solution_id];
    }
  }

  const nonSystem = messages.filter((m) => m.role !== "system");
  const pairs = [];
  for (let i = 0; i < nonSystem.length; i += 2) {
    pairs.push([nonSystem[i], nonSystem[i + 1]]);
  }

  const offset = pairs.length - solutionPairs.length;
  return pairs.map((pair, i) => {
    const cpPair = i >= offset ? solutionPairs[i - offset] : null;
    const a = pair[1];
    return {
      userContent: pair[0]?.content || "",
      assistantContent: a ? (a.content || JSON.stringify(a.tool_calls || a)) : null,
      afterCpId: cpPair?.afterCpId || null,
    };
  });
}

function renderSessions() {
  els.sessions.innerHTML = "";
  for (const item of state.sessions) {
    const btn = document.createElement("button");
    btn.className = "session" + (item.session.id === state.activeSessionId ? " active" : "");
    btn.type = "button";
    btn.innerHTML = '<span class="title">' + (item.session.name || shortId(item.session.id)) + '</span>';
    btn.addEventListener("click", () => selectSession(item.session.id));
    els.sessions.appendChild(btn);
  }
}

function renderMessages() {
  els.messages.innerHTML = "";
  for (const exchange of state.exchanges) {
    const el = document.createElement("div");
    el.className = "exchange";

    const userEl = document.createElement("div");
    userEl.className = "bubble user";
    userEl.textContent = exchange.userContent;
    el.appendChild(userEl);

    if (exchange.assistantContent === null) {
      const thinking = document.createElement("div");
      thinking.className = "bubble assistant thinking";
      thinking.innerHTML = '<span class="dot"></span><span class="dot"></span><span class="dot"></span>';
      el.appendChild(thinking);
    } else {
      const asstEl = document.createElement("div");
      asstEl.className = "bubble assistant";
      asstEl.innerHTML = marked.parse(exchange.assistantContent);
      el.appendChild(asstEl);

      if (exchange.afterCpId) {
        const toolbar = document.createElement("div");
        toolbar.className = "fork-toolbar";

        const forkBtn = document.createElement("button");
        forkBtn.className = "fork-btn";
        forkBtn.innerHTML = '<svg viewBox="0 0 16 16" width="13" height="13" stroke="currentColor" stroke-width="1.6" fill="none" stroke-linecap="round" stroke-linejoin="round"><circle cx="8" cy="13" r="2"></circle><circle cx="3" cy="3" r="2"></circle><circle cx="13" cy="3" r="2"></circle><path d="M13 5v2c0 1.1-.9 2-2 2H5c-1.1 0-2-.9-2-2V5"></path><path d="M8 11V9"></path></svg>';
        forkBtn.title = "Fork from here";
        forkBtn.addEventListener("click", () => forkCheckpoint(exchange.afterCpId));
        toolbar.appendChild(forkBtn);

        el.appendChild(toolbar);
      }
    }

    els.messages.appendChild(el);
  }
  els.messages.scrollTop = els.messages.scrollHeight;
}

function setInputLocked(locked) {
  state.sending = locked;
  els.input.disabled = locked;
  els.form.querySelector('button[type=submit]').disabled = locked;
}

async function loadSessions() {
  state.sessions = await api("/api/sessions");
  renderSessions();
}

async function loadSession(sessionId) {
  if (!sessionId) {
    state.exchanges = [];
    state.checkpoints = [];
    renderMessages();
    return;
  }
  const data = await api("/api/sessions/" + sessionId + "/checkpoints");
  state.checkpoints = data.checkpoints;

  const head = [...state.checkpoints]
    .reverse()
    .find((c) => c.checkpoint.kind !== "before_solution");

  if (!head || head.message_count === 0) {
    state.exchanges = [];
    renderMessages();
    return;
  }

  const ctx = await api("/api/checkpoints/" + head.checkpoint.id + "/context");
  state.exchanges = buildExchanges(state.checkpoints, ctx.messages);
  renderMessages();
}

async function selectSession(sessionId) {
  state.activeSessionId = sessionId;
  renderSessions();
  await loadSession(sessionId);
}

async function createSession(name) {
  const data = await api("/api/sessions", {
    method: "POST",
    body: JSON.stringify({ name }),
  });
  state.activeSessionId = data.session.id;
  state.exchanges = [];
  await loadSessions();
  return data;
}

async function sendMessage(event) {
  event.preventDefault();
  if (state.sending) return;
  const content = els.input.value.trim();
  if (!content) return;
  els.input.value = "";

  if (!state.activeSessionId) {
    await createSession(truncateTitle(content));
  }

  setInputLocked(true);
  state.exchanges.push({
    userContent: content,
    assistantContent: null,
    afterCpId: null,
  });
  renderMessages();

  try {
    const data = await api("/api/sessions/" + state.activeSessionId + "/messages", {
      method: "POST",
      body: JSON.stringify({ content }),
    });
    const last = state.exchanges[state.exchanges.length - 1];
    last.assistantContent = data.assistant;
    last.afterCpId = data.after_checkpoint.checkpoint.id;
    renderMessages();

    const cpData = await api("/api/sessions/" + state.activeSessionId + "/checkpoints");
    state.checkpoints = cpData.checkpoints;
    await loadSessions();
  } catch (err) {
    const last = state.exchanges[state.exchanges.length - 1];
    last.assistantContent = "Error: " + err.message;
    renderMessages();
  } finally {
    setInputLocked(false);
  }
}

async function forkCheckpoint(checkpointId) {
  const label = "fork \u00B7 " + shortId(checkpointId);
  const data = await api("/api/checkpoints/" + checkpointId + "/fork", {
    method: "POST",
    body: JSON.stringify({ name: label }),
  });
  state.activeSessionId = data.session.id;
  await loadSessions();
  await loadSession(data.session.id);
}

async function refreshAll() {
  await loadSessions();
  if (state.activeSessionId) {
    await loadSession(state.activeSessionId);
  }
}

els.newSession.addEventListener("click", () => {
  state.activeSessionId = null;
  state.exchanges = [];
  renderSessions();
  renderMessages();
});

els.refresh.addEventListener("click", refreshAll);
els.form.addEventListener("submit", sendMessage);

els.input.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    els.form.dispatchEvent(new Event("submit"));
  }
});

loadSessions()
  .then(async () => {
    if (state.sessions.length > 0) {
      state.activeSessionId = state.sessions[0].session.id;
      renderSessions();
      await loadSession(state.activeSessionId);
    }
  })
  .catch((err) => console.error("Init error:", err));
