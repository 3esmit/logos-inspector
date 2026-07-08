from __future__ import annotations

import json
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any


INHERITED_KEYS = ("version", "edition", "rust-version", "license", "publish")
PACKAGE_MANIFESTS = (
    Path("Cargo.toml"),
    Path("crates/core-ffi/Cargo.toml"),
    Path("crates/standalone-gui/Cargo.toml"),
)
CORE_FFI_HEADER = Path("crates/core-ffi/include/logos_inspector_core.h")
LEGACY_CORE_HEADER = Path("core/lib/logos_inspector_core.h")


@dataclass(frozen=True)
class PackageIdentity:
    root: Path

    def validate(self) -> list[str]:
        errors: list[str] = []
        cargo = self.load_toml(Path("Cargo.toml"))
        workspace_package = cargo.get("workspace", {}).get("package", {})
        version = str(workspace_package.get("version", ""))

        if not version:
            errors.append("Cargo.toml missing [workspace.package].version")

        for key in INHERITED_KEYS:
            if key not in workspace_package:
                errors.append(f"Cargo.toml missing [workspace.package].{key}")

        errors.extend(self.package_manifest_errors(workspace_package))
        errors.extend(self.metadata_errors(version))
        errors.extend(self.header_errors())
        return errors

    def package_manifest_errors(self, workspace_package: dict[str, Any]) -> list[str]:
        errors: list[str] = []
        for manifest in PACKAGE_MANIFESTS:
            package = self.load_toml(manifest).get("package", {})
            for key in INHERITED_KEYS:
                if not inherited(package, key):
                    errors.append(f"{manifest}: package.{key} must inherit workspace value")
                    continue
                actual = direct_or_workspace(package, workspace_package, key)
                expected = workspace_package.get(key)
                if actual != expected:
                    errors.append(f"{manifest}: package.{key} drifted from workspace value")
        return errors

    def metadata_errors(self, version: str) -> list[str]:
        errors: list[str] = []
        ui_metadata = self.load_json(Path("metadata.json"))
        core_metadata = self.load_json(Path("core/metadata.json"))
        if ui_metadata.get("version") != version:
            errors.append("metadata.json version must match Cargo workspace version")
        if core_metadata.get("version") != version:
            errors.append("core/metadata.json version must match Cargo workspace version")

        core_name = str(core_metadata.get("name", ""))
        ui_dependencies = ui_metadata.get("dependencies")
        if not isinstance(ui_dependencies, list) or core_name not in ui_dependencies:
            errors.append("metadata.json dependencies must include core module name")

        external_libraries = core_metadata.get("nix", {}).get("external_libraries", [])
        external_names = [
            item.get("name")
            for item in external_libraries
            if isinstance(item, dict)
        ]
        if "logos_inspector_core" not in external_names:
            errors.append("core/metadata.json must declare logos_inspector_core external library")
        return errors

    def header_errors(self) -> list[str]:
        errors: list[str] = []
        if not self.path(CORE_FFI_HEADER).is_file():
            errors.append("FFI header must live under crates/core-ffi/include")
        if self.path(LEGACY_CORE_HEADER).exists():
            errors.append("FFI header must not live under core/lib")
        return errors

    def load_toml(self, relative: Path) -> dict[str, Any]:
        with self.path(relative).open("rb") as handle:
            return tomllib.load(handle)

    def load_json(self, relative: Path) -> dict[str, Any]:
        with self.path(relative).open("r", encoding="utf-8") as handle:
            return json.load(handle)

    def path(self, relative: Path) -> Path:
        return self.root / relative


def inherited(package: dict[str, Any], key: str) -> bool:
    value = package.get(key)
    return isinstance(value, dict) and value.get("workspace") is True


def direct_or_workspace(package: dict[str, Any], workspace: dict[str, Any], key: str) -> Any:
    if inherited(package, key):
        return workspace.get(key)
    return package.get(key)
