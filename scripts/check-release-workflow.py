#!/usr/bin/env python3
"""Check the static contracts of the manual alpha release workflow."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github" / "workflows" / "release-alpha.yml"
CHANGELOG = ROOT / "CHANGELOG.md"
PROCESS = ROOT / "docs" / "release-process.md"
ARTIFACT_TOOL = ROOT / "scripts" / "release_artifacts.py"


def require_text(path: Path, needles: tuple[str, ...], errors: list[str]) -> str:
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as error:
        errors.append(f"failed to read {path.relative_to(ROOT)}: {error}")
        return ""
    for needle in needles:
        if needle not in text:
            errors.append(f"{path.relative_to(ROOT)} is missing `{needle}`")
    return text


def main() -> int:
    errors: list[str] = []
    workflow = require_text(
        WORKFLOW,
        (
            "workflow_dispatch:",
            "ubuntu-24.04",
            "macos-14",
            "x86_64-linux",
            "aarch64-darwin",
            "cachix/install-nix-action@v31",
            "actions/upload-artifact@v4",
            "actions/download-artifact@v5",
            "contents: write",
            "gh release create",
            "--draft",
            "gh release edit",
            "--prerelease",
            "--latest=false",
            "catalog_e2e_evidence_url",
            "https://raw.githubusercontent.com/3esmit/logos-3esmit-release/main/logos-repo.json",
            "release_artifacts.py",
            "check-build-pipeline.py identity",
        ),
        errors,
    )
    if "push:" in workflow:
        errors.append("release workflow must remain manual while the project is in alpha")
    for forbidden in ("result-standalone", "--standalone-dir"):
        if forbidden in workflow:
            errors.append(f"release workflow must not publish the current non-self-contained standalone package ({forbidden})")

    changelog = require_text(CHANGELOG, ("# Changelog", "## [Unreleased]"), errors)
    process = require_text(
        PROCESS,
        ("Alpha", "Beta", "manual", "CHANGELOG.md", "GitHub Release"),
        errors,
    )
    if changelog and process and "## Release artifacts" not in process:
        errors.append("release process must document the release artifact contract")

    source = subprocess.run(
        [sys.executable, str(ARTIFACT_TOOL), "validate-source", "--root", str(ROOT)],
        check=False,
        capture_output=True,
        text=True,
    )
    if source.returncode != 0:
        errors.append(source.stderr.strip() or "source version validation failed")
    else:
        version = source.stdout.strip()
        if f"## [{version}]" not in changelog:
            errors.append(f"CHANGELOG.md must include a section for source version {version}")

    fixture = subprocess.run(
        [sys.executable, str(ARTIFACT_TOOL), "self-test"],
        check=False,
        capture_output=True,
        text=True,
    )
    if fixture.returncode != 0:
        errors.append(fixture.stderr.strip() or "release artifact fixture test failed")

    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
