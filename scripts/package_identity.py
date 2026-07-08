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
PACKAGE_NAMES = {
    Path("Cargo.toml"): "logos-inspector",
    Path("crates/core-ffi/Cargo.toml"): "logos-inspector-core-ffi",
    Path("crates/standalone-gui/Cargo.toml"): "logos-inspector-standalone-gui",
}
CORE_MODULE_NAME = "logos_inspector"
UI_MODULE_NAME = "logos_inspector_ui"
CORE_FFI_LIB_NAME = "logos_inspector_core"
STANDALONE_PACKAGE_NAME = "logos-inspector-standalone-gui"
CORE_FFI_HEADER = Path("crates/core-ffi/include/logos_inspector_core.h")
LEGACY_CORE_HEADER = Path("core/lib/logos_inspector_core.h")
STANDALONE_QML_ENTRY = Path("qml/StandaloneMain.qml")


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
        errors.extend(self.package_identity_errors())
        errors.extend(self.metadata_errors(version))
        errors.extend(self.header_errors())
        errors.extend(self.launch_errors())
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

    def package_identity_errors(self) -> list[str]:
        errors: list[str] = []
        for manifest, expected in PACKAGE_NAMES.items():
            package = self.load_toml(manifest).get("package", {})
            if package.get("name") != expected:
                errors.append(f"{manifest}: package.name must be {expected}")

        core_ffi = self.load_toml(Path("crates/core-ffi/Cargo.toml"))
        if core_ffi.get("lib", {}).get("name") != CORE_FFI_LIB_NAME:
            errors.append(
                "crates/core-ffi/Cargo.toml: lib.name must be logos_inspector_core"
            )
        return errors

    def metadata_errors(self, version: str) -> list[str]:
        errors: list[str] = []
        ui_metadata = self.load_json(Path("metadata.json"))
        core_metadata = self.load_json(Path("core/metadata.json"))
        if ui_metadata.get("name") != UI_MODULE_NAME:
            errors.append("metadata.json name must be logos_inspector_ui")
        if core_metadata.get("name") != CORE_MODULE_NAME:
            errors.append("core/metadata.json name must be logos_inspector")
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
        if CORE_FFI_LIB_NAME not in external_names:
            errors.append("core/metadata.json must declare logos_inspector_core external library")
        errors.extend(self.ui_resource_errors(ui_metadata))
        return errors

    def ui_resource_errors(self, ui_metadata: dict[str, Any]) -> list[str]:
        errors: list[str] = []
        view = metadata_relative_path(ui_metadata, "view")
        icon = metadata_relative_path(ui_metadata, "icon")
        if view is None:
            errors.append("metadata.json view must be a repository-relative QML file")
        elif not self.path(view).is_file():
            errors.append(f"metadata.json view does not exist: {view}")
        if icon is None:
            errors.append("metadata.json icon must be a repository-relative asset")
        elif not self.path(icon).is_file():
            errors.append(f"metadata.json icon does not exist: {icon}")
        if not self.path(STANDALONE_QML_ENTRY).is_file():
            errors.append(f"standalone QML entry does not exist: {STANDALONE_QML_ENTRY}")
        return errors

    def header_errors(self) -> list[str]:
        errors: list[str] = []
        if not self.path(CORE_FFI_HEADER).is_file():
            errors.append("FFI header must live under crates/core-ffi/include")
        if self.path(LEGACY_CORE_HEADER).exists():
            errors.append("FFI header must not live under core/lib")
        return errors

    def launch_errors(self) -> list[str]:
        checks = (
            (
                Path("src/gui/planner.rs"),
                STANDALONE_PACKAGE_NAME,
                "src/gui/planner.rs must launch the standalone GUI binary",
            ),
            (
                Path("src/gui/planner.rs"),
                "#standalone",
                "src/gui/planner.rs must fall back to the standalone flake app",
            ),
            (
                Path("crates/standalone-gui/build.rs"),
                f"cargo:rustc-link-arg-bin={STANDALONE_PACKAGE_NAME}",
                "standalone build script must target the standalone binary",
            ),
            (
                Path("scripts/gui-visual-action-smoke.sh"),
                f"cargo run -p {STANDALONE_PACKAGE_NAME}",
                "GUI smoke script must run the standalone GUI package",
            ),
            (
                Path("flake.nix"),
                f'meta.mainProgram = "{STANDALONE_PACKAGE_NAME}"',
                "flake.nix must expose the standalone GUI main program",
            ),
            (
                Path("flake.nix"),
                f"/bin/{STANDALONE_PACKAGE_NAME}",
                "flake.nix apps must launch the standalone GUI binary",
            ),
        )
        errors: list[str] = []
        for relative, needle, message in checks:
            if needle not in self.read_text(relative):
                errors.append(message)
        return errors

    def load_toml(self, relative: Path) -> dict[str, Any]:
        with self.path(relative).open("rb") as handle:
            return tomllib.load(handle)

    def load_json(self, relative: Path) -> dict[str, Any]:
        with self.path(relative).open("r", encoding="utf-8") as handle:
            return json.load(handle)

    def read_text(self, relative: Path) -> str:
        return self.path(relative).read_text(encoding="utf-8")

    def path(self, relative: Path) -> Path:
        return self.root / relative


def inherited(package: dict[str, Any], key: str) -> bool:
    value = package.get(key)
    return isinstance(value, dict) and value.get("workspace") is True


def direct_or_workspace(package: dict[str, Any], workspace: dict[str, Any], key: str) -> Any:
    if inherited(package, key):
        return workspace.get(key)
    return package.get(key)


def metadata_relative_path(metadata: dict[str, Any], key: str) -> Path | None:
    value = metadata.get(key)
    if not isinstance(value, str):
        return None
    path = Path(value)
    if path.is_absolute() or ".." in path.parts:
        return None
    return path
