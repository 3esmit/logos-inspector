#!/usr/bin/env python3
from __future__ import annotations

import json
import sys
import tomllib
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
INHERITED_KEYS = ("version", "edition", "rust-version", "license", "publish")


def load_toml(path: Path) -> dict[str, Any]:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def inherited(package: dict[str, Any], key: str) -> bool:
    value = package.get(key)
    return isinstance(value, dict) and value.get("workspace") is True


def direct_or_workspace(package: dict[str, Any], workspace: dict[str, Any], key: str) -> Any:
    if inherited(package, key):
        return workspace.get(key)
    return package.get(key)


def main() -> int:
    errors: list[str] = []
    cargo = load_toml(ROOT / "Cargo.toml")
    workspace_package = cargo.get("workspace", {}).get("package", {})
    version = str(workspace_package.get("version", ""))

    if not version:
        errors.append("Cargo.toml missing [workspace.package].version")

    for key in INHERITED_KEYS:
        if key not in workspace_package:
            errors.append(f"Cargo.toml missing [workspace.package].{key}")

    package_manifests = [
        ROOT / "Cargo.toml",
        ROOT / "crates/core-ffi/Cargo.toml",
        ROOT / "crates/standalone-gui/Cargo.toml",
    ]
    for manifest in package_manifests:
        package = load_toml(manifest).get("package", {})
        label = manifest.relative_to(ROOT)
        for key in INHERITED_KEYS:
            if not inherited(package, key):
                errors.append(f"{label}: package.{key} must inherit workspace value")
                continue
            actual = direct_or_workspace(package, workspace_package, key)
            expected = workspace_package.get(key)
            if actual != expected:
                errors.append(f"{label}: package.{key} drifted from workspace value")

    ui_metadata = load_json(ROOT / "metadata.json")
    core_metadata = load_json(ROOT / "core/metadata.json")
    if ui_metadata.get("version") != version:
        errors.append("metadata.json version must match Cargo workspace version")
    if core_metadata.get("version") != version:
        errors.append("core/metadata.json version must match Cargo workspace version")

    core_name = str(core_metadata.get("name", ""))
    ui_dependencies = ui_metadata.get("dependencies")
    if not isinstance(ui_dependencies, list) or core_name not in ui_dependencies:
        errors.append("metadata.json dependencies must include core module name")

    external_libraries = (
        core_metadata.get("nix", {})
        .get("external_libraries", [])
    )
    external_names = [
        item.get("name")
        for item in external_libraries
        if isinstance(item, dict)
    ]
    if "logos_inspector_core" not in external_names:
        errors.append("core/metadata.json must declare logos_inspector_core external library")

    if not (ROOT / "crates/core-ffi/include/logos_inspector_core.h").is_file():
        errors.append("FFI header must live under crates/core-ffi/include")
    if (ROOT / "core/lib/logos_inspector_core.h").exists():
        errors.append("FFI header must not live under core/lib")

    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
