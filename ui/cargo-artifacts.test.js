import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import assert from "node:assert/strict";

import {
  cargoBinaryCandidates,
  cargoTargetDir,
  findCargoBinary,
  parseCargoBuildTargetDir
} from "./cargo-artifacts.js";

test("parseCargoBuildTargetDir reads build target-dir", () => {
  const config = `
[profile.dev]
debug = true

[build]
target-dir = "../.cargo-target/logos-inspector" # outside repo
`;

  assert.equal(parseCargoBuildTargetDir(config), "../.cargo-target/logos-inspector");
});

test("cargoTargetDir prefers CARGO_TARGET_DIR", async () => {
  const repo = await tempRepo();
  try {
    await mkdir(path.join(repo, ".cargo"), { recursive: true });
    await writeFile(path.join(repo, ".cargo", "config.toml"), "[build]\ntarget-dir = \"from-config\"\n");

    const targetDir = await cargoTargetDir(repo, { CARGO_TARGET_DIR: "from-env" });

    assert.equal(targetDir, path.join(repo, "from-env"));
  } finally {
    await rm(repo, { recursive: true, force: true });
  }
});

test("cargoBinaryCandidates follows configured target-dir", async () => {
  const repo = await tempRepo();
  try {
    await mkdir(path.join(repo, ".cargo"), { recursive: true });
    await writeFile(path.join(repo, ".cargo", "config.toml"), "[build]\ntarget-dir = \"../shared-target\"\n");

    const candidates = await cargoBinaryCandidates({
      repoRoot: repo,
      binaryName: "logos-inspector",
      env: {}
    });

    assert.deepEqual(candidates, [
      path.resolve(repo, "../shared-target/debug/logos-inspector"),
      path.resolve(repo, "../shared-target/release/logos-inspector")
    ]);
  } finally {
    await rm(repo, { recursive: true, force: true });
  }
});

test("findCargoBinary returns configured CLI before Cargo artifacts", async () => {
  const repo = await tempRepo();
  try {
    const configured = path.join(repo, "custom-cli");
    await writeFile(configured, "");

    const found = await findCargoBinary({
      repoRoot: repo,
      binaryName: "logos-inspector",
      configuredCli: configured,
      env: {}
    });

    assert.equal(found, configured);
  } finally {
    await rm(repo, { recursive: true, force: true });
  }
});

async function tempRepo() {
  return mkdtemp(path.join(os.tmpdir(), "logos-inspector-cargo-artifacts-"));
}
