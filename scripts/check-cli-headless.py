#!/usr/bin/env python3
"""Build and smoke-test the root CLI without desktop dependencies."""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from pathlib import Path

from cli_release import CliReleaseError, smoke_binary, source_version


ROOT = Path(__file__).resolve().parents[1]
FORBIDDEN_DEPENDENCIES = (
    "cxx-qt",
    "cxx-qt-build",
    "cxx-qt-lib",
    "logos-inspector-standalone-gui",
)
FEATURES = "cli,local-wallet-runtime"


def run(command: list[str], *, env: dict[str, str] | None = None) -> str:
    result = subprocess.run(
        command,
        cwd=ROOT,
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip()
        raise CliReleaseError(
            f"{' '.join(command)} failed with exit code {result.returncode}: {detail}"
        )
    return result.stdout


def main() -> int:
    try:
        tree = run(
            [
                "cargo",
                "tree",
                "--locked",
                "--package",
                "logos-inspector",
                "--no-default-features",
                "--features",
                FEATURES,
                "--edges",
                "normal",
            ]
        ).lower()
        for dependency in FORBIDDEN_DEPENDENCIES:
            if dependency in tree:
                raise CliReleaseError(
                    f"headless CLI dependency tree unexpectedly contains {dependency}"
                )

        with tempfile.TemporaryDirectory(prefix="logos-inspector-cli-check-") as temporary:
            target_dir = Path(temporary) / "target"
            env = os.environ.copy()
            env["CARGO_TARGET_DIR"] = str(target_dir)
            env["RISC0_SKIP_BUILD"] = "1"
            run(
                [
                    "cargo",
                    "build",
                    "--release",
                    "--locked",
                    "--package",
                    "logos-inspector",
                    "--no-default-features",
                    "--features",
                    FEATURES,
                    "--bin",
                    "logos-inspector",
                ],
                env=env,
            )
            smoke_binary(target_dir / "release" / "logos-inspector", source_version(ROOT))
    except CliReleaseError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
