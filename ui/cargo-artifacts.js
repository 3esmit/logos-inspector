import { readFile, stat } from "node:fs/promises";
import path from "node:path";

export async function findCargoBinary({
  repoRoot,
  binaryName,
  configuredCli = null,
  env = process.env
}) {
  if (configuredCli && await exists(configuredCli)) {
    return configuredCli;
  }

  for (const candidate of await cargoBinaryCandidates({ repoRoot, binaryName, env })) {
    if (await exists(candidate)) {
      return candidate;
    }
  }
  return null;
}

export async function cargoBinaryCandidates({ repoRoot, binaryName, env = process.env }) {
  const targetDir = await cargoTargetDir(repoRoot, env);
  return [
    path.join(targetDir, "debug", binaryName),
    path.join(targetDir, "release", binaryName)
  ];
}

export async function cargoTargetDir(repoRoot, env = process.env) {
  if (env.CARGO_TARGET_DIR) {
    return path.resolve(repoRoot, env.CARGO_TARGET_DIR);
  }

  const configured = await cargoConfigTargetDir(repoRoot);
  if (configured) {
    return path.resolve(repoRoot, configured);
  }

  return path.join(repoRoot, "target");
}

export function parseCargoBuildTargetDir(configText) {
  let inBuildSection = false;
  for (const rawLine of String(configText || "").split(/\r?\n/)) {
    const line = stripTomlComment(rawLine).trim();
    if (!line) {
      continue;
    }
    const section = line.match(/^\[([^\]]+)\]$/);
    if (section) {
      inBuildSection = section[1].trim() === "build";
      continue;
    }
    if (!inBuildSection) {
      continue;
    }
    const match = line.match(/^target-dir\s*=\s*(.+)$/);
    if (!match) {
      continue;
    }
    return unquoteTomlValue(match[1].trim());
  }
  return null;
}

async function cargoConfigTargetDir(repoRoot) {
  try {
    const configText = await readFile(path.join(repoRoot, ".cargo", "config.toml"), "utf8");
    return parseCargoBuildTargetDir(configText);
  } catch {
    return null;
  }
}

function stripTomlComment(line) {
  let quoted = false;
  let quote = "";
  for (let index = 0; index < line.length; index += 1) {
    const char = line[index];
    if ((char === "\"" || char === "'") && (index === 0 || line[index - 1] !== "\\")) {
      if (!quoted) {
        quoted = true;
        quote = char;
      } else if (quote === char) {
        quoted = false;
      }
    }
    if (char === "#" && !quoted) {
      return line.slice(0, index);
    }
  }
  return line;
}

function unquoteTomlValue(value) {
  const first = value[0];
  const last = value[value.length - 1];
  if ((first === "\"" && last === "\"") || (first === "'" && last === "'")) {
    return value.slice(1, -1);
  }
  return value;
}

async function exists(file) {
  try {
    await stat(file);
    return true;
  } catch {
    return false;
  }
}
