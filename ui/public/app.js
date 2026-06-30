const endpointStoreKey = "lezInspectEndpoints";
const idlStoreKey = "lezInspectIdls";

const storedEndpoints = loadStoredJson(endpointStoreKey, {});

const state = {
  endpoint: storedEndpoints.endpoint || "https://testnet.lez.logos.co/",
  indexerEndpoint: storedEndpoints.indexerEndpoint || "http://127.0.0.1:8779/",
  currentBlock: null,
  latestHead: null,
  indexerHead: null,
  lastRange: [],
  idls: loadStoredJson(idlStoreKey, {})
};

const commands = [
  ["fetch-tx", "<tx_hash> [sequencer_endpoint]"],
  ["find-tx-block", "<tx_hash> <start> <end> [sequencer_endpoint]"],
  ["account-json", "<account_id> [sequencer_endpoint]"],
  ["account-data-hex", "<account_id> [sequencer_endpoint]"],
  ["program-id", "<program.bin>"],
  ["hash-deploy", "<program.bin>"],
  ["create-public-account", ""],
  ["strip-r0bf", "<input.bin> <output.bin> [strip-bin]"],
  ["decode-block", "<block-json-file>"],
  ["decode-block-range", "<range-json-file>"]
];

const base58Alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const textDecoder = new TextDecoder();

const $ = (selector, root = document) => root.querySelector(selector);
const $$ = (selector, root = document) => [...root.querySelectorAll(selector)];

function loadStoredJson(key, fallback) {
  try {
    return JSON.parse(localStorage.getItem(key) || "") || fallback;
  } catch {
    return fallback;
  }
}

function saveEndpoints() {
  localStorage.setItem(endpointStoreKey, JSON.stringify({
    endpoint: state.endpoint,
    indexerEndpoint: state.indexerEndpoint
  }));
}

function saveIdls() {
  localStorage.setItem(idlStoreKey, JSON.stringify(state.idls));
}

function endpoint() {
  return $("#endpointInput").value.trim() || state.endpoint;
}

function indexerEndpoint() {
  return $("#indexerEndpointInput").value.trim() || state.indexerEndpoint;
}

function setBusy(element, busy) {
  if (!element) return;
  element.disabled = busy;
  element.dataset.originalText ||= element.textContent;
  element.textContent = busy ? "Working" : element.dataset.originalText;
}

function toast(message) {
  const el = $("#toast");
  el.textContent = message;
  el.classList.add("show");
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => el.classList.remove("show"), 2600);
}

async function post(path, body = {}) {
  const response = await fetch(path, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body)
  });
  const json = await response.json();
  if (!response.ok || json.ok === false) {
    throw new Error(json.error || `HTTP ${response.status}`);
  }
  return json;
}

async function getJson(path) {
  const response = await fetch(path);
  const json = await response.json();
  if (!response.ok || json.ok === false) {
    throw new Error(json.error || `HTTP ${response.status}`);
  }
  return json;
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function statusClass(status) {
  return String(status || "").toLowerCase();
}

function formatUtc(ms) {
  const n = Number(ms);
  if (!Number.isFinite(n)) return "-";
  return new Date(n).toISOString().replace(".000Z", "Z");
}

function resultLoading(target, label = "Loading") {
  target.innerHTML = `<div class="loading">${escapeHtml(label)}</div>`;
}

function resultError(target, error) {
  target.innerHTML = `<div class="error">${escapeHtml(error.message || error)}</div>`;
}

function renderKv(values = {}) {
  const entries = Object.entries(values).filter(([, value]) => value !== undefined && value !== null);
  if (!entries.length) return `<div class="empty">No values</div>`;
  return `<dl>${entries.map(([key, value]) => `
    <div class="kv">
      <dt>${escapeHtml(key)}</dt>
      <dd class="${String(value).length > 48 ? "hash" : ""}">${escapeHtml(value)}</dd>
    </div>
  `).join("")}</dl>`;
}

function renderJson(value) {
  return `<pre class="code-block">${escapeHtml(JSON.stringify(value, null, 2))}</pre>`;
}

function renderRaw(raw) {
  const parts = [];
  if (raw?.stdout) parts.push(`stdout\n${raw.stdout}`);
  if (raw?.stderr) parts.push(`stderr\n${raw.stderr}`);
  if (!parts.length) parts.push("no output");
  return `<pre class="code-block">${escapeHtml(parts.join("\n\n"))}</pre>`;
}

function renderRawDetails(raw, label = "Raw output") {
  if (!raw) return "";
  return `<details>
    <summary class="tx-item">${escapeHtml(label)}</summary>
    ${renderRaw(raw)}
  </details>`;
}

function renderJsonDetails(value, label = "Raw JSON") {
  return `<details>
    <summary class="tx-item">${escapeHtml(label)}</summary>
    ${renderJson(value)}
  </details>`;
}

function formatLabel(value) {
  return String(value || "")
    .replaceAll("_", " ")
    .replaceAll("-", " ")
    .replace(/\b\w/g, (ch) => ch.toUpperCase());
}

function isScalarValue(value) {
  return value === null || ["string", "number", "boolean"].includes(typeof value);
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

function renderStructuredValue(value) {
  if (value === null || value === undefined) return `<div class="empty">No result</div>`;
  if (isScalarValue(value)) return renderKv({ value: valueText(value) });
  if (Array.isArray(value)) {
    if (!value.length) return `<div class="empty">No items</div>`;
    return `<div class="tx-list">${value.map((item, index) => `
      <div class="tx-item">
        <div class="tx-top">
          <strong>Item ${index + 1}</strong>
          <span class="pill">${escapeHtml(valueKind(item))}</span>
        </div>
        ${renderStructuredValue(item)}
      </div>
    `).join("")}</div>`;
  }

  const scalars = {};
  const nested = [];
  for (const [key, item] of Object.entries(value)) {
    if (isScalarValue(item)) scalars[formatLabel(key)] = valueText(item);
    else nested.push([key, item]);
  }

  return `
    ${Object.keys(scalars).length ? renderKv(scalars) : ""}
    ${nested.map(([key, item]) => `
      <div class="tx-item">
        <div class="tx-top">
          <strong>${escapeHtml(formatLabel(key))}</strong>
          <span class="pill">${escapeHtml(valueKind(item))}</span>
        </div>
        ${renderStructuredValue(item)}
      </div>
    `).join("")}
  `;
}

function renderStructuredResult(value, title = "Result") {
  return `<div class="tx-item">
    <div class="tx-top">
      <strong>${escapeHtml(title)}</strong>
      <span class="pill">${escapeHtml(valueKind(value))}</span>
    </div>
    ${renderStructuredValue(value)}
  </div>`;
}

function renderRpcResponse(response) {
  const hasError = response && Object.hasOwn(response, "error");
  const hasResult = response && Object.hasOwn(response, "result");
  return `
    ${renderKv({
      jsonrpc: response?.jsonrpc,
      id: response?.id,
      status: hasError ? "error" : hasResult ? "ok" : "unknown",
      result: hasResult ? valueKind(response.result) : "-"
    })}
    ${hasError ? `<div class="error">${escapeHtml(valueText(response.error))}</div>` : ""}
    ${hasResult ? renderStructuredResult(response.result, "RPC result") : `<div class="empty">No result field</div>`}
  `;
}

function renderProgramFileResult(json) {
  return `${renderStructuredResult(json.parsed?.values || {}, "Program file")}${renderRawDetails(json.raw, "Raw command output")}`;
}

function renderCliResult(json) {
  return `
    ${renderStructuredResult(json.parsed?.values || {}, "Command values")}
    ${renderTransactions(json.parsed?.transactions || [])}
    ${renderRawDetails(json.raw, "Raw command output")}
  `;
}

function parseWords(value) {
  if (Array.isArray(value)) {
    return value.map((word) => Number(word)).filter(Number.isFinite);
  }
  return String(value || "")
    .split(",")
    .map((word) => Number(word.trim()))
    .filter(Number.isFinite);
}

function parseList(value) {
  if (Array.isArray(value)) return value.map(String);
  return String(value || "")
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

function bytesToHex(bytes) {
  return [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function wordsToBytesLe(words) {
  const bytes = [];
  for (const raw of words) {
    const word = Number(raw) >>> 0;
    bytes.push(word & 0xff, (word >>> 8) & 0xff, (word >>> 16) & 0xff, (word >>> 24) & 0xff);
  }
  return bytes;
}

function base58ToBytes(value) {
  const text = String(value || "").trim();
  if (!text) throw new Error("empty base58");
  let n = 0n;
  for (const ch of text) {
    const index = base58Alphabet.indexOf(ch);
    if (index < 0) throw new Error("invalid base58");
    n = n * 58n + BigInt(index);
  }

  let bytes = [];
  while (n > 0n) {
    bytes.unshift(Number(n & 0xffn));
    n >>= 8n;
  }

  let leadingZeros = 0;
  for (const ch of text) {
    if (ch !== "1") break;
    leadingZeros += 1;
  }
  while (leadingZeros > 0) {
    bytes.unshift(0);
    leadingZeros -= 1;
  }
  return bytes;
}

function base58ToFixedBytes(value, length = 32) {
  const bytes = base58ToBytes(value);
  if (bytes.length > length) throw new Error("base58 value too long");
  while (bytes.length < length) bytes.unshift(0);
  return bytes;
}

function bytesToBase58(bytes) {
  let zeros = 0;
  for (const byte of bytes) {
    if (byte !== 0) break;
    zeros += 1;
  }

  let n = 0n;
  for (const byte of bytes) {
    n = (n << 8n) + BigInt(byte);
  }

  let out = "";
  while (n > 0n) {
    const rem = Number(n % 58n);
    out = base58Alphabet[rem] + out;
    n /= 58n;
  }

  return "1".repeat(zeros) + (out || "");
}

function base64ToBytes(value) {
  const binary = atob(String(value || ""));
  return [...binary].map((ch) => ch.charCodeAt(0));
}

function hexToBytes(value) {
  const hex = String(value || "").trim().replace(/^0x/i, "");
  if (hex.length % 2 !== 0 || /[^0-9a-f]/i.test(hex)) throw new Error("invalid hex data");
  const bytes = [];
  for (let i = 0; i < hex.length; i += 2) {
    bytes.push(Number.parseInt(hex.slice(i, i + 2), 16));
  }
  return bytes;
}

function accountDataBytes(account) {
  if (Array.isArray(account?.data)) return account.data.map((byte) => Number(byte) & 0xff);
  if (typeof account?.data === "string") return base64ToBytes(account.data);
  return [];
}

function accountOwnerKey(account) {
  if (Array.isArray(account?.program_owner)) return bytesToHex(wordsToBytesLe(account.program_owner));
  if (typeof account?.program_owner === "string") return programKey(account.program_owner);
  return "";
}

function accountOwnerDisplay(account) {
  if (Array.isArray(account?.program_owner)) return bytesToHex(wordsToBytesLe(account.program_owner));
  return account?.program_owner || "-";
}

function programKey(value) {
  const raw = String(value || "").trim();
  const hex = raw.replace(/^0x/i, "").toLowerCase();
  if (/^[0-9a-f]{64}$/.test(hex)) return hex;
  try {
    return bytesToHex(base58ToFixedBytes(raw, 32));
  } catch {
    return raw.toLowerCase();
  }
}

function readUnsigned(words, offset, count) {
  if (offset + count > words.length) return null;
  let value = 0n;
  for (let i = 0; i < count; i += 1) {
    value += BigInt(words[offset + i] >>> 0) << (32n * BigInt(i));
  }
  return value;
}

function readSigned32(word) {
  return String(word | 0);
}

function dataView(bytes) {
  const array = Uint8Array.from(bytes);
  return new DataView(array.buffer, array.byteOffset, array.byteLength);
}

function ensureBytes(bytes, offset, count) {
  if (offset + count > bytes.length) {
    throw new Error("unexpected end of account data");
  }
}

function readLeUnsigned(bytes, offset, count) {
  ensureBytes(bytes, offset, count);
  let value = 0n;
  for (let i = 0; i < count; i += 1) {
    value += BigInt(bytes[offset + i]) << (8n * BigInt(i));
  }
  return value;
}

function readLeSigned(bytes, offset, count) {
  const unsigned = readLeUnsigned(bytes, offset, count);
  const bits = BigInt(count * 8);
  const sign = 1n << (bits - 1n);
  return unsigned >= sign ? unsigned - (1n << bits) : unsigned;
}

function findDefinedShape(idl, name) {
  const type = (idl.types || []).find((item) => item.name === name);
  if (type) return type;
  const account = (idl.accounts || []).find((item) => item.name === name);
  return account?.type || null;
}

function decodeBorshShape(shape, bytes, offset, idl, depth = 0) {
  if (depth > 16) throw new Error("IDL nesting too deep");
  if (!shape || typeof shape !== "object") throw new Error("invalid IDL shape");

  if (shape.kind === "struct") {
    const fields = {};
    let cursor = offset;
    for (const field of shape.fields || []) {
      const decoded = decodeBorshType(field.type, bytes, cursor, idl, depth + 1);
      fields[field.name] = decoded.value;
      cursor += decoded.consumed;
    }
    return { value: { kind: "struct", fields }, consumed: cursor - offset };
  }

  if (shape.kind === "enum") {
    ensureBytes(bytes, offset, 1);
    const variantIndex = bytes[offset];
    const variant = (shape.variants || [])[variantIndex];
    if (!variant) throw new Error(`enum variant ${variantIndex} not present`);
    const fields = {};
    let cursor = offset + 1;
    for (const field of variant.fields || []) {
      const decoded = decodeBorshType(field.type, bytes, cursor, idl, depth + 1);
      fields[field.name] = decoded.value;
      cursor += decoded.consumed;
    }
    return {
      value: { kind: "enum", variant: variant.name, variantIndex, fields },
      consumed: cursor - offset
    };
  }

  throw new Error(`unsupported IDL account kind ${shape.kind || "unknown"}`);
}

function decodeBorshType(type, bytes, offset, idl, depth = 0) {
  const label = typeLabel(type);
  if (typeof type === "object" && type?.option) {
    ensureBytes(bytes, offset, 1);
    if (bytes[offset] === 0) return { value: null, consumed: 1 };
    const decoded = decodeBorshType(type.option, bytes, offset + 1, idl, depth + 1);
    return { value: decoded.value, consumed: decoded.consumed + 1 };
  }
  if (typeof type === "object" && type?.vec) {
    const length = Number(readLeUnsigned(bytes, offset, 4));
    if (!Number.isSafeInteger(length) || length > 2_000) throw new Error("unsupported vector length");
    const values = [];
    let cursor = offset + 4;
    for (let i = 0; i < length; i += 1) {
      const decoded = decodeBorshType(type.vec, bytes, cursor, idl, depth + 1);
      values.push(decoded.value);
      cursor += decoded.consumed;
    }
    return { value: values, consumed: cursor - offset };
  }
  if (typeof type === "object" && type?.defined) {
    const shape = findDefinedShape(idl, type.defined);
    if (!shape) throw new Error(`defined type not found: ${type.defined}`);
    return decodeBorshShape(shape, bytes, offset, idl, depth + 1);
  }
  if (typeof type === "object" && type.kind) {
    return decodeBorshShape(type, bytes, offset, idl, depth + 1);
  }
  if (typeof type === "object") throw new Error(`unsupported IDL type ${label}`);

  switch (type) {
    case "bool":
      ensureBytes(bytes, offset, 1);
      return { value: bytes[offset] !== 0, consumed: 1 };
    case "u8":
      ensureBytes(bytes, offset, 1);
      return { value: String(bytes[offset]), consumed: 1 };
    case "i8":
      ensureBytes(bytes, offset, 1);
      return { value: String(bytes[offset] > 127 ? bytes[offset] - 256 : bytes[offset]), consumed: 1 };
    case "u16":
      ensureBytes(bytes, offset, 2);
      return { value: String(dataView(bytes).getUint16(offset, true)), consumed: 2 };
    case "i16":
      ensureBytes(bytes, offset, 2);
      return { value: String(dataView(bytes).getInt16(offset, true)), consumed: 2 };
    case "u32":
      ensureBytes(bytes, offset, 4);
      return { value: String(dataView(bytes).getUint32(offset, true)), consumed: 4 };
    case "i32":
      ensureBytes(bytes, offset, 4);
      return { value: String(dataView(bytes).getInt32(offset, true)), consumed: 4 };
    case "u64":
      return { value: readLeUnsigned(bytes, offset, 8).toString(), consumed: 8 };
    case "i64":
      return { value: readLeSigned(bytes, offset, 8).toString(), consumed: 8 };
    case "u128":
      return { value: readLeUnsigned(bytes, offset, 16).toString(), consumed: 16 };
    case "i128":
      return { value: readLeSigned(bytes, offset, 16).toString(), consumed: 16 };
    case "account_id":
      ensureBytes(bytes, offset, 32);
      return { value: bytesToBase58(bytes.slice(offset, offset + 32)), consumed: 32 };
    case "program_id":
      ensureBytes(bytes, offset, 32);
      return { value: bytesToHex(bytes.slice(offset, offset + 32)), consumed: 32 };
    case "string": {
      const length = Number(readLeUnsigned(bytes, offset, 4));
      if (!Number.isSafeInteger(length)) throw new Error("string length too large");
      ensureBytes(bytes, offset + 4, length);
      return {
        value: textDecoder.decode(Uint8Array.from(bytes.slice(offset + 4, offset + 4 + length))),
        consumed: 4 + length
      };
    }
    default:
      throw new Error(`unsupported IDL type ${type}`);
  }
}

function flattenDecodedValue(value, prefix = "", rows = []) {
  if (value && typeof value === "object" && value.kind === "enum") {
    rows.push({ name: prefix ? `${prefix}.variant` : "variant", value: value.variant });
    for (const [name, child] of Object.entries(value.fields || {})) {
      flattenDecodedValue(child, prefix ? `${prefix}.${name}` : name, rows);
    }
    return rows;
  }
  if (value && typeof value === "object" && value.kind === "struct") {
    for (const [name, child] of Object.entries(value.fields || {})) {
      flattenDecodedValue(child, prefix ? `${prefix}.${name}` : name, rows);
    }
    return rows;
  }
  rows.push({
    name: prefix || "value",
    value: Array.isArray(value) || (value && typeof value === "object")
      ? JSON.stringify(value)
      : String(value)
  });
  return rows;
}

function typeLabel(type) {
  if (typeof type === "string") return type;
  if (type?.option) return `option<${typeLabel(type.option)}>`;
  if (type?.vec) return `vec<${typeLabel(type.vec)}>`;
  if (type?.defined) return type.defined;
  return JSON.stringify(type);
}

function decodeByType(type, words, offset) {
  const label = typeLabel(type);
  if (typeof type === "object" && type?.option) {
    if (offset >= words.length) return { unsupported: true, label };
    const tag = words[offset] >>> 0;
    if (tag === 0) return { value: "None", consumed: 1, label };
    const decoded = decodeByType(type.option, words, offset + 1);
    if (decoded.unsupported) return { unsupported: true, label };
    return { value: `Some(${decoded.value})`, consumed: decoded.consumed + 1, label };
  }
  if (typeof type === "object") return { unsupported: true, label };

  switch (type) {
    case "bool":
      if (offset >= words.length) return { unsupported: true, label };
      return { value: words[offset] ? "true" : "false", consumed: 1, label };
    case "u8":
    case "u16":
    case "u32":
      if (offset >= words.length) return { unsupported: true, label };
      return { value: String(words[offset] >>> 0), consumed: 1, label };
    case "i8":
    case "i16":
    case "i32":
      if (offset >= words.length) return { unsupported: true, label };
      return { value: readSigned32(words[offset]), consumed: 1, label };
    case "u64": {
      const value = readUnsigned(words, offset, 2);
      return value === null ? { unsupported: true, label } : { value: value.toString(), consumed: 2, label };
    }
    case "i64": {
      const value = readUnsigned(words, offset, 2);
      return value === null ? { unsupported: true, label } : { value: value.toString(), consumed: 2, label };
    }
    case "u128": {
      const value = readUnsigned(words, offset, 4);
      return value === null ? { unsupported: true, label } : { value: value.toString(), consumed: 4, label };
    }
    case "i128": {
      const value = readUnsigned(words, offset, 4);
      return value === null ? { unsupported: true, label } : { value: value.toString(), consumed: 4, label };
    }
    case "program_id": {
      if (offset + 8 > words.length) return { unsupported: true, label };
      return {
        value: bytesToHex(wordsToBytesLe(words.slice(offset, offset + 8))),
        consumed: 8,
        label
      };
    }
    case "account_id": {
      if (offset + 8 > words.length) return { unsupported: true, label };
      return {
        value: bytesToBase58(wordsToBytesLe(words.slice(offset, offset + 8))),
        consumed: 8,
        label
      };
    }
    case "string": {
      if (offset >= words.length) return { unsupported: true, label };
      const byteLength = words[offset] >>> 0;
      const wordLength = Math.ceil(byteLength / 4);
      if (offset + 1 + wordLength > words.length) return { unsupported: true, label };
      const bytes = wordsToBytesLe(words.slice(offset + 1, offset + 1 + wordLength)).slice(0, byteLength);
      return {
        value: textDecoder.decode(new Uint8Array(bytes)),
        consumed: 1 + wordLength,
        label
      };
    }
    default:
      return { unsupported: true, label };
  }
}

function decodeInstruction(programId, instructionData, accountIds) {
  const key = programKey(programId);
  const entry = state.idls[key] || state.idls[String(programId || "").toLowerCase()];
  if (!entry?.idl) return null;

  const words = parseWords(instructionData);
  if (!words.length) {
    return { matched: false, reason: "empty instruction data", idl: entry.idl, programId };
  }

  const variantIndex = words[0] >>> 0;
  const instruction = entry.idl.instructions?.[variantIndex];
  if (!instruction) {
    return {
      matched: false,
      reason: `variant ${variantIndex} not present in ${entry.idl.name || "IDL"}`,
      idl: entry.idl,
      programId
    };
  }

  const accounts = parseList(accountIds);
  const decodedAccounts = (instruction.accounts || []).map((account, index) => ({
    name: account.name || `account_${index}`,
    account: accounts[index] || "-"
  }));
  for (let i = decodedAccounts.length; i < accounts.length; i += 1) {
    decodedAccounts.push({ name: `extra_${i}`, account: accounts[i] });
  }

  let offset = 1;
  const args = [];
  for (const arg of instruction.args || []) {
    const decoded = decodeByType(arg.type, words, offset);
    if (decoded.unsupported) {
      args.push({
        name: arg.name,
        type: decoded.label,
        value: `raw words ${offset}..${Math.max(offset, words.length - 1)}`
      });
      offset = words.length;
      break;
    }
    args.push({
      name: arg.name,
      type: decoded.label,
      value: decoded.value
    });
    offset += decoded.consumed;
  }

  return {
    matched: true,
    idl: entry.idl,
    programId,
    instruction: instruction.name,
    variantIndex,
    accounts: decodedAccounts,
    args,
    remainingWords: words.slice(offset)
  };
}

function renderDecodeTable(rows, keyName, valueName) {
  if (!rows.length) return "";
  return `
    <div class="decode-table">
      ${rows.map((row) => `
        <div>
          <span>${escapeHtml(row[keyName])}</span>
          <strong class="hash">${escapeHtml(row[valueName])}</strong>
        </div>
      `).join("")}
    </div>
  `;
}

function renderInstructionDecode(decoded, programId) {
  if (!decoded) {
    return `
      <div class="decode-box">
        <div class="decode-title">IDL not mapped</div>
        <div class="hash">${escapeHtml(programId || "-")}</div>
      </div>
    `;
  }
  if (!decoded.matched) {
    return `
      <div class="decode-box">
        <div class="decode-title">${escapeHtml(decoded.idl?.name || "IDL")} decode skipped</div>
        <div>${escapeHtml(decoded.reason)}</div>
      </div>
    `;
  }
  const args = decoded.args.map((arg) => ({
    name: `${arg.name} : ${arg.type}`,
    value: arg.value
  }));
  if (decoded.remainingWords.length) {
    args.push({
      name: "remaining_words",
      value: decoded.remainingWords.join(",")
    });
  }
  return `
    <div class="decode-box">
      <div class="decode-title">
        ${escapeHtml(decoded.idl?.name || "program")}::${escapeHtml(decoded.instruction)}
        <span>variant ${escapeHtml(decoded.variantIndex)}</span>
      </div>
      ${renderDecodeTable(decoded.accounts, "name", "account")}
      ${renderDecodeTable(args, "name", "value")}
    </div>
  `;
}

function idlAccountOptions() {
  const options = [];
  for (const [key, entry] of Object.entries(state.idls)) {
    for (const account of entry.idl?.accounts || []) {
      options.push({
        key,
        accountName: account.name,
        label: `${entry.idl?.name || "IDL"} / ${account.name}`,
        idl: entry.idl,
        account
      });
    }
  }
  return options;
}

function accountOptionValue(key, accountName) {
  return `${encodeURIComponent(key)}|${encodeURIComponent(accountName)}`;
}

function parseAccountOptionValue(value) {
  const [key, accountName] = String(value || "").split("|");
  if (!key || !accountName) return null;
  return {
    key: decodeURIComponent(key),
    accountName: decodeURIComponent(accountName)
  };
}

function renderAccountTypeSelect() {
  const select = $("#accountIdlType");
  if (!select) return;
  const current = select.value;
  const options = idlAccountOptions();
  select.innerHTML = `<option value="">Auto</option>${options.map((option) => `
    <option value="${escapeHtml(accountOptionValue(option.key, option.accountName))}">
      ${escapeHtml(option.label)}
    </option>
  `).join("")}`;
  if ([...select.options].some((option) => option.value === current)) {
    select.value = current;
  }
}

function accountDecodeCandidates(account, selectedValue) {
  const options = idlAccountOptions();
  const selected = parseAccountOptionValue(selectedValue);
  if (selected) {
    return options.filter((option) => option.key === selected.key && option.accountName === selected.accountName);
  }

  const owner = accountOwnerKey(account);
  return [
    ...options.filter((option) => option.key === owner),
    ...options.filter((option) => option.key !== owner)
  ];
}

function decodeAccountData(account, selectedValue) {
  const bytes = accountDataBytes(account);
  if (!bytes.length) return [];

  const results = [];
  for (const candidate of accountDecodeCandidates(account, selectedValue)) {
    try {
      const decoded = decodeBorshShape(candidate.account.type, bytes, 0, candidate.idl);
      if (decoded.consumed !== bytes.length) {
        if (selectedValue) {
          results.push({
            ok: false,
            label: candidate.label,
            error: `decoded ${decoded.consumed} of ${bytes.length} bytes`
          });
        }
        continue;
      }
      results.push({
        ok: true,
        label: candidate.label,
        rows: flattenDecodedValue(decoded.value)
      });
      if (selectedValue) break;
    } catch (error) {
      if (selectedValue) {
        results.push({ ok: false, label: candidate.label, error: error.message });
        break;
      }
    }
  }
  return results;
}

function renderAccountDecode(results) {
  if (!results.length) return `<div class="empty">No IDL account decode matched</div>`;
  return results.map((result) => `
    <div class="decode-box">
      <div class="decode-title">
        ${escapeHtml(result.label)}
        <span>${result.ok ? "decoded" : "failed"}</span>
      </div>
      ${result.ok
        ? renderDecodeTable(result.rows, "name", "value")
        : `<div class="error">${escapeHtml(result.error)}</div>`}
    </div>
  `).join("");
}

function renderAccountInspection(target, account, raw = null) {
  const bytes = accountDataBytes(account);
  const selected = $("#accountIdlType").value;
  const decoded = decodeAccountData(account, selected);
  target.innerHTML = `
    ${renderKv({
      program_owner: accountOwnerDisplay(account),
      balance: account?.balance ?? "-",
      nonce: account?.nonce ?? "-",
      data_len: bytes.length,
      data_hex: bytesToHex(bytes)
    })}
    ${renderAccountDecode(decoded)}
    ${raw ? renderRawDetails(raw, "Raw account") : renderJsonDetails(account, "Raw account")}
  `;
}

function renderTransactions(transactions = []) {
  if (!transactions.length) return "";
  return `
    <div class="tx-list">
      ${transactions.map((tx) => {
        const programId = tx.program_id_hex || tx.program_id;
        const decoded = tx.kind === "Public"
          ? decodeInstruction(programId, tx.instruction_data, tx.account_ids)
          : null;
        return `
          <div class="tx-item">
            <div class="tx-top">
              <strong>tx[${escapeHtml(tx.index ?? "-")}]</strong>
              <span class="pill">${escapeHtml(tx.kind || "transaction")}</span>
            </div>
            ${renderKv(Object.fromEntries(Object.entries(tx).filter(([key]) => key !== "index")))}
            ${tx.kind === "Public" ? renderInstructionDecode(decoded, programId) : ""}
          </div>
        `;
      }).join("")}
    </div>
  `;
}

function renderBlock(target, decoded) {
  const values = decoded.values || {};
  target.innerHTML = `
    <div class="tx-item">
      <div class="tx-top">
        <strong>Block ${escapeHtml(values.block_id || "-")}</strong>
        <span class="pill ${statusClass(values.bedrock_status)}">${escapeHtml(values.bedrock_status || "-")}</span>
      </div>
      ${renderKv({
        timestamp: values.timestamp || "-",
        utc: formatUtc(values.timestamp),
        tx_count: values.tx_count || "0"
      })}
    </div>
    ${renderTransactions(decoded.transactions)}
    ${renderRawDetails(decoded.raw, "Raw output")}
  `;
}

function renderRail(target, blocks = []) {
  target.innerHTML = blocks.map((block) => {
    const values = block.values || block;
    const status = values.bedrock_status || "Unknown";
    return `
      <button class="rail-block ${statusClass(status)}" type="button" data-block="${escapeHtml(values.block_id)}">
        <strong>${escapeHtml(values.block_id || "-")}</strong>
        <span>${escapeHtml(status)}</span>
      </button>
    `;
  }).join("");
}

function renderRows(target, blocks = []) {
  target.innerHTML = blocks.map((block) => {
    const values = block.values || block;
    const status = values.bedrock_status || "-";
    return `
      <tr>
        <td><button class="secondary row-block" type="button" data-block="${escapeHtml(values.block_id)}">${escapeHtml(values.block_id || "-")}</button></td>
        <td><span class="pill ${statusClass(status)}">${escapeHtml(status)}</span></td>
        <td>${escapeHtml(formatUtc(values.timestamp))}</td>
        <td>${escapeHtml(values.tx_count || "-")}</td>
      </tr>
    `;
  }).join("");
}

function renderPrograms(target, programs) {
  const entries = Object.entries(programs || {});
  if (!entries.length) {
    target.innerHTML = `<div class="empty">No program IDs</div>`;
    return;
  }
  target.innerHTML = entries.map(([name, value]) => `
    <div class="program-item">
      <strong>${escapeHtml(name)}</strong>
      <span class="hash">${escapeHtml(Array.isArray(value) ? value.join(", ") : value)}</span>
    </div>
  `).join("");
}

function enumPayload(value) {
  if (!value || typeof value !== "object") return ["Unknown", value];
  const entries = Object.entries(value);
  if (entries.length === 1) return entries[0];
  return ["Unknown", value];
}

function flattenIndexerTransaction(tx, index = 0) {
  const [kind, payload] = enumPayload(tx);
  if (kind === "Public") {
    const message = payload.message || {};
    return {
      index,
      kind,
      hash: payload.hash,
      program_id: message.program_id,
      account_ids: parseList(message.account_ids || []).join(","),
      nonces: parseList(message.nonces || []).join(","),
      instruction_data: parseWords(message.instruction_data || []).join(",")
    };
  }
  if (kind === "ProgramDeployment") {
    return {
      index,
      kind,
      hash: payload.hash,
      bytecode_len: payload.message?.bytecode?.length || "-"
    };
  }
  return {
    index,
    kind,
    hash: payload?.hash || "-"
  };
}

function normalizeIndexerBlock(block) {
  const transactions = block?.body?.transactions || [];
  return {
    values: {
      block_id: block?.header?.block_id,
      timestamp: block?.header?.timestamp,
      bedrock_status: block?.bedrock_status,
      tx_count: transactions.length,
      hash: block?.header?.hash,
      bedrock_parent_id: block?.bedrock_parent_id
    },
    transactions: transactions.map(flattenIndexerTransaction),
    raw: block
  };
}

function renderIndexerBlocks(target, blocks = []) {
  if (!blocks.length) {
    target.innerHTML = `<div class="empty">No blocks</div>`;
    return;
  }
  const normalized = blocks.map(normalizeIndexerBlock);
  target.innerHTML = `
    <div class="rail">${normalized.map((block) => {
      const values = block.values;
      return `
        <button class="rail-block ${statusClass(values.bedrock_status)}" type="button" data-indexer-block="${escapeHtml(values.block_id)}">
          <strong>${escapeHtml(values.block_id)}</strong>
          <span>${escapeHtml(values.bedrock_status || "-")}</span>
        </button>
      `;
    }).join("")}</div>
    ${normalized.map((block) => `
      <div class="tx-item">
        <div class="tx-top">
          <strong>Block ${escapeHtml(block.values.block_id)}</strong>
          <span class="pill ${statusClass(block.values.bedrock_status)}">${escapeHtml(block.values.bedrock_status || "-")}</span>
        </div>
        ${renderKv({
          hash: block.values.hash,
          timestamp: block.values.timestamp,
          utc: formatUtc(block.values.timestamp),
          tx_count: block.values.tx_count
        })}
        ${renderTransactions(block.transactions)}
      </div>
    `).join("")}
  `;
}

function renderIndexerTransactionResult(target, tx) {
  if (!tx) {
    target.innerHTML = `<div class="empty">Transaction not found</div>`;
    return;
  }
  target.innerHTML = `${renderTransactions([flattenIndexerTransaction(tx)])}${renderJsonDetails(tx, "Raw transaction")}`;
}

async function sequencerRpc(method, params = []) {
  return post("/api/rpc", { endpoint: endpoint(), method, params });
}

async function indexerRpc(method, params = []) {
  return post("/api/indexer-rpc", { endpoint: indexerEndpoint(), method, params });
}

async function refreshStatus() {
  const status = await getJson("/api/status");
  state.endpoint = storedEndpoints.endpoint || status.defaultSequencerEndpoint || status.defaultEndpoint;
  state.indexerEndpoint = storedEndpoints.indexerEndpoint || status.defaultIndexerEndpoint;
  $("#endpointInput").value ||= state.endpoint;
  $("#indexerEndpointInput").value ||= state.indexerEndpoint;
  $("#cliState").textContent = status.cliReady ? "cli ready" : "cli missing";
  $("#cliMetric").textContent = status.cliReady ? "ready" : "missing";
  if (!status.cliReady) toast("CLI binary missing. Build CLI before decoding.");
  await Promise.allSettled([refreshHead(), refreshIndexerHead()]);
}

async function refreshHead() {
  const json = await sequencerRpc("getLastBlockId", []);
  const result = json.response.result;
  state.latestHead = result;
  $("#headMetric").textContent = result ?? "-";
  if (!$("#blockIdInput").value && result) $("#blockIdInput").placeholder = String(result);
  return result;
}

async function refreshIndexerHead() {
  try {
    const json = await indexerRpc("getLastFinalizedBlockId", []);
    const result = json.response.result;
    state.indexerHead = result;
    $("#indexerHeadMetric").textContent = result ?? "-";
    return result;
  } catch (error) {
    state.indexerHead = null;
    $("#indexerHeadMetric").textContent = "offline";
    return null;
  }
}

async function inspectBlock(blockId, target = $("#blockResult")) {
  const id = Number(blockId || state.latestHead);
  if (!Number.isInteger(id) || id < 1) throw new Error("block ID is required");
  state.currentBlock = id;
  $("#blockIdInput").value = String(state.currentBlock);
  resultLoading(target, `Loading block ${id}`);
  const json = await post("/api/block", { endpoint: endpoint(), blockId: state.currentBlock });
  renderBlock(target, json.decoded);
  return json;
}

function updateOverviewFromRange(blocks) {
  const latest = blocks[blocks.length - 1]?.values;
  $("#latestStatusMetric").textContent = latest
    ? `${latest.block_id} ${latest.bedrock_status || "-"}`
    : "-";
}

async function scanRange(start, end, targets = {}) {
  const rail = targets.rail || $("#rangeRail");
  const rows = targets.rows || $("#rangeRows");
  rail.innerHTML = `<div class="loading">Scanning</div>`;
  rows.innerHTML = "";
  const json = await post("/api/block-range", { endpoint: endpoint(), start, end });
  state.lastRange = json.decoded.blocks;
  renderRail(rail, state.lastRange);
  renderRows(rows, state.lastRange);
  updateOverviewFromRange(state.lastRange);
  return json;
}

async function scanHeadWindow(targets = {}) {
  await refreshHead();
  const end = Number(state.latestHead);
  if (!Number.isInteger(end) || end < 1) throw new Error("sequencer head unavailable");
  const windowSize = Math.max(1, Math.min(51, Number($("#overviewWindowInput").value || 8)));
  const start = Math.max(1, end - windowSize + 1);
  $("#rangeStartInput").value = String(start);
  $("#rangeEndInput").value = String(end);
  return scanRange(start, end, targets);
}

async function loadProgramIds() {
  const json = await sequencerRpc("getProgramIds", []);
  renderPrograms($("#programGrid"), json.response.result);
  renderPrograms($("#programsPanelGrid"), json.response.result);
}

function activateView(name) {
  $$(".nav-item").forEach((button) => button.classList.toggle("is-active", button.dataset.view === name));
  $$(".view").forEach((view) => view.classList.toggle("is-active", view.id === `view-${name}`));
  $("#viewTitle").textContent = $(".nav-item.is-active")?.textContent || "LEZ Inspect";
}

function updateCommandPreview() {
  const command = $("#cliCommand").value;
  const args = splitArgs($("#cliArgs").value);
  const preview = ["cargo", "run", "-p", "lez-inspect", "--", command, ...args]
    .map(shellQuote)
    .join(" ");
  $("#commandPreview").textContent = preview;
}

function shellQuote(value) {
  if (/^[A-Za-z0-9_./:=+-]+$/.test(value)) return value;
  return `'${value.replaceAll("'", "'\\''")}'`;
}

function splitArgs(raw) {
  const args = [];
  let current = "";
  let quote = null;
  for (let i = 0; i < raw.length; i += 1) {
    const ch = raw[i];
    if (quote) {
      if (ch === quote) quote = null;
      else current += ch;
      continue;
    }
    if (ch === "'" || ch === '"') {
      quote = ch;
    } else if (/\s/.test(ch)) {
      if (current) {
        args.push(current);
        current = "";
      }
    } else {
      current += ch;
    }
  }
  if (current) args.push(current);
  return args;
}

function renderIdlList() {
  const entries = Object.entries(state.idls);
  const target = $("#idlMapList");
  if (!entries.length) {
    target.innerHTML = `<div class="empty">No IDLs mapped</div>`;
    renderAccountTypeSelect();
    return;
  }
  target.innerHTML = entries.map(([key, entry]) => `
    <div class="program-item">
      <strong>${escapeHtml(entry.idl?.name || "program")} ${escapeHtml(entry.idl?.version || "")}</strong>
      <span class="hash">${escapeHtml(entry.programId || key)}</span>
      <span>${escapeHtml(entry.idl?.instructions?.length || 0)} instructions</span>
      <span>${escapeHtml(entry.idl?.accounts?.length || 0)} accounts</span>
      <button class="secondary" type="button" data-remove-idl="${escapeHtml(key)}">Remove</button>
    </div>
  `).join("");
  renderAccountTypeSelect();
}

async function loadArtifacts() {
  const json = await getJson("/api/idls");
  const select = $("#idlArtifactSelect");
  select.innerHTML = `<option value="">Select IDL</option>${json.artifacts.map((artifact) => `
    <option value="${escapeHtml(artifact.path)}">${escapeHtml(artifact.name)}</option>
  `).join("")}`;
}

function setRpcSample(sample) {
  const source = $("#rpcSource").value;
  if (source === "indexer") {
    const samples = {
      head: ["getLastFinalizedBlockId", "[]"],
      programs: ["getSchema", "[]"],
      block: ["getBlockById", `[${state.indexerHead || 1}]`],
      health: ["checkHealth", "[]"]
    };
    const [method, params] = samples[sample] || samples.head;
    $("#rpcMethod").value = method;
    $("#rpcParams").value = params;
    return;
  }
  const samples = {
    head: ["getLastBlockId", "[]"],
    programs: ["getProgramIds", "[]"],
    block: ["getBlock", `[${state.latestHead || 1}]`],
    health: ["checkHealth", "[]"]
  };
  const [method, params] = samples[sample] || samples.head;
  $("#rpcMethod").value = method;
  $("#rpcParams").value = params;
}

async function loadIndexerStatus(target = $("#indexerStatusResult")) {
  resultLoading(target, "Checking indexer");
  const [health, head] = await Promise.allSettled([
    indexerRpc("checkHealth", []),
    indexerRpc("getLastFinalizedBlockId", [])
  ]);
  const values = {
    endpoint: indexerEndpoint(),
    health: health.status === "fulfilled" ? "ok" : health.reason.message,
    last_finalized_block_id: head.status === "fulfilled" ? head.value.response.result : head.reason.message
  };
  if (head.status === "fulfilled") {
    state.indexerHead = head.value.response.result;
    $("#indexerHeadMetric").textContent = state.indexerHead ?? "-";
  }
  target.innerHTML = renderKv(values);
}

async function loadIndexerBlocks(target = $("#indexerBlocksResult")) {
  resultLoading(target, "Loading indexer blocks");
  const beforeRaw = $("#indexerBeforeInput").value.trim();
  const before = beforeRaw ? Number(beforeRaw) : null;
  const limit = Math.max(1, Math.min(50, Number($("#indexerLimitInput").value || 10)));
  const json = await indexerRpc("getBlocks", [before, limit]);
  renderIndexerBlocks(target, json.response.result || []);
}

async function runIndexerLookup(target = $("#indexerLookupResult")) {
  resultLoading(target, "Running lookup");
  const type = $("#indexerLookupType").value;
  const value = $("#indexerLookupValue").value.trim();
  const limit = Math.max(1, Math.min(100, Number($("#indexerLookupLimitInput").value || 20)));
  const offset = Math.max(0, Number($("#indexerOffsetInput").value || 0));
  if (!value && type !== "blocks") throw new Error("lookup value is required");

  if (type === "block-id") {
    const json = await indexerRpc("getBlockById", [Number(value)]);
    const block = json.response.result;
    if (!block) target.innerHTML = `<div class="empty">Block not found</div>`;
    else renderIndexerBlocks(target, [block]);
    return;
  }
  if (type === "block-hash") {
    const json = await indexerRpc("getBlockByHash", [value]);
    const block = json.response.result;
    if (!block) target.innerHTML = `<div class="empty">Block not found</div>`;
    else renderIndexerBlocks(target, [block]);
    return;
  }
  if (type === "transaction") {
    const json = await indexerRpc("getTransaction", [value]);
    renderIndexerTransactionResult(target, json.response.result);
    return;
  }
  if (type === "account") {
    const json = await indexerRpc("getAccount", [value]);
    target.innerHTML = json.response.result
      ? renderStructuredResult(json.response.result, "Account")
      : `<div class="empty">Account not found</div>`;
    return;
  }
  if (type === "account-transactions") {
    const json = await indexerRpc("getTransactionsByAccount", [value, offset, limit]);
    const txs = json.response.result || [];
    target.innerHTML = txs.length
      ? `${renderTransactions(txs.map(flattenIndexerTransaction))}${renderJsonDetails(txs, "Raw transactions")}`
      : `<div class="empty">No transactions</div>`;
  }
}

function bindEvents() {
  $$(".nav-item").forEach((button) => {
    button.addEventListener("click", () => activateView(button.dataset.view));
  });

  $("#endpointForm").addEventListener("submit", async (event) => {
    event.preventDefault();
    state.endpoint = endpoint();
    state.indexerEndpoint = indexerEndpoint();
    saveEndpoints();
    try {
      await Promise.allSettled([refreshHead(), refreshIndexerHead()]);
      toast("Connected");
    } catch (error) {
      toast(error.message);
    }
  });

  $("#refreshBtn").addEventListener("click", async (event) => {
    setBusy(event.currentTarget, true);
    try {
      await Promise.allSettled([refreshHead(), refreshIndexerHead()]);
      toast("Refreshed");
    } catch (error) {
      toast(error.message);
    } finally {
      setBusy(event.currentTarget, false);
    }
  });

  $("#buildBtn").addEventListener("click", async (event) => {
    setBusy(event.currentTarget, true);
    try {
      const result = await post("/api/build", {});
      $("#cliMetric").textContent = result.ok ? "ready" : "failed";
      $("#cliState").textContent = result.ok ? "cli ready" : "cli failed";
      toast(result.ok ? "CLI built" : "Build failed");
    } catch (error) {
      toast(error.message);
    } finally {
      setBusy(event.currentTarget, false);
    }
  });

  $("#overviewWindowForm").addEventListener("submit", async (event) => {
    event.preventDefault();
    try {
      await scanHeadWindow({ rail: $("#boundaryRail"), rows: $("#boundaryRows") });
    } catch (error) {
      toast(error.message);
    }
  });

  $("#blockForm").addEventListener("submit", async (event) => {
    event.preventDefault();
    try {
      await inspectBlock($("#blockIdInput").value || state.latestHead);
    } catch (error) {
      resultError($("#blockResult"), error);
    }
  });

  $("#prevBlockBtn").addEventListener("click", () => inspectBlock(Math.max(1, (state.currentBlock || state.latestHead || 1) - 1)).catch((error) => resultError($("#blockResult"), error)));
  $("#nextBlockBtn").addEventListener("click", () => inspectBlock((state.currentBlock || state.latestHead || 1) + 1).catch((error) => resultError($("#blockResult"), error)));

  $("#rangeForm").addEventListener("submit", async (event) => {
    event.preventDefault();
    try {
      await scanRange(Number($("#rangeStartInput").value), Number($("#rangeEndInput").value));
    } catch (error) {
      toast(error.message);
    }
  });

  $("#scanCurrentHeadBtn").addEventListener("click", async () => {
    try {
      await scanHeadWindow();
    } catch (error) {
      toast(error.message);
    }
  });

  document.addEventListener("click", (event) => {
    const sequencerTarget = event.target.closest("[data-block]");
    if (sequencerTarget) {
      activateView("blocks");
      inspectBlock(Number(sequencerTarget.dataset.block)).catch((error) => resultError($("#blockResult"), error));
      return;
    }
    const indexerTarget = event.target.closest("[data-indexer-block]");
    if (indexerTarget) {
      activateView("indexer");
      $("#indexerLookupType").value = "block-id";
      $("#indexerLookupValue").value = indexerTarget.dataset.indexerBlock;
      runIndexerLookup().catch((error) => resultError($("#indexerLookupResult"), error));
      return;
    }
    const removeIdl = event.target.closest("[data-remove-idl]");
    if (removeIdl) {
      delete state.idls[removeIdl.dataset.removeIdl];
      saveIdls();
      renderIdlList();
    }
  });

  $("#txForm").addEventListener("submit", async (event) => {
    event.preventDefault();
    const target = $("#txResult");
    resultLoading(target, "Fetching transaction");
    try {
      const json = await post("/api/tx", { endpoint: endpoint(), hash: $("#txHashInput").value.trim() });
      const values = json.parsed.values;
      target.innerHTML = `
        ${renderKv(values)}
        ${values.kind === "Public" ? renderInstructionDecode(decodeInstruction(values.program_id_hex, values.instruction_data, values.account_ids), values.program_id_hex) : ""}
        ${renderRawDetails(json.raw, "Raw transaction")}
      `;
    } catch (error) {
      resultError(target, error);
    }
  });

  $("#indexerTxForm").addEventListener("submit", async (event) => {
    event.preventDefault();
    const target = $("#indexerTxResult");
    resultLoading(target, "Fetching transaction");
    try {
      const json = await indexerRpc("getTransaction", [$("#indexerTxHashInput").value.trim()]);
      renderIndexerTransactionResult(target, json.response.result);
    } catch (error) {
      resultError(target, error);
    }
  });

  $("#accountForm").addEventListener("submit", async (event) => {
    event.preventDefault();
    const target = $("#accountResult");
    resultLoading(target, "Fetching account");
    try {
      if ($("#accountSource").value === "indexer") {
        const json = await indexerRpc("getAccount", [$("#accountIdInput").value.trim()]);
        renderAccountInspection(target, json.response.result);
        return;
      }
      const json = await post("/api/account", {
        endpoint: endpoint(),
        accountId: $("#accountIdInput").value.trim(),
        decoder: "account-json"
      });
      if (!json.json) {
        target.innerHTML = `${renderStructuredResult({}, "Account")}${renderRawDetails(json.raw, "Raw account")}`;
        return;
      }
      renderAccountInspection(target, json.json, json.raw);
    } catch (error) {
      resultError(target, error);
    }
  });

  $("#clearAccountBtn").addEventListener("click", () => {
    $("#accountIdInput").value = "";
    $("#accountResult").innerHTML = "";
  });

  $("#programFileForm").addEventListener("submit", async (event) => {
    event.preventDefault();
    const target = $("#programFileResult");
    resultLoading(target, "Running");
    try {
      const json = await post("/api/program-file", {
        mode: $("#programFileMode").value,
        path: $("#programPathInput").value.trim()
      });
      target.innerHTML = renderProgramFileResult(json);
    } catch (error) {
      resultError(target, error);
    }
  });

  $("#loadProgramIdsBtn").addEventListener("click", () => loadProgramIds().catch((error) => toast(error.message)));
  $("#loadProgramsPanelBtn").addEventListener("click", () => loadProgramIds().catch((error) => toast(error.message)));

  $("#indexerStatusBtn").addEventListener("click", () => loadIndexerStatus().catch((error) => resultError($("#indexerStatusResult"), error)));
  $("#indexerBlocksForm").addEventListener("submit", (event) => {
    event.preventDefault();
    loadIndexerBlocks().catch((error) => resultError($("#indexerBlocksResult"), error));
  });
  $("#indexerLookupForm").addEventListener("submit", (event) => {
    event.preventDefault();
    runIndexerLookup().catch((error) => resultError($("#indexerLookupResult"), error));
  });

  $("#idlFileInput").addEventListener("change", async (event) => {
    const [file] = event.target.files || [];
    if (!file) return;
    $("#idlText").value = await file.text();
  });

  $("#loadArtifactBtn").addEventListener("click", async () => {
    const selected = $("#idlArtifactSelect").value;
    if (!selected) return;
    try {
      const json = await post("/api/idl-file", { path: selected });
      $("#idlText").value = JSON.stringify(json.idl, null, 2);
      $("#idlResult").innerHTML = renderKv({
        loaded: json.path,
        name: json.idl.name,
        instructions: json.idl.instructions?.length || 0,
        accounts: json.idl.accounts?.length || 0
      });
    } catch (error) {
      resultError($("#idlResult"), error);
    }
  });

  $("#idlForm").addEventListener("submit", (event) => {
    event.preventDefault();
    try {
      const programId = $("#idlProgramInput").value.trim();
      const idl = JSON.parse($("#idlText").value || "{}");
      if (!Array.isArray(idl.instructions) && !Array.isArray(idl.accounts)) {
        throw new Error("IDL must contain instructions or accounts");
      }
      const key = programId
        ? programKey(programId)
        : `idl:${idl.name || "anonymous"}:${Date.now()}`;
      state.idls[key] = { programId, idl, savedAt: new Date().toISOString() };
      saveIdls();
      renderIdlList();
      $("#idlResult").innerHTML = renderKv({
        mapped_program: programId || "(local)",
        canonical_key: key,
        idl: idl.name || "-",
        instructions: idl.instructions?.length || 0,
        accounts: idl.accounts?.length || 0
      });
      toast("IDL saved");
    } catch (error) {
      resultError($("#idlResult"), error);
    }
  });

  $("#clearIdlsBtn").addEventListener("click", () => {
    state.idls = {};
    saveIdls();
    renderIdlList();
  });

  $("#rpcSource").addEventListener("change", () => setRpcSample("head"));
  $$("[data-rpc-sample]").forEach((button) => {
    button.addEventListener("click", () => setRpcSample(button.dataset.rpcSample));
  });

  $("#rpcForm").addEventListener("submit", async (event) => {
    event.preventDefault();
    const target = $("#rpcResult");
    resultLoading(target, "Sending");
    try {
      const params = JSON.parse($("#rpcParams").value || "[]");
      const source = $("#rpcSource").value;
      const json = source === "indexer"
        ? await indexerRpc($("#rpcMethod").value.trim(), params)
        : await sequencerRpc($("#rpcMethod").value.trim(), params);
      target.innerHTML = renderRpcResponse(json.response);
    } catch (error) {
      resultError(target, error);
    }
  });

  $("#cliCommand").addEventListener("change", updateCommandPreview);
  $("#cliArgs").addEventListener("input", updateCommandPreview);

  $("#copyCommandBtn").addEventListener("click", async () => {
    await navigator.clipboard.writeText($("#commandPreview").textContent);
    toast("Command copied");
  });

  $("#cliForm").addEventListener("submit", async (event) => {
    event.preventDefault();
    const target = $("#cliResult");
    resultLoading(target, "Running command");
    try {
      const json = await post("/api/cli", {
        command: $("#cliCommand").value,
        args: splitArgs($("#cliArgs").value)
      });
      target.innerHTML = renderCliResult(json);
    } catch (error) {
      resultError(target, error);
    }
  });
}

function initCommandSelect() {
  $("#cliCommand").innerHTML = commands.map(([command]) => `
    <option value="${escapeHtml(command)}">${escapeHtml(command)}</option>
  `).join("");
  $("#cliArgs").placeholder = commands[0][1];
  $("#cliCommand").addEventListener("change", () => {
    const selected = commands.find(([command]) => command === $("#cliCommand").value);
    $("#cliArgs").placeholder = selected?.[1] || "";
  });
  updateCommandPreview();
}

async function init() {
  $("#endpointInput").value = state.endpoint;
  $("#indexerEndpointInput").value = state.indexerEndpoint;
  bindEvents();
  initCommandSelect();
  renderIdlList();
  await loadArtifacts().catch((error) => toast(error.message));
  try {
    await refreshStatus();
    await scanHeadWindow({ rail: $("#boundaryRail"), rows: $("#boundaryRows") });
    await loadProgramIds();
    await loadIndexerStatus($("#indexerStatusResult"));
  } catch (error) {
    toast(error.message);
  }
}

init();
