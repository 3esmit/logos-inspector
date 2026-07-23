#!/usr/bin/env python3
"""Assemble and verify Logos Inspector GitHub release artifacts.

The release workflow builds each target independently, then this script makes
the release contract explicit before any GitHub Release is created.  Keeping
the package checks here also makes them runnable locally without a hosted
runner.
"""

from __future__ import annotations

import argparse
import gzip
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
STANDALONE_NAME = "logos-inspector-standalone"
STANDALONE_PACKAGE_NAME = "logos-inspector-standalone-gui"
STANDALONE_ENTRYPOINT = "bin/logos-inspector-standalone-gui"
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
    standalone: str


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
        standalone=f"logos-inspector-standalone-{suffix}.tar.gz",
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
    elif expected_type == "ui_qml":
        dependencies = manifest.get("dependencies")
        require(isinstance(dependencies, list), f"{path.name} UI manifest dependencies must be an array")
        require(
            CORE_MODULE_NAME in dependencies,
            f"{path.name} UI manifest must depend on {CORE_MODULE_NAME}",
        )


def normalized_tarinfo(info: tarfile.TarInfo) -> tarfile.TarInfo:
    info.uid = 0
    info.gid = 0
    info.uname = "root"
    info.gname = "root"
    info.mtime = 0
    return info


def create_standalone_archive(
    source: Path,
    destination: Path,
    *,
    version: str,
    platform: str,
) -> None:
    require(source.is_dir(), f"standalone output is not a directory: {source}")
    expected_package_name = f"{STANDALONE_PACKAGE_NAME}-{version}"
    require(
        source.name == expected_package_name,
        "standalone package version does not match the release version",
    )
    with tempfile.TemporaryDirectory(prefix="logos-inspector-release-") as temporary:
        staging = Path(temporary) / destination.name.removesuffix(".tar.gz")
        shutil.copytree(source, staging, symlinks=True)
        entrypoint = staging / STANDALONE_ENTRYPOINT
        require(entrypoint.is_file(), "standalone package is missing its GUI entrypoint")
        qml_entry = staging / "share" / "logos-inspector" / "qml" / "StandaloneMain.qml"
        require(qml_entry.is_file(), "standalone package is missing its QML entrypoint")
        release_manifest = {
            "format": 1,
            "name": STANDALONE_NAME,
            "version": version,
            "platform": platform,
            "entrypoint": STANDALONE_ENTRYPOINT,
            "source": "flake package standalone",
        }
        (staging / "release-manifest.json").write_text(
            json.dumps(release_manifest, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        with destination.open("wb") as raw:
            with gzip.GzipFile(fileobj=raw, mode="wb", mtime=0) as compressed:
                with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as archive:
                    archive.add(staging, arcname=staging.name, filter=normalized_tarinfo)


def validate_standalone_archive(path: Path, *, version: str, platform: str) -> None:
    root = path.name.removesuffix(".tar.gz")
    manifest_name = f"{root}/release-manifest.json"
    entrypoint_name = f"{root}/{STANDALONE_ENTRYPOINT}"
    qml_entry_name = f"{root}/share/logos-inspector/qml/StandaloneMain.qml"
    try:
        with tarfile.open(path, mode="r:gz") as archive:
            names = {member.name for member in archive.getmembers()}
            require(manifest_name in names, f"{path.name} is missing release-manifest.json")
            require(entrypoint_name in names, f"{path.name} is missing its GUI entrypoint")
            require(qml_entry_name in names, f"{path.name} is missing its QML entrypoint")
            handle = archive.extractfile(manifest_name)
            require(handle is not None, f"{path.name} release manifest cannot be read")
            try:
                manifest = json.load(handle)
            except json.JSONDecodeError as error:
                raise ReleaseError(f"{path.name} has invalid release manifest JSON: {error}") from error
    except (OSError, tarfile.TarError) as error:
        raise ReleaseError(f"{path.name} is not a readable gzip tar archive: {error}") from error
    require(isinstance(manifest, dict), f"{path.name} release manifest must be a JSON object")
    require(manifest.get("format") == 1, f"{path.name} has an unsupported release manifest format")
    require(manifest.get("name") == STANDALONE_NAME, f"{path.name} has an unexpected standalone name")
    require(manifest.get("version") == version, f"{path.name} version does not match {version}")
    require(manifest.get("platform") == platform, f"{path.name} platform does not match {platform}")
    require(manifest.get("entrypoint") == STANDALONE_ENTRYPOINT, f"{path.name} has an unexpected entrypoint")


def assemble(
    *,
    ui_dir: Path,
    core_dir: Path,
    standalone_dir: Path,
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
    create_standalone_archive(
        standalone_dir,
        output_dir / files.standalone,
        version=version,
        platform=platform,
    )
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
    validate_standalone_archive(output_dir / files.standalone, version=version, platform=platform)
    return files


def expected_files(version: str, platforms: Iterable[str]) -> list[str]:
    names: list[str] = []
    for platform in platforms:
        files = release_files(version, platform)
        names.extend((files.core, files.ui, files.standalone))
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
        validate_standalone_archive(directory / files.standalone, version=version, platform=platform)

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
) -> None:
    manifest: dict[str, Any] = {
        "name": name,
        "type": module_type,
        "version": version,
        "dependencies": [CORE_MODULE_NAME] if module_type == "ui_qml" else [],
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
        standalone_dir = root / f"{STANDALONE_PACKAGE_NAME}-{version}"
        ui_dir.mkdir()
        core_dir.mkdir()
        (standalone_dir / "bin").mkdir(parents=True)
        (standalone_dir / "share" / "logos-inspector" / "qml").mkdir(parents=True)
        (standalone_dir / STANDALONE_ENTRYPOINT).write_text("#!/bin/sh\n", encoding="utf-8")
        (standalone_dir / "share" / "logos-inspector" / "qml" / "StandaloneMain.qml").write_text(
            "import QtQuick\n", encoding="utf-8"
        )
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
            standalone_dir=standalone_dir,
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
        checksums = output / "SHA256SUMS"
        content = checksums.read_text(encoding="utf-8")
        checksums.write_text("f" + content[1:], encoding="utf-8")
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
    assemble_parser.add_argument("--standalone-dir", type=Path, required=True)
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
                standalone_dir=args.standalone_dir.resolve(),
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
