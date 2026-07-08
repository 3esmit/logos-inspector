from __future__ import annotations

import json
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
CATALOG_PATH = Path("build-artifacts.json")
LEZ_DEPENDENCIES = (
    "common",
    "lee",
    "lee_core",
    "sequencer_service_rpc",
    "wallet",
)


def load_catalog(root: Path = ROOT) -> dict[str, Any]:
    with (root / CATALOG_PATH).open("r", encoding="utf-8") as handle:
        return json.load(handle)


def circuits_repo(catalog: dict[str, Any] | None = None) -> str:
    return str((catalog or load_catalog())["circuits"]["repo"])


def circuits_release(catalog: dict[str, Any] | None = None) -> str:
    return str((catalog or load_catalog())["circuits"]["release"])


def circuit_target_by_platform(catalog: dict[str, Any], os_name: str, arch: str) -> dict[str, str]:
    for target in catalog["circuits"]["targets"].values():
        if target.get("os") == os_name and target.get("arch") == arch:
            return {
                "os": str(target["os"]),
                "arch": str(target["arch"]),
                "hash": str(target["hash"]),
            }
    raise RuntimeError(f"no circuits artifact for {os_name}-{arch}")


def circuit_artifact_name(release: str, target: dict[str, str]) -> str:
    return f"logos-blockchain-circuits-{release}-{target['os']}-{target['arch']}.tar.gz"


def circuit_artifact_url(catalog: dict[str, Any], release: str, target: dict[str, str]) -> str:
    artifact = circuit_artifact_name(release, target)
    return f"https://github.com/{circuits_repo(catalog)}/releases/download/{release}/{artifact}"


@dataclass(frozen=True)
class BuildArtifacts:
    root: Path

    def validate(self) -> list[str]:
        errors: list[str] = []
        try:
            catalog = load_catalog(self.root)
        except OSError as err:
            return [f"{CATALOG_PATH}: failed to read catalog: {err}"]
        except json.JSONDecodeError as err:
            return [f"{CATALOG_PATH}: invalid JSON: {err}"]

        errors.extend(catalog_shape_errors(catalog))
        errors.extend(self.cargo_manifest_errors(catalog))
        errors.extend(self.cargo_lock_errors(catalog))
        errors.extend(self.ci_errors(catalog))
        return errors

    def cargo_manifest_errors(self, catalog: dict[str, Any]) -> list[str]:
        errors: list[str] = []
        cargo = self.load_toml(Path("Cargo.toml"))
        deps = cargo.get("dependencies", {})
        lez = catalog.get("lez", {})
        tag = lez.get("cargoTag")
        repo = f"https://github.com/{lez.get('repo')}.git"
        for dep_name in LEZ_DEPENDENCIES:
            dep = deps.get(dep_name)
            if not isinstance(dep, dict):
                errors.append(f"Cargo.toml dependency `{dep_name}` must be a table")
                continue
            if dep.get("git") != repo:
                errors.append(f"Cargo.toml dependency `{dep_name}` git URL drifted from catalog")
            if dep.get("tag") != tag:
                errors.append(f"Cargo.toml dependency `{dep_name}` tag drifted from catalog")

        standalone = self.load_toml(Path("crates/standalone-gui/Cargo.toml"))
        rapidsnark = standalone.get("dependencies", {}).get("rust-rapidsnark")
        if not isinstance(rapidsnark, dict):
            errors.append("crates/standalone-gui/Cargo.toml rust-rapidsnark dependency must be a table")
        elif rapidsnark.get("rev") != catalog.get("rapidsnark", {}).get("cargoRev"):
            errors.append("crates/standalone-gui/Cargo.toml rust-rapidsnark rev drifted from catalog")
        return errors

    def cargo_lock_errors(self, catalog: dict[str, Any]) -> list[str]:
        lockfile = self.path(Path("Cargo.lock")).read_text(encoding="utf-8")
        errors: list[str] = []
        lez = catalog.get("lez", {})
        lez_source = (
            f"https://github.com/{lez.get('repo')}.git?"
            f"tag={lez.get('cargoTag')}#{lez.get('revision')}"
        )
        if lez_source not in lockfile:
            errors.append("Cargo.lock LEZ source drifted from build artifact catalog")

        rapidsnark_rev = str(catalog.get("rapidsnark", {}).get("cargoRev", ""))
        rapidsnark_source = (
            "https://github.com/logos-blockchain/logos-blockchain-rust-rapidsnark.git"
            f"?rev={rapidsnark_rev}#{rapidsnark_rev}"
        )
        if rapidsnark_source not in lockfile:
            errors.append("Cargo.lock rust-rapidsnark source drifted from build artifact catalog")

        circuits = catalog.get("circuits", {})
        circuits_source = (
            f"https://github.com/{circuits.get('repo')}.git?"
            f"tag={circuits.get('release')}"
        )
        if circuits_source not in lockfile:
            errors.append("Cargo.lock circuits source drifted from build artifact catalog")
        return errors

    def ci_errors(self, catalog: dict[str, Any]) -> list[str]:
        ci_text = self.path(Path(".github/workflows/ci.yml")).read_text(encoding="utf-8")
        release = str(catalog.get("circuits", {}).get("release", ""))
        if f"setup-circuits.py {release}" in ci_text:
            return ["CI must use the build pipeline script instead of hardcoded circuits release"]
        return []

    def load_toml(self, relative: Path) -> dict[str, Any]:
        with self.path(relative).open("rb") as handle:
            return tomllib.load(handle)

    def path(self, relative: Path) -> Path:
        return self.root / relative


def catalog_shape_errors(catalog: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    circuits = catalog.get("circuits")
    if not isinstance(circuits, dict):
        return ["build-artifacts.json must contain a circuits object"]
    if not str(circuits.get("release", "")).startswith("v"):
        errors.append("circuits.release must include the leading v")
    if not circuits.get("repo"):
        errors.append("circuits.repo is required")
    errors.extend(target_errors("circuits.targets", circuits.get("targets"), ("os", "arch", "hash")))

    rapidsnark = catalog.get("rapidsnark")
    if not isinstance(rapidsnark, dict):
        errors.append("build-artifacts.json must contain a rapidsnark object")
    else:
        if not rapidsnark.get("version"):
            errors.append("rapidsnark.version is required")
        if not rapidsnark.get("cargoRev"):
            errors.append("rapidsnark.cargoRev is required")
        errors.extend(target_errors("rapidsnark.targets", rapidsnark.get("targets"), ("url", "hash")))

    lez = catalog.get("lez")
    if not isinstance(lez, dict):
        errors.append("build-artifacts.json must contain a lez object")
    else:
        for key in ("repo", "cargoTag", "revision", "sourceHash"):
            if not lez.get(key):
                errors.append(f"lez.{key} is required")
    return errors


def target_errors(label: str, targets: object, required_keys: tuple[str, ...]) -> list[str]:
    if not isinstance(targets, dict) or not targets:
        return [f"{label} must be a non-empty object"]

    errors: list[str] = []
    for system, target in targets.items():
        if not isinstance(target, dict):
            errors.append(f"{label}.{system} must be an object")
            continue
        for key in required_keys:
            if not target.get(key):
                errors.append(f"{label}.{system}.{key} is required")
    return errors
