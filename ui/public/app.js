const endpointStoreKey = "logosInspectorEndpoints";

const defaults = {
  sequencer: "https://testnet.lez.logos.co/",
  indexer: "http://127.0.0.1:8779/",
  node: "http://127.0.0.1:8080/"
};

const state = {
  endpoints: loadStoredJson(endpointStoreKey, defaults),
  activeView: "overview"
};

const $ = (selector, root = document) => root.querySelector(selector);
const $$ = (selector, root = document) => [...root.querySelectorAll(selector)];

function loadStoredJson(key, fallback) {
  try {
    return { ...fallback, ...(JSON.parse(localStorage.getItem(key) || "{}") || {}) };
  } catch {
    return fallback;
  }
}

function saveEndpoints() {
  localStorage.setItem(endpointStoreKey, JSON.stringify(state.endpoints));
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function toast(message) {
  const el = $("#toast");
  el.textContent = message;
  el.classList.add("show");
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => el.classList.remove("show"), 2400);
}

function setBusy(button, busy) {
  if (!button) return;
  button.disabled = busy;
  button.dataset.originalText ||= button.textContent;
  button.textContent = busy ? "Working" : button.dataset.originalText;
}

async function post(path, body = {}) {
  const response = await fetch(path, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body)
  });
  const json = await response.json();
  if (!response.ok || json.ok === false) {
    const error = new Error(json.error || `HTTP ${response.status}`);
    error.payload = json;
    throw error;
  }
  return json;
}

async function getJson(path) {
  const response = await fetch(path);
  const json = await response.json();
  if (!response.ok || json.ok === false) throw new Error(json.error || `HTTP ${response.status}`);
  return json;
}

function endpointArgs() {
  return [
    "--sequencer-url",
    state.endpoints.sequencer,
    "--indexer-url",
    state.endpoints.indexer,
    "--node-url",
    state.endpoints.node
  ];
}

async function cli(command, args = []) {
  const result = await post("/api/cli", { command, args });
  return result.json ?? result;
}

function valueKind(value) {
  if (value === null || value === undefined) return "empty";
  if (Array.isArray(value)) return `${value.length} items`;
  if (typeof value === "object") return `${Object.keys(value).length} fields`;
  return typeof value;
}

function valueText(value) {
  if (value === null || value === undefined || value === "") return "-";
  if (typeof value === "boolean") return value ? "yes" : "no";
  if (Array.isArray(value)) return `${value.length} items`;
  if (typeof value === "object") return `${Object.keys(value).length} fields`;
  return String(value);
}

function formatLabel(value) {
  return String(value || "")
    .replaceAll("_", " ")
    .replaceAll("-", " ")
    .replace(/\b\w/g, (ch) => ch.toUpperCase());
}

function renderKv(values = {}) {
  const entries = Object.entries(values).filter(([, value]) => value !== undefined);
  if (!entries.length) return `<div class="empty">No values</div>`;
  return `<dl>${entries.map(([key, value]) => `
    <div class="kv">
      <dt>${escapeHtml(key)}</dt>
      <dd class="${String(valueText(value)).length > 52 ? "hash" : ""}">${escapeHtml(valueText(value))}</dd>
    </div>
  `).join("")}</dl>`;
}

function renderJson(value) {
  return `<pre class="code-block">${escapeHtml(JSON.stringify(value, null, 2))}</pre>`;
}

function renderStructured(value) {
  if (value === null || value === undefined) return `<div class="empty">No result</div>`;
  if (["string", "number", "boolean"].includes(typeof value)) {
    return renderKv({ value });
  }
  if (Array.isArray(value)) {
    if (!value.length) return `<div class="empty">No items</div>`;
    return `<div class="item-list">${value.map((item, index) => `
      <div class="item">
        <div class="item-top">
          <strong>Item ${index + 1}</strong>
          <span class="pill">${escapeHtml(valueKind(item))}</span>
        </div>
        ${renderStructured(item)}
      </div>
    `).join("")}</div>`;
  }

  const scalars = {};
  const nested = [];
  for (const [key, item] of Object.entries(value)) {
    if (item === null || ["string", "number", "boolean"].includes(typeof item)) {
      scalars[formatLabel(key)] = item;
    } else {
      nested.push([key, item]);
    }
  }

  return `
    ${Object.keys(scalars).length ? renderKv(scalars) : ""}
    ${nested.map(([key, item]) => `
      <details class="item" open>
        <summary class="item-top">
          <strong>${escapeHtml(formatLabel(key))}</strong>
          <span class="pill">${escapeHtml(valueKind(item))}</span>
        </summary>
        ${renderStructured(item)}
      </details>
    `).join("")}
  `;
}

function renderResult(target, value) {
  target.innerHTML = `
    <div class="result-actions">
      <button class="secondary copy-result" type="button">Copy JSON</button>
    </div>
    ${renderStructured(value)}
    <details>
      <summary class="item-top"><strong>Raw JSON</strong><span class="pill">${escapeHtml(valueKind(value))}</span></summary>
      ${renderJson(value)}
    </details>
  `;
  $(".copy-result", target)?.addEventListener("click", () => {
    navigator.clipboard.writeText(JSON.stringify(value, null, 2));
    toast("Copied");
  });
}

function renderError(target, error) {
  const detail = error.payload ? `\n${JSON.stringify(error.payload, null, 2)}` : "";
  target.innerHTML = `<div class="error">${escapeHtml(error.message || error)}${escapeHtml(detail)}</div>`;
}

async function runInto(button, target, command, args = []) {
  setBusy(button, true);
  target.innerHTML = `<div class="loading">Loading</div>`;
  try {
    const result = await cli(command, args);
    renderResult(target, result);
    return result;
  } catch (error) {
    renderError(target, error);
    return null;
  } finally {
    setBusy(button, false);
  }
}

function probeValue(report, service, field) {
  const probe = report?.[service]?.[field];
  if (!probe) return "-";
  if (!probe.ok) return "error";
  if (probe.value === undefined || probe.value === null) return "ok";
  return valueText(probe.value);
}

function moduleCount(report) {
  const modules = report?.status?.value?.value?.modules;
  if (!Array.isArray(modules)) return "-";
  return String(modules.length);
}

function setMetricsFromOverview(report) {
  $("#metricSequencerHead").textContent = probeValue(report, "sequencer", "head");
  $("#metricIndexerHead").textContent = probeValue(report, "indexer", "head");
  $("#metricCryptarchia").textContent = probeValue(report, "node", "consensus");
}

function setMetricsFromModules(report) {
  $("#metricModules").textContent = moduleCount(report);
}

function applyEndpointInputs() {
  $("#sequencerInput").value = state.endpoints.sequencer;
  $("#indexerInput").value = state.endpoints.indexer;
  $("#nodeInput").value = state.endpoints.node;
}

function selectView(view) {
  state.activeView = view;
  $$(".nav-item").forEach((button) => {
    button.classList.toggle("is-active", button.dataset.view === view);
  });
  $$(".view").forEach((section) => {
    section.classList.toggle("is-active", section.id === `view-${view}`);
  });
  $("#viewTitle").textContent = formatLabel(view === "lez" ? "LEZ" : view);
}

async function refreshOverview() {
  const overviewButton = $("#overviewBtn");
  const modulesButton = $("#modulesBtn");
  const overview = await runInto(overviewButton, $("#overviewResult"), "overview", endpointArgs());
  if (overview) setMetricsFromOverview(overview);
  const modules = await runInto(modulesButton, $("#modulesResult"), "modules", []);
  if (modules) setMetricsFromModules(modules);
}

function nonEmptyArgs(flag, value) {
  const raw = String(value || "").trim();
  return raw ? [flag, raw] : [];
}

function parseArgsTextarea(value) {
  const parsed = JSON.parse(value || "[]");
  if (!Array.isArray(parsed)) throw new Error("args must be a JSON array");
  return parsed.map(String);
}

function wire() {
  applyEndpointInputs();

  $$(".nav-item").forEach((button) => {
    button.addEventListener("click", () => selectView(button.dataset.view));
  });

  $("#endpointForm").addEventListener("submit", (event) => {
    event.preventDefault();
    state.endpoints = {
      sequencer: $("#sequencerInput").value.trim() || defaults.sequencer,
      indexer: $("#indexerInput").value.trim() || defaults.indexer,
      node: $("#nodeInput").value.trim() || defaults.node
    };
    saveEndpoints();
    toast("Endpoints applied");
  });

  $("#refreshBtn").addEventListener("click", refreshOverview);
  $("#overviewBtn").addEventListener("click", refreshOverview);
  $("#modulesBtn").addEventListener("click", async (event) => {
    const result = await runInto(event.currentTarget, $("#modulesResult"), "modules", []);
    if (result) setMetricsFromModules(result);
  });

  $("#buildBtn").addEventListener("click", async (event) => {
    setBusy(event.currentTarget, true);
    try {
      const result = await post("/api/build");
      toast(result.ok ? "Build complete" : "Build failed");
      await updateCliStatus();
    } catch (error) {
      toast(error.message);
    } finally {
      setBusy(event.currentTarget, false);
    }
  });

  $("#blockchainNodeBtn").addEventListener("click", (event) => {
    runInto(event.currentTarget, $("#blockchainNodeResult"), "blockchain-node", endpointArgs());
  });
  $("#blockchainBlocksForm").addEventListener("submit", (event) => {
    event.preventDefault();
    runInto($("button", event.currentTarget), $("#blockchainBlocksResult"), "blockchain-blocks", [
      "--slot-from",
      $("#blockchainSlotFrom").value,
      "--slot-to",
      $("#blockchainSlotTo").value,
      ...endpointArgs()
    ]);
  });

  $("#channelForm").addEventListener("submit", (event) => {
    event.preventDefault();
    runInto($("button", event.currentTarget), $("#channelResult"), "channels", [
      "--slot-from",
      $("#channelSlotFrom").value,
      "--slot-to",
      $("#channelSlotTo").value,
      ...endpointArgs()
    ]);
  });

  $("#storageBtn").addEventListener("click", (event) => {
    runInto(event.currentTarget, $("#storageResult"), "storage", nonEmptyArgs("--cid", $("#storageCid").value));
  });
  $("#storageForm").addEventListener("submit", (event) => {
    event.preventDefault();
    runInto($("button", event.currentTarget), $("#storageResult"), "storage", nonEmptyArgs("--cid", $("#storageCid").value));
  });

  $("#messagingBtn").addEventListener("click", (event) => {
    runInto(event.currentTarget, $("#messagingResult"), "messaging", nonEmptyArgs("--info-id", $("#messagingInfoId").value));
  });
  $("#messagingForm").addEventListener("submit", (event) => {
    event.preventDefault();
    runInto($("button", event.currentTarget), $("#messagingResult"), "messaging", nonEmptyArgs("--info-id", $("#messagingInfoId").value));
  });

  $("#capabilitiesBtn").addEventListener("click", (event) => {
    runInto(event.currentTarget, $("#capabilitiesResult"), "capabilities", []);
  });
  $("#allModulesBtn").addEventListener("click", async (event) => {
    const result = await runInto(event.currentTarget, $("#capabilitiesResult"), "modules", []);
    if (result) setMetricsFromModules(result);
  });

  $("#lezHeadBtn").addEventListener("click", (event) => {
    runInto(event.currentTarget, $("#lezResult"), "head", ["--sequencer-url", state.endpoints.sequencer]);
  });
  $("#lezProgramsBtn").addEventListener("click", (event) => {
    runInto(event.currentTarget, $("#lezResult"), "programs", ["--sequencer-url", state.endpoints.sequencer]);
  });
  $("#lezBlockForm").addEventListener("submit", (event) => {
    event.preventDefault();
    runInto($("button", event.currentTarget), $("#lezResult"), "block", [
      $("#lezBlockId").value,
      "--sequencer-url",
      state.endpoints.sequencer
    ]);
  });
  $("#lezTxForm").addEventListener("submit", (event) => {
    event.preventDefault();
    runInto($("button", event.currentTarget), $("#lezResult"), "inspect-tx", [
      $("#lezTxHash").value,
      "--sequencer-url",
      state.endpoints.sequencer
    ]);
  });
  $("#lezAccountForm").addEventListener("submit", (event) => {
    event.preventDefault();
    runInto($("button", event.currentTarget), $("#lezResult"), "account", [
      $("#lezAccountId").value,
      ...endpointArgs()
    ]);
  });
  $("#indexerRpcForm").addEventListener("submit", (event) => {
    event.preventDefault();
    runInto($("button", event.currentTarget), $("#indexerResult"), "rpc", [
      state.endpoints.indexer,
      $("#indexerMethod").value,
      $("#indexerParams").value
    ]);
  });

  $("#spelBtn").addEventListener("click", async (event) => {
    const target = $("#spelResult");
    setBusy(event.currentTarget, true);
    target.innerHTML = `<div class="loading">Loading</div>`;
    try {
      const result = await post("/api/spel-idl", { idlJson: $("#spelIdlJson").value });
      renderResult(target, result.json ?? result);
    } catch (error) {
      renderError(target, error);
    } finally {
      setBusy(event.currentTarget, false);
    }
  });

  $("#rawCliForm").addEventListener("submit", (event) => {
    event.preventDefault();
    try {
      const args = parseArgsTextarea($("#rawArgs").value);
      runInto($("button", event.currentTarget), $("#rawResult"), $("#rawCommand").value.trim(), args);
    } catch (error) {
      renderError($("#rawResult"), error);
    }
  });
}

async function updateCliStatus() {
  try {
    const status = await getJson("/api/status");
    $("#cliState").textContent = status.built ? "ready" : "not built";
    $("#metricModules").textContent = status.built ? $("#metricModules").textContent : "-";
  } catch {
    $("#cliState").textContent = "error";
  }
}

wire();
updateCliStatus();
refreshOverview().catch(() => {});
