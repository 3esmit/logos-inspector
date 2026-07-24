#!/usr/bin/env python3
"""Assemble and verify Logos Inspector GitHub release artifacts.

The release workflow builds each target independently, then this script makes
the release contract explicit before any GitHub Release is created.  Keeping
the package checks here also makes them runnable locally without a hosted
runner.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import sys
import tarfile
import tempfile
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable


CORE_MODULE_NAME = "logos_inspector"
UI_MODULE_NAME = "logos_inspector_ui"
CORE_RUNTIME_DEPENDENCIES = (
    "blockchain_module",
    "storage_module",
    "delivery_module",
    "lez_core",
)
SUPPORTED_PLATFORMS = ("linux-amd64", "darwin-arm64")
SEMVER = re.compile(
    r"^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z][0-9A-Za-z.-]*)?(?:\+[0-9A-Za-z.-]+)?$"
)
SHA256 = re.compile(r"^[0-9a-f]{64}$")


class ReleaseError(ValueError):
    """A release artifact does not satisfy the published release contract."""


@dataclass(frozen=True)
class ReleaseFiles:
    core: str
    ui: str


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ReleaseError(message)


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise ReleaseError(f"failed to read {path}: {error}") from error
    except json.JSONDecodeError as error:
        raise ReleaseError(f"invalid JSON in {path}: {error}") from error
    require(isinstance(value, dict), f"{path} must contain a JSON object")
    return value


def release_files(version: str, platform: str) -> ReleaseFiles:
    validate_version(version)
    validate_platform(platform)
    suffix = f"{version}-{platform}"
    return ReleaseFiles(
        core=f"logos-inspector-core-{suffix}.lgx",
        ui=f"logos-inspector-ui-{suffix}.lgx",
    )


def validate_version(version: str) -> None:
    require(bool(SEMVER.fullmatch(version)), f"invalid release version `{version}`")


def validate_platform(platform: str) -> None:
    require(
        platform in SUPPORTED_PLATFORMS,
        f"unsupported platform `{platform}`; expected one of {', '.join(SUPPORTED_PLATFORMS)}",
    )


def source_version(root: Path) -> str:
    cargo_path = root / "Cargo.toml"
    try:
        with cargo_path.open("rb") as handle:
            cargo = tomllib.load(handle)
    except OSError as error:
        raise ReleaseError(f"failed to read {cargo_path}: {error}") from error
    workspace = cargo.get("workspace")
    require(isinstance(workspace, dict), "Cargo.toml is missing [workspace]")
    package = workspace.get("package")
    require(isinstance(package, dict), "Cargo.toml is missing [workspace.package]")
    version = package.get("version")
    require(isinstance(version, str), "Cargo.toml is missing [workspace.package].version")
    validate_version(version)
    return version


def validate_source(root: Path) -> str:
    version = source_version(root)
    ui = read_json(root / "metadata.json")
    core = read_json(root / "core" / "metadata.json")

    require(ui.get("name") == UI_MODULE_NAME, "UI metadata has an unexpected module name")
    require(core.get("name") == CORE_MODULE_NAME, "core metadata has an unexpected module name")
    require(ui.get("version") == version, "UI metadata version must match Cargo workspace version")
    require(core.get("version") == version, "core metadata version must match Cargo workspace version")
    require(
        core.get("dependencies") == list(CORE_RUNTIME_DEPENDENCIES),
        "core metadata must declare the required runtime module dependencies",
    )
    dependencies = ui.get("dependencies")
    require(isinstance(dependencies, list), "UI metadata dependencies must be an array")
    require(
        CORE_MODULE_NAME in dependencies,
        "UI metadata must depend on the Inspector core module",
    )
    return version


def find_single_lgx(directory: Path, label: str) -> Path:
    try:
        candidates = sorted(path for path in directory.glob("*.lgx") if path.is_file())
    except OSError as error:
        raise ReleaseError(f"failed to list {label} output {directory}: {error}") from error
    require(len(candidates) == 1, f"{label} output must contain exactly one .lgx file")
    return candidates[0]


def read_lgx(path: Path) -> tuple[dict[str, Any], set[str]]:
    try:
        with tarfile.open(path, mode="r:gz") as archive:
            names = [member.name for member in archive.getmembers()]
            require(names.count("manifest.json") == 1, f"{path.name} must contain one manifest.json")
            member = archive.getmember("manifest.json")
            handle = archive.extractfile(member)
            require(handle is not None, f"{path.name} manifest.json cannot be read")
            try:
                value = json.load(handle)
            except json.JSONDecodeError as error:
                raise ReleaseError(f"{path.name} has invalid manifest JSON: {error}") from error
    except (OSError, tarfile.TarError) as error:
        raise ReleaseError(f"{path.name} is not a readable gzip tar LGX archive: {error}") from error
    require(isinstance(value, dict), f"{path.name} manifest must be a JSON object")
    return value, set(names)


def validate_lgx(
    path: Path,
    *,
    expected_name: str,
    expected_type: str,
    version: str,
    platform: str,
) -> None:
    manifest, entries = read_lgx(path)
    require(manifest.get("name") == expected_name, f"{path.name} has unexpected module name")
    require(manifest.get("type") == expected_type, f"{path.name} has unexpected module type")
    require(manifest.get("version") == version, f"{path.name} version does not match {version}")

    variant_prefix = f"variants/{platform}/"
    require(
        any(entry.startswith(variant_prefix) for entry in entries),
        f"{path.name} does not contain the {platform} variant",
    )

    hashes = manifest.get("hashes")
    require(isinstance(hashes, dict), f"{path.name} must include manifest hashes")
    for key in ("root", "variants", f"variants/{platform}"):
        digest = hashes.get(key)
        require(
            isinstance(digest, str) and bool(SHA256.fullmatch(digest)),
            f"{path.name} has no valid SHA-256 manifest hash for {key}",
        )

    if expected_type == "core":
        main = manifest.get("main")
        require(isinstance(main, dict), f"{path.name} core manifest must declare main binaries")
        binary = main.get(platform)
        require(
            isinstance(binary, str) and binary,
            f"{path.name} core manifest has no main binary for {platform}",
        )
        require(
            f"{variant_prefix}{binary}" in entries,
            f"{path.name} core main binary is missing from its {platform} variant",
        )
        require(
            manifest.get("dependencies") == list(CORE_RUNTIME_DEPENDENCIES),
            f"{path.name} core manifest must declare the required runtime module dependencies",
        )
    elif expected_type == "ui_qml":
        dependencies = manifest.get("dependencies")
        require(isinstance(dependencies, list), f"{path.name} UI manifest dependencies must be an array")
        require(
            CORE_MODULE_NAME in dependencies,
            f"{path.name} UI manifest must depend on {CORE_MODULE_NAME}",
        )


def assemble(
    *,
    ui_dir: Path,
    core_dir: Path,
    output_dir: Path,
    version: str,
    platform: str,
) -> ReleaseFiles:
    files = release_files(version, platform)
    require(not output_dir.exists(), f"release output directory already exists: {output_dir}")
    output_dir.mkdir(parents=True)
    ui_source = find_single_lgx(ui_dir, "UI")
    core_source = find_single_lgx(core_dir, "core")
    ui_target = output_dir / files.ui
    core_target = output_dir / files.core
    shutil.copy2(ui_source, ui_target)
    shutil.copy2(core_source, core_target)
    validate_lgx(
        ui_target,
        expected_name=UI_MODULE_NAME,
        expected_type="ui_qml",
        version=version,
        platform=platform,
    )
    validate_lgx(
        core_target,
        expected_name=CORE_MODULE_NAME,
        expected_type="core",
        version=version,
        platform=platform,
    )
    return files


def expected_files(version: str, platforms: Iterable[str]) -> list[str]:
    names: list[str] = []
    for platform in platforms:
        files = release_files(version, platform)
        names.extend((files.core, files.ui))
    return sorted(names)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_checksums(directory: Path, names: Iterable[str]) -> Path:
    checksum_path = directory / "SHA256SUMS"
    lines = [f"{sha256(directory / name)}  {name}\n" for name in sorted(names)]
    checksum_path.write_text("".join(lines), encoding="utf-8")
    return checksum_path


def validate_checksums(directory: Path, names: Iterable[str]) -> None:
    expected_names = sorted(names)
    checksum_path = directory / "SHA256SUMS"
    require(checksum_path.is_file(), "release is missing SHA256SUMS")
    entries: dict[str, str] = {}
    for line in checksum_path.read_text(encoding="utf-8").splitlines():
        digest, separator, name = line.partition("  ")
        require(separator == "  " and name, "SHA256SUMS has an invalid entry")
        require(bool(SHA256.fullmatch(digest)), "SHA256SUMS has an invalid digest")
        require(name not in entries, f"SHA256SUMS repeats {name}")
        entries[name] = digest
    require(sorted(entries) == expected_names, "SHA256SUMS does not cover exactly the release artifacts")
    for name in expected_names:
        require((directory / name).is_file(), f"release is missing {name}")
        require(entries[name] == sha256(directory / name), f"checksum mismatch for {name}")


def validate_release(
    directory: Path,
    *,
    version: str,
    platforms: tuple[str, ...],
    write_checksum_file: bool,
) -> None:
    validate_version(version)
    require(platforms, "at least one platform is required")
    for platform in platforms:
        validate_platform(platform)
    require(len(set(platforms)) == len(platforms), "release platforms must be unique")
    expected = expected_files(version, platforms)
    require(directory.is_dir(), f"release directory is not a directory: {directory}")
    actual = sorted(path.name for path in directory.iterdir() if path.is_file())
    expected_with_checksums = sorted([*expected, "SHA256SUMS"])
    if write_checksum_file:
        require(actual == expected, "release directory contains missing or unexpected artifacts")
    else:
        require(actual == expected_with_checksums, "release directory contains missing or unexpected artifacts")

    for platform in platforms:
        files = release_files(version, platform)
        validate_lgx(
            directory / files.ui,
            expected_name=UI_MODULE_NAME,
            expected_type="ui_qml",
            version=version,
            platform=platform,
        )
        validate_lgx(
            directory / files.core,
            expected_name=CORE_MODULE_NAME,
            expected_type="core",
            version=version,
            platform=platform,
        )

    if write_checksum_file:
        write_checksums(directory, expected)
    else:
        validate_checksums(directory, expected)


def create_test_lgx(
    path: Path,
    *,
    name: str,
    module_type: str,
    version: str,
    platform: str,
    dependencies: list[str] | None = None,
) -> None:
    resolved_dependencies = dependencies
    if resolved_dependencies is None:
        resolved_dependencies = (
            [CORE_MODULE_NAME]
            if module_type == "ui_qml"
            else list(CORE_RUNTIME_DEPENDENCIES)
        )
    manifest: dict[str, Any] = {
        "name": name,
        "type": module_type,
        "version": version,
        "dependencies": resolved_dependencies,
        "hashes": {
            "root": "0" * 64,
            "variants": "1" * 64,
            f"variants/{platform}": "2" * 64,
        },
    }
    binary = "logos_inspector_plugin.so"
    if module_type == "core":
        manifest["main"] = {platform: binary}
    else:
        manifest["main"] = {}
    with tarfile.open(path, mode="w:gz") as archive:
        payload = json.dumps(manifest).encode("utf-8")
        manifest_info = tarfile.TarInfo("manifest.json")
        manifest_info.size = len(payload)
        archive.addfile(manifest_info, fileobj=BytesReader(payload))
        if module_type == "core":
            binary_payload = b"test binary"
            binary_info = tarfile.TarInfo(f"variants/{platform}/{binary}")
            binary_info.size = len(binary_payload)
            archive.addfile(binary_info, fileobj=BytesReader(binary_payload))
        else:
            qml_payload = b"import QtQuick\n"
            qml_info = tarfile.TarInfo(f"variants/{platform}/qml/Main.qml")
            qml_info.size = len(qml_payload)
            archive.addfile(qml_info, fileobj=BytesReader(qml_payload))


class BytesReader:
    """Small file-like adapter for tarfile without a persistent fixture file."""

    def __init__(self, data: bytes) -> None:
        self.data = data
        self.offset = 0

    def read(self, size: int = -1) -> bytes:
        if size < 0:
            size = len(self.data) - self.offset
        result = self.data[self.offset : self.offset + size]
        self.offset += len(result)
        return result


def self_test() -> None:
    version = "0.2.0-alpha.1"
    platform = "linux-amd64"
    with tempfile.TemporaryDirectory(prefix="logos-inspector-release-test-") as temporary:
        root = Path(temporary)
        ui_dir = root / "ui"
        core_dir = root / "core"
        ui_dir.mkdir()
        core_dir.mkdir()
        create_test_lgx(
            ui_dir / "ui.lgx",
            name=UI_MODULE_NAME,
            module_type="ui_qml",
            version=version,
            platform=platform,
        )
        create_test_lgx(
            core_dir / "core.lgx",
            name=CORE_MODULE_NAME,
            module_type="core",
            version=version,
            platform=platform,
        )
        output = root / "release"
        assemble(
            ui_dir=ui_dir,
            core_dir=core_dir,
            output_dir=output,
            version=version,
            platform=platform,
        )
        validate_release(
            output,
            version=version,
            platforms=(platform,),
            write_checksum_file=True,
        )
        validate_release(
            output,
            version=version,
            platforms=(platform,),
            write_checksum_file=False,
        )
        invalid_core = root / "invalid-core.lgx"
        create_test_lgx(
            invalid_core,
            name=CORE_MODULE_NAME,
            module_type="core",
            version=version,
            platform=platform,
            dependencies=list(CORE_RUNTIME_DEPENDENCIES[:-1]),
        )
        try:
            validate_lgx(
                invalid_core,
                expected_name=CORE_MODULE_NAME,
                expected_type="core",
                version=version,
                platform=platform,
            )
        except ReleaseError:
            pass
        else:
            raise ReleaseError("release artifact fixture accepted a core package without lez_core")
        checksums = output / "SHA256SUMS"
        content = checksums.read_text(encoding="utf-8")
        corrupted_prefix = "0" if content[0] != "0" else "1"
        checksums.write_text(corrupted_prefix + content[1:], encoding="utf-8")
        try:
            validate_release(
                output,
                version=version,
                platforms=(platform,),
                write_checksum_file=False,
            )
        except ReleaseError:
            return
        raise ReleaseError("release artifact fixture accepted a corrupted checksum")


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description="Assemble and verify Inspector release artifacts")
    commands = result.add_subparsers(dest="command", required=True)

    source = commands.add_parser("validate-source", help="validate source version and module identity")
    source.add_argument("--root", type=Path, default=Path("."))

    assemble_parser = commands.add_parser("assemble", help="stage one platform's release artifacts")
    assemble_parser.add_argument("--ui-dir", type=Path, required=True)
    assemble_parser.add_argument("--core-dir", type=Path, required=True)
    assemble_parser.add_argument("--output-dir", type=Path, required=True)
    assemble_parser.add_argument("--version", required=True)
    assemble_parser.add_argument("--platform", required=True, choices=SUPPORTED_PLATFORMS)

    verify = commands.add_parser("verify", help="verify a complete release directory")
    verify.add_argument("--input-dir", type=Path, required=True)
    verify.add_argument("--version", required=True)
    verify.add_argument("--platform", action="append", choices=SUPPORTED_PLATFORMS, required=True)
    verify.add_argument("--write-checksums", action="store_true")

    commands.add_parser("self-test", help="run a dependency-free release artifact fixture test")
    return result


def main() -> int:
    args = parser().parse_args()
    try:
        if args.command == "validate-source":
            print(validate_source(args.root.resolve()))
        elif args.command == "assemble":
            assemble(
                ui_dir=args.ui_dir.resolve(),
                core_dir=args.core_dir.resolve(),
                output_dir=args.output_dir.resolve(),
                version=args.version,
                platform=args.platform,
            )
        elif args.command == "verify":
            validate_release(
                args.input_dir.resolve(),
                version=args.version,
                platforms=tuple(args.platform),
                write_checksum_file=args.write_checksums,
            )
        elif args.command == "self-test":
            self_test()
        else:
            raise ReleaseError(f"unsupported command: {args.command}")
    except ReleaseError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
