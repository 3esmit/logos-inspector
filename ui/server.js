import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdir, mkdtemp, readFile, readdir, rm, stat, writeFile } from "node:fs/promises";
import { createReadStream } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const publicDir = path.join(__dirname, "public");
const repoRoot = path.resolve(__dirname, "..");
const cliManifest = path.resolve(repoRoot, "crates/lez-inspect/Cargo.toml");
const cliDebug = path.resolve(repoRoot, "target/debug/lez-inspect");
const cliRelease = path.resolve(repoRoot, "target/release/lez-inspect");
const configuredCli = process.env.LEZ_INSPECT_CLI
  ? path.resolve(process.env.LEZ_INSPECT_CLI)
  : null;
const defaultSequencerEndpoint = process.env.LEZ_SEQUENCER_ENDPOINT || "https://testnet.lez.logos.co/";
const defaultIndexerEndpoint = process.env.LEZ_INDEXER_ENDPOINT || "http://127.0.0.1:8779/";
const idlDir = path.resolve(process.env.LEZ_IDL_DIR || path.join(repoRoot, "artifacts"));
const port = Number(process.env.PORT || 8787);
const host = process.env.HOST || "127.0.0.1";

const allowedCliCommands = new Set([
  "hash-deploy",
  "decode-block",
  "decode-block-range",
  "fetch-tx",
  "find-tx-block",
  "account-json",
  "account-data-hex",
  "program-id",
  "create-public-account",
  "strip-r0bf"
]);

const contentTypes = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
  ".ico": "image/x-icon"
};

function sendJson(res, status, value) {
  const body = JSON.stringify(value);
  res.writeHead(status, {
    "content-type": "application/json; charset=utf-8",
    "content-length": Buffer.byteLength(body)
  });
  res.end(body);
}

function sendText(res, status, text) {
  res.writeHead(status, {
    "content-type": "text/plain; charset=utf-8",
    "content-length": Buffer.byteLength(text)
  });
  res.end(text);
}

function normalizeEndpoint(endpoint) {
  const raw = String(endpoint || defaultSequencerEndpoint).trim();
  const url = new URL(raw);
  if (url.protocol !== "https:" && url.protocol !== "http:") {
    throw new Error("endpoint must be http or https");
  }
  return url.toString();
}

async function readBody(req) {
  const chunks = [];
  let size = 0;
  for await (const chunk of req) {
    size += chunk.length;
    if (size > 2_000_000) {
      throw new Error("request body too large");
    }
    chunks.push(chunk);
  }
  if (chunks.length === 0) {
    return {};
  }
  const raw = Buffer.concat(chunks).toString("utf8");
  return raw ? JSON.parse(raw) : {};
}

async function exists(file) {
  try {
    await stat(file);
    return true;
  } catch {
    return false;
  }
}

async function cliPath() {
  if (configuredCli && await exists(configuredCli)) return configuredCli;
  if (await exists(cliDebug)) return cliDebug;
  if (await exists(cliRelease)) return cliRelease;
  return null;
}

function runProcess(command, args, options = {}) {
  return new Promise((resolve) => {
    const child = spawn(command, args, {
      cwd: repoRoot,
      env: { ...process.env, ...(options.env || {}) },
      shell: false
    });
    let stdout = "";
    let stderr = "";
    let killed = false;
    const max = options.maxOutput ?? 8_000_000;
    const timeout = setTimeout(() => {
      killed = true;
      child.kill("SIGTERM");
    }, options.timeoutMs ?? 60_000);

    child.stdout.on("data", (data) => {
      if (stdout.length < max) stdout += data.toString("utf8");
    });
    child.stderr.on("data", (data) => {
      if (stderr.length < max) stderr += data.toString("utf8");
    });
    child.on("close", (code) => {
      clearTimeout(timeout);
      resolve({ code, stdout, stderr, timedOut: killed });
    });
    child.on("error", (error) => {
      clearTimeout(timeout);
      resolve({ code: 127, stdout, stderr: String(error), timedOut: killed });
    });
  });
}

async function buildCli() {
  return runProcess("cargo", ["build", "--manifest-path", cliManifest], {
    timeoutMs: 180_000,
    maxOutput: 6_000_000
  });
}

async function getCli({ autoBuild = true } = {}) {
  let binary = await cliPath();
  if (!binary && autoBuild) {
    const build = await buildCli();
    binary = await cliPath();
    if (!binary) {
      const error = new Error("lez-inspect binary missing after build");
      error.details = build;
      throw error;
    }
  }
  if (!binary) {
    throw new Error("lez-inspect binary not built");
  }
  return binary;
}

async function runCli(args, options = {}) {
  if (!allowedCliCommands.has(args[0])) {
    throw new Error(`command not allowed: ${args[0]}`);
  }
  const binary = await getCli({ autoBuild: options.autoBuild !== false });
  return runProcess(binary, args, {
    timeoutMs: options.timeoutMs ?? 90_000,
    maxOutput: options.maxOutput ?? 8_000_000,
    env: options.env
  });
}

async function rpc(endpoint, method, params = []) {
  const response = await fetch(normalizeEndpoint(endpoint), {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      jsonrpc: "2.0",
      id: Date.now(),
      method,
      params
    })
  });
  const text = await response.text();
  let json;
  try {
    json = JSON.parse(text);
  } catch {
    throw new Error(`invalid JSON-RPC response: ${text.slice(0, 400)}`);
  }
  if (!response.ok) {
    throw new Error(`HTTP ${response.status}: ${text.slice(0, 400)}`);
  }
  return json;
}

async function listIdlArtifacts() {
  if (!await exists(idlDir)) return [];
  const names = await readdir(idlDir);
  return names
    .filter((name) => name.endsWith("-idl.json"))
    .sort()
    .map((name) => ({
      name,
      path: name
    }));
}

function artifactPath(relativePath) {
  const raw = String(relativePath || "").trim();
  if (!raw) throw new Error("path is required");
  const fullPath = path.resolve(idlDir, raw);
  if (!fullPath.startsWith(`${idlDir}${path.sep}`) || !fullPath.endsWith(".json")) {
    throw new Error("IDL path must be under LEZ_IDL_DIR and end with .json");
  }
  return fullPath;
}

function parseCliOutput(stdout) {
  const lines = stdout.split(/\r?\n/).filter(Boolean);
  const values = {};
  const transactions = [];
  for (const line of lines) {
    const txMatch = /^tx\[(\d+)]\.([^=]+)=(.*)$/.exec(line);
    if (txMatch) {
      const index = Number(txMatch[1]);
      transactions[index] ||= { index };
      transactions[index][txMatch[2]] = txMatch[3];
      continue;
    }
    const split = line.indexOf("=");
    if (split > 0) {
      values[line.slice(0, split)] = line.slice(split + 1);
    }
  }
  return { values, transactions: transactions.filter(Boolean), lines };
}

async function decodeRpcBlockJson(json) {
  const tmp = await mkdtemp(path.join(os.tmpdir(), "lez-block-"));
  const file = path.join(tmp, "block.json");
  try {
    await writeFile(file, JSON.stringify(json), "utf8");
    const result = await runCli(["decode-block", file], { maxOutput: 12_000_000 });
    return { ...parseCliOutput(result.stdout), raw: result };
  } finally {
    await rm(tmp, { recursive: true, force: true });
  }
}

async function decodeRpcBlockRangeJson(json) {
  const tmp = await mkdtemp(path.join(os.tmpdir(), "lez-range-"));
  const file = path.join(tmp, "range.json");
  try {
    await writeFile(file, JSON.stringify(json), "utf8");
    const result = await runCli(["decode-block-range", file], { maxOutput: 24_000_000 });
    const lines = result.stdout.split(/\r?\n/).filter(Boolean);
    const blocks = [];
    let current = null;
    for (const line of lines) {
      if (line.startsWith("block_id=")) {
        if (current) blocks.push(current);
        current = { values: {}, transactions: [], lines: [] };
      }
      if (!current) continue;
      current.lines.push(line);
      const txMatch = /^tx\[(\d+)]\.([^=]+)=(.*)$/.exec(line);
      if (txMatch) {
        const index = Number(txMatch[1]);
        current.transactions[index] ||= { index };
        current.transactions[index][txMatch[2]] = txMatch[3];
        continue;
      }
      const split = line.indexOf("=");
      if (split > 0) current.values[line.slice(0, split)] = line.slice(split + 1);
    }
    if (current) blocks.push(current);
    return { blocks, raw: result };
  } finally {
    await rm(tmp, { recursive: true, force: true });
  }
}

function integer(value, name, min = 0, max = Number.MAX_SAFE_INTEGER) {
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < min || parsed > max) {
    throw new Error(`${name} must be an integer from ${min} to ${max}`);
  }
  return parsed;
}

function stringList(value) {
  if (!Array.isArray(value)) return [];
  return value.map((item) => String(item));
}

async function routeApi(req, res, pathname) {
  try {
    if (req.method === "GET" && pathname === "/api/status") {
      const binary = await cliPath();
      sendJson(res, 200, {
        ok: true,
        defaultEndpoint: defaultSequencerEndpoint,
        defaultSequencerEndpoint,
        defaultIndexerEndpoint,
        idlDir,
        cliReady: Boolean(binary),
        cliPath: binary,
        repoRoot,
        uptimeSeconds: Math.round(process.uptime())
      });
      return;
    }

    if (req.method === "GET" && pathname === "/api/idls") {
      sendJson(res, 200, { ok: true, artifacts: await listIdlArtifacts() });
      return;
    }

    if (req.method === "POST" && pathname === "/api/build") {
      const result = await buildCli();
      sendJson(res, result.code === 0 ? 200 : 500, {
        ok: result.code === 0,
        ...result,
        cliPath: await cliPath()
      });
      return;
    }

    const body = await readBody(req);
    const endpoint = normalizeEndpoint(body.endpoint || defaultSequencerEndpoint);

    if (req.method === "POST" && pathname === "/api/rpc") {
      const method = String(body.method || "");
      const params = Array.isArray(body.params) ? body.params : [];
      if (!method) throw new Error("method is required");
      sendJson(res, 200, { ok: true, response: await rpc(endpoint, method, params) });
      return;
    }

    if (req.method === "POST" && pathname === "/api/indexer-rpc") {
      const indexerEndpoint = normalizeEndpoint(body.endpoint || defaultIndexerEndpoint);
      const method = String(body.method || "");
      const params = Array.isArray(body.params) ? body.params : [];
      if (!method) throw new Error("method is required");
      sendJson(res, 200, {
        ok: true,
        response: await rpc(indexerEndpoint, method, params)
      });
      return;
    }

    if (req.method === "POST" && pathname === "/api/block") {
      const blockId = integer(body.blockId, "blockId", 1);
      const response = await rpc(endpoint, "getBlock", [blockId]);
      const decoded = await decodeRpcBlockJson(response);
      sendJson(res, 200, { ok: true, endpoint, rpc: response, decoded });
      return;
    }

    if (req.method === "POST" && pathname === "/api/block-range") {
      const start = integer(body.start, "start", 1);
      const end = integer(body.end, "end", 1);
      if (end < start) throw new Error("end must be greater than or equal to start");
      if (end - start > 50) throw new Error("range limit is 51 blocks");
      const response = await rpc(endpoint, "getBlockRange", [start, end]);
      const decoded = await decodeRpcBlockRangeJson(response);
      sendJson(res, 200, { ok: true, endpoint, rpc: response, decoded });
      return;
    }

    if (req.method === "POST" && pathname === "/api/tx") {
      const hash = String(body.hash || "").trim();
      if (!hash) throw new Error("hash is required");
      const result = await runCli(["fetch-tx", hash, endpoint]);
      sendJson(res, result.code === 0 ? 200 : 404, {
        ok: result.code === 0,
        parsed: parseCliOutput(result.stdout),
        raw: result
      });
      return;
    }

    if (req.method === "POST" && pathname === "/api/account") {
      const accountId = String(body.accountId || "").trim();
      const decoder = String(body.decoder || "account-json");
      if (!accountId) throw new Error("accountId is required");
      const args = (() => {
        if (decoder === "account-data-hex") return ["account-data-hex", accountId, endpoint];
        return ["account-json", accountId, endpoint];
      })();
      const result = await runCli(args);
      let json = null;
      try {
        json = JSON.parse(result.stdout);
      } catch {
        json = null;
      }
      sendJson(res, result.code === 0 ? 200 : 500, {
        ok: result.code === 0,
        json,
        parsed: parseCliOutput(result.stdout),
        raw: result
      });
      return;
    }

    if (req.method === "POST" && pathname === "/api/program-file") {
      const mode = String(body.mode || "");
      const filePath = String(body.path || "").trim();
      if (!filePath) throw new Error("path is required");
      const command = mode === "hash-deploy" ? "hash-deploy" : "program-id";
      const result = await runCli([command, filePath]);
      sendJson(res, result.code === 0 ? 200 : 500, {
        ok: result.code === 0,
        parsed: parseCliOutput(result.stdout),
        raw: result
      });
      return;
    }

    if (req.method === "POST" && pathname === "/api/idl-file") {
      const fullPath = artifactPath(body.path);
      const raw = await readFile(fullPath, "utf8");
      const idl = JSON.parse(raw);
      sendJson(res, 200, {
        ok: true,
        path: path.relative(repoRoot, fullPath),
        idl
      });
      return;
    }

    if (req.method === "POST" && pathname === "/api/cli") {
      const command = String(body.command || "");
      const args = stringList(body.args);
      if (!allowedCliCommands.has(command)) {
        throw new Error(`command not allowed: ${command}`);
      }
      const result = await runCli([command, ...args], { timeoutMs: 180_000 });
      sendJson(res, result.code === 0 ? 200 : 500, {
        ok: result.code === 0,
        parsed: parseCliOutput(result.stdout),
        raw: result
      });
      return;
    }

    sendJson(res, 404, { ok: false, error: "unknown API route" });
  } catch (error) {
    sendJson(res, 500, {
      ok: false,
      error: error.message || String(error),
      details: error.details || null
    });
  }
}

async function routeStatic(req, res, pathname) {
  let requestPath = decodeURIComponent(pathname);
  if (requestPath === "/") requestPath = "/index.html";
  const fullPath = path.normalize(path.join(publicDir, requestPath));
  if (!fullPath.startsWith(publicDir)) {
    sendText(res, 403, "forbidden");
    return;
  }
  try {
    await stat(fullPath);
    const ext = path.extname(fullPath);
    res.writeHead(200, { "content-type": contentTypes[ext] || "application/octet-stream" });
    createReadStream(fullPath).pipe(res);
  } catch {
    sendText(res, 404, "not found");
  }
}

await mkdir(publicDir, { recursive: true });

const server = createServer(async (req, res) => {
  const url = new URL(req.url || "/", `http://${req.headers.host || "localhost"}`);
  if (url.pathname.startsWith("/api/")) {
    await routeApi(req, res, url.pathname);
    return;
  }
  await routeStatic(req, res, url.pathname);
});

server.listen(port, host, () => {
  console.log(`lez-inspect-ui listening on http://${host}:${port}`);
});
