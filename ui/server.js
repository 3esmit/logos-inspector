import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { createReadStream } from "node:fs";
import { mkdir, mkdtemp, rm, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { findCargoBinary } from "./cargo-artifacts.js";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const publicDir = path.join(__dirname, "public");
const repoRoot = path.resolve(__dirname, "..");
const configuredCli = process.env.LOGOS_INSPECTOR_CLI
  ? path.resolve(process.env.LOGOS_INSPECTOR_CLI)
  : null;
const port = Number(process.env.PORT || 8787);
const host = process.env.HOST || "127.0.0.1";

const allowedCommands = new Set([
  "overview",
  "health",
  "head",
  "programs",
  "block",
  "tx",
  "inspect-tx",
  "trace-tx",
  "account",
  "decode-account",
  "decode-instruction",
  "decode-event",
  "program-file",
  "blockchain-node",
  "blockchain-blocks",
  "logoscore-status",
  "source-policy",
  "modules",
  "blockchain-module",
  "storage",
  "messaging",
  "capabilities",
  "channels",
  "spel-idl",
  "rpc"
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

async function readBody(req) {
  const chunks = [];
  let size = 0;
  for await (const chunk of req) {
    size += chunk.length;
    if (size > 4_000_000) throw new Error("request body too large");
    chunks.push(chunk);
  }
  if (!chunks.length) return {};
  const raw = Buffer.concat(chunks).toString("utf8");
  return raw ? JSON.parse(raw) : {};
}

async function cliPath() {
  return findCargoBinary({
    repoRoot,
    binaryName: process.platform === "win32" ? "logos-inspector.exe" : "logos-inspector",
    configuredCli
  });
}

function runProcess(command, args, options = {}) {
  return new Promise((resolve) => {
    const child = spawn(command, args, {
      cwd: repoRoot,
      env: {
        ...process.env,
        RISC0_SKIP_BUILD: process.env.RISC0_SKIP_BUILD || "1",
        ...(options.env || {})
      },
      shell: false
    });
    let stdout = "";
    let stderr = "";
    let timedOut = false;
    const maxOutput = options.maxOutput ?? 12_000_000;
    const timeout = setTimeout(() => {
      timedOut = true;
      child.kill("SIGTERM");
    }, options.timeoutMs ?? 120_000);

    child.stdout.on("data", (data) => {
      if (stdout.length < maxOutput) stdout += data.toString("utf8");
    });
    child.stderr.on("data", (data) => {
      if (stderr.length < maxOutput) stderr += data.toString("utf8");
    });
    child.on("close", (code) => {
      clearTimeout(timeout);
      resolve({ code, stdout, stderr, timedOut });
    });
    child.on("error", (error) => {
      clearTimeout(timeout);
      resolve({ code: 127, stdout, stderr: String(error), timedOut });
    });
  });
}

async function buildCli() {
  return runProcess("cargo", ["build"], {
    timeoutMs: 240_000,
    maxOutput: 12_000_000
  });
}

async function getCli({ autoBuild = true } = {}) {
  let binary = await cliPath();
  if (!binary && autoBuild) {
    const build = await buildCli();
    binary = await cliPath();
    if (!binary) {
      const error = new Error("logos-inspector binary missing after build");
      error.details = build;
      throw error;
    }
  }
  if (!binary) throw new Error("logos-inspector binary not built");
  return binary;
}

function parseJsonMaybe(stdout) {
  const trimmed = stdout.trim();
  if (!trimmed) return null;
  try {
    return JSON.parse(trimmed);
  } catch {
    return null;
  }
}

function assertAllowed(command) {
  if (!allowedCommands.has(command)) {
    throw new Error(`command not allowed: ${command}`);
  }
}

async function runCli(command, args = [], options = {}) {
  assertAllowed(command);
  if (!Array.isArray(args)) throw new Error("args must be an array");
  const binary = await getCli({ autoBuild: options.autoBuild !== false });
  const cliArgs = ["cli", command, ...args.map((arg) => String(arg))];
  const raw = await runProcess(binary, cliArgs, {
    timeoutMs: options.timeoutMs ?? 120_000,
    maxOutput: options.maxOutput ?? 12_000_000
  });
  const json = parseJsonMaybe(raw.stdout);
  return {
    ok: raw.code === 0 && !raw.timedOut,
    command,
    args,
    json,
    stdout: raw.stdout,
    stderr: raw.stderr,
    code: raw.code,
    timedOut: raw.timedOut
  };
}

async function runSpelIdl(idlJson) {
  const tmp = await mkdtemp(path.join(os.tmpdir(), "logos-spel-idl-"));
  const file = path.join(tmp, "idl.json");
  try {
    await writeFile(file, String(idlJson || ""), "utf8");
    return await runCli("spel-idl", [file], { maxOutput: 16_000_000 });
  } finally {
    await rm(tmp, { recursive: true, force: true });
  }
}

async function serveStatic(req, res) {
  const url = new URL(req.url, `http://${host}:${port}`);
  const pathname = decodeURIComponent(url.pathname);
  const target = pathname === "/" ? "/index.html" : pathname;
  const fullPath = path.resolve(publicDir, `.${target}`);
  if (!fullPath.startsWith(`${publicDir}${path.sep}`)) {
    sendText(res, 403, "forbidden");
    return;
  }
  try {
    await stat(fullPath);
  } catch {
    sendText(res, 404, "not found");
    return;
  }
  const contentType = contentTypes[path.extname(fullPath)] || "application/octet-stream";
  res.writeHead(200, { "content-type": contentType });
  createReadStream(fullPath).pipe(res);
}

async function handleApi(req, res) {
  const url = new URL(req.url, `http://${host}:${port}`);
  if (req.method === "GET" && url.pathname === "/api/status") {
    const binary = await cliPath();
    sendJson(res, 200, {
      ok: true,
      binary,
      built: Boolean(binary),
      commands: [...allowedCommands].sort()
    });
    return;
  }

  if (req.method === "POST" && url.pathname === "/api/build") {
    const raw = await buildCli();
    sendJson(res, raw.code === 0 ? 200 : 500, {
      ok: raw.code === 0,
      raw
    });
    return;
  }

  if (req.method === "POST" && url.pathname === "/api/cli") {
    const body = await readBody(req);
    const result = await runCli(body.command, body.args || []);
    sendJson(res, result.ok ? 200 : 500, result);
    return;
  }

  if (req.method === "POST" && url.pathname === "/api/spel-idl") {
    const body = await readBody(req);
    const result = await runSpelIdl(body.idlJson);
    sendJson(res, result.ok ? 200 : 500, result);
    return;
  }

  sendJson(res, 404, { ok: false, error: "api route not found" });
}

const server = createServer((req, res) => {
  Promise.resolve()
    .then(async () => {
      await mkdir(publicDir, { recursive: true });
      if (req.url?.startsWith("/api/")) {
        await handleApi(req, res);
      } else {
        await serveStatic(req, res);
      }
    })
    .catch((error) => {
      sendJson(res, 500, {
        ok: false,
        error: error.message,
        details: error.details
      });
    });
});

server.listen(port, host, () => {
  process.stdout.write(`Logos Inspector webapp: http://${host}:${port}\n`);
});
