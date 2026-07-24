#!/usr/bin/env python3
"""Validate Logos Inspector standalone source and release artifacts."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import posixpath
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
from pathlib import Path, PurePosixPath
from typing import Any


CORE_MODULE_NAME = "logos_inspector"
UI_MODULE_NAME = "logos_inspector_ui"
DISPLAY_NAME = "Logos Inspector"
CORE_RUNTIME_DEPENDENCIES = (
    "blockchain_module",
    "storage_module",
    "delivery_module",
    "lez_core",
)
SEMVER = re.compile(
    r"^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z][0-9A-Za-z.-]*)?(?:\+[0-9A-Za-z.-]+)?$"
)
SHA256 = re.compile(r"^[0-9a-f]{64}$")
PRINTABLE_STRING = re.compile(rb"[\x20-\x7e]{4,}")
APP_NAME = "logos-inspector-standalone"
MAIN_BINARY = "logos-inspector-standalone-gui"
HELPER_BINARY = "logos-inspector-testnet-v02-helper"
QML_ENTRY = "share/logos-inspector/qml/StandaloneMain.qml"
EXECUTABLE_MAGICS = (
    b"\x7fELF",
    b"\xca\xfe\xba\xbe",
    b"\xca\xfe\xba\xbf",
    b"\xce\xfa\xed\xfe",
    b"\xcf\xfa\xed\xfe",
    b"\xfe\xed\xfa\xce",
    b"\xfe\xed\xfa\xcf",
)


class ReleaseError(ValueError):
    """Source or artifact violates the standalone release contract."""


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


def validate_version(version: str) -> None:
    require(bool(SEMVER.fullmatch(version)), f"invalid release version `{version}`")


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
    require(
        isinstance(version, str),
        "Cargo.toml is missing [workspace.package].version",
    )
    validate_version(version)
    return version


def validate_source(root: Path) -> str:
    version = source_version(root)
    ui = read_json(root / "metadata.json")
    core = read_json(root / "core" / "metadata.json")
    require(ui.get("name") == UI_MODULE_NAME, "UI metadata has unexpected name")
    require(core.get("name") == CORE_MODULE_NAME, "core metadata has unexpected name")
    require(ui.get("display_name") == DISPLAY_NAME, "UI display name is not human-facing")
    require(core.get("display_name") == DISPLAY_NAME, "core display name drifted")
    require(ui.get("version") == version, "UI metadata version differs from Cargo")
    require(core.get("version") == version, "core metadata version differs from Cargo")
    require(
        ui.get("dependencies") == [CORE_MODULE_NAME],
        "UI metadata must depend only on Inspector Core",
    )
    require(
        core.get("dependencies") == list(CORE_RUNTIME_DEPENDENCIES),
        "core metadata runtime dependency names drifted",
    )
    return version


def asset_names(version: str) -> tuple[str, str]:
    validate_version(version)
    return (
        f"{APP_NAME}-{version}-linux-amd64.AppImage",
        f"{APP_NAME}-{version}-darwin-arm64.app.tar.gz",
    )


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_checksums(directory: Path, names: tuple[str, ...]) -> None:
    lines = [f"{sha256(directory / name)}  {name}\n" for name in sorted(names)]
    (directory / "SHA256SUMS").write_text("".join(lines), encoding="utf-8")


def verify_checksums(directory: Path, names: tuple[str, ...]) -> None:
    checksum_path = directory / "SHA256SUMS"
    require(checksum_path.is_file(), "release is missing SHA256SUMS")
    entries: dict[str, str] = {}
    for line in checksum_path.read_text(encoding="utf-8").splitlines():
        digest, separator, name = line.partition("  ")
        require(separator == "  " and name, "SHA256SUMS has invalid syntax")
        require(bool(SHA256.fullmatch(digest)), "SHA256SUMS has invalid digest")
        require(name not in entries, f"SHA256SUMS repeats {name}")
        entries[name] = digest
    require(sorted(entries) == sorted(names), "SHA256SUMS does not cover exact assets")
    for name in names:
        require(entries[name] == sha256(directory / name), f"checksum mismatch for {name}")


def safe_archive_path(name: str) -> PurePosixPath:
    path = PurePosixPath(name)
    require(not path.is_absolute(), f"archive member is absolute: {name}")
    require(".." not in path.parts, f"archive member escapes root: {name}")
    return path


def safe_archive_link(
    *,
    member: tarfile.TarInfo,
    app_root: str,
) -> PurePosixPath:
    target = PurePosixPath(member.linkname)
    require(bool(member.linkname), f"archive link has empty target: {member.name}")
    require(
        not target.is_absolute(),
        f"archive link has absolute target: {member.name}",
    )
    if member.issym():
        member_path = safe_archive_path(member.name)
        combined = member_path.parent / target
    else:
        combined = target
    normalized = PurePosixPath(posixpath.normpath(combined.as_posix()))
    require(
        bool(normalized.parts) and normalized.parts[0] == app_root,
        f"archive link escapes app root: {member.name}",
    )
    return normalized


def verify_macos_archive(path: Path) -> None:
    try:
        with tarfile.open(path, "r:gz") as archive:
            members = archive.getmembers()
            require(bool(members), f"{path.name} is empty")
            names = {safe_archive_path(member.name).as_posix() for member in members}
            roots = {
                PurePosixPath(name).parts[0]
                for name in names
                if PurePosixPath(name).parts
            }
            require(len(roots) == 1, f"{path.name} must contain one app root")
            root = next(iter(roots))
            require(root.endswith(".app"), f"{path.name} does not contain an app bundle")
            required = (
                f"{root}/Contents/MacOS/{MAIN_BINARY}",
                f"{root}/Contents/libexec/{HELPER_BINARY}",
                f"{root}/Contents/{QML_ENTRY}",
            )
            for required_name in required:
                require(required_name in names, f"{path.name} is missing {required_name}")
            for member in members:
                if member.issym() or member.islnk():
                    safe_archive_link(member=member, app_root=root)
            launcher = archive.extractfile(f"{root}/Contents/MacOS/{MAIN_BINARY}")
            require(launcher is not None, f"{path.name} launcher is unreadable")
            launcher_bytes = launcher.read()
            require(
                b"/nix/store/" not in launcher_bytes,
                f"{path.name} launcher contains a build-host Nix store path",
            )
    except (OSError, tarfile.TarError) as error:
        raise ReleaseError(f"{path.name} is not a readable gzip tar archive: {error}") from error


def verify_release(
    directory: Path,
    *,
    version: str,
    write_checksum_file: bool,
) -> None:
    names = asset_names(version)
    require(directory.is_dir(), f"release input is not a directory: {directory}")
    actual = sorted(path.name for path in directory.iterdir() if path.is_file())
    expected = sorted(names if write_checksum_file else (*names, "SHA256SUMS"))
    require(actual == expected, "release contains missing or unexpected assets")
    appimage = directory / names[0]
    require(appimage.read_bytes()[:4] == b"\x7fELF", "AppImage lacks ELF header")
    verify_macos_archive(directory / names[1])
    if write_checksum_file:
        write_checksums(directory, names)
    else:
        verify_checksums(directory, names)


def within(root: Path, path: Path) -> bool:
    try:
        path.relative_to(root)
    except ValueError:
        return False
    return True


def inspect_runtime_metadata(path: Path, payload: bytes) -> None:
    if payload.startswith(b"\x7fELF"):
        tool = shutil.which("readelf")
        require(tool is not None, "readelf is required to verify an ELF bundle")
        command = [tool, "--wide", "--program-headers", "--dynamic", str(path)]
    else:
        tool = shutil.which("otool")
        require(tool is not None, "otool is required to verify a Mach-O bundle")
        command = [tool, "-L", "-l", str(path)]
    result = subprocess.run(command, check=False, capture_output=True)
    require(
        result.returncode == 0,
        f"failed to inspect runtime metadata for {path}: "
        + result.stderr.decode("utf-8", errors="replace").strip(),
    )
    require(
        b"/nix/store/" not in result.stdout,
        f"runtime metadata contains a build-host path: {path}",
    )


def verify_tree(root: Path, launcher: Path) -> None:
    root = root.resolve()
    launcher = launcher.resolve()
    require(root.is_dir(), f"bundle root does not exist: {root}")
    require(launcher.is_file(), f"bundle launcher does not exist: {launcher}")
    require(os.access(launcher, os.X_OK), f"bundle launcher is not executable: {launcher}")
    required = (
        root / QML_ENTRY,
        root / "libexec" / HELPER_BINARY,
    )
    for path in required:
        require(path.is_file(), f"bundle is missing {path.relative_to(root)}")
    require(
        os.access(required[1], os.X_OK),
        f"bundle helper is not executable: {required[1].relative_to(root)}",
    )
    for path in root.rglob("*"):
        if path.is_symlink():
            target = path.resolve(strict=False)
            require(
                within(root, target),
                f"bundle symlink escapes root: {path.relative_to(root)} -> {target}",
            )
            require(target.exists(), f"bundle has broken symlink: {path.relative_to(root)}")
        elif path.is_file():
            try:
                with path.open("rb") as handle:
                    first = handle.readline(4096)
                    remainder = (
                        handle.read()
                        if first.startswith(b"#!") and path.stat().st_size < 1024 * 1024
                        else b""
                    )
            except OSError as error:
                raise ReleaseError(f"failed to inspect {path}: {error}") from error
            if executable_format(first):
                inspect_runtime_metadata(path, first)
            if first.startswith(b"#!"):
                require(
                    b"/nix/store/" not in first + remainder,
                    f"bundle script contains a build-host path: {path.relative_to(root)}",
                )
    if launcher.stat().st_size < 1024 * 1024:
        require(
            b"/nix/store/" not in launcher.read_bytes(),
            "bundle launcher contains a build-host Nix store path",
        )


def executable_format(payload: bytes) -> bool:
    return any(payload.startswith(magic) for magic in EXECUTABLE_MAGICS)


def classify_embedded_reference(value: str) -> str:
    lowered = value.lower()
    if "/include/" in lowered or any(
        suffix in lowered
        for suffix in (".h", ".hpp", ".hxx", ".c", ".cc", ".cpp", ".rs")
    ):
        return "source-or-assertion"
    if any(
        marker in lowered
        for marker in (
            "/share/locale",
            "/etc/",
            "gsettings",
            "schema",
            "hwdb",
            "preset",
        )
    ):
        return "vendor-default-data"
    if any(marker in lowered for marker in ("/lib/", "/lib:", ".so", ".dylib")):
        return "linker-or-library-literal"
    return "other-vendor-literal"


def audit_binary_references(root: Path) -> None:
    root = root.resolve()
    require(root.is_dir(), f"bundle root does not exist: {root}")
    categories: dict[str, int] = {}
    affected_files = 0
    references = 0
    forbidden_text_paths: list[Path] = []

    for path in root.rglob("*"):
        if not path.is_file() or path.is_symlink():
            continue
        try:
            payload = path.read_bytes()
        except OSError as error:
            raise ReleaseError(f"failed to inspect {path}: {error}") from error
        if b"/nix/store/" not in payload:
            continue
        if not executable_format(payload):
            forbidden_text_paths.append(path.relative_to(root))
            continue

        affected_files += 1
        for match in PRINTABLE_STRING.finditer(payload):
            value = match.group().decode("ascii")
            occurrences = value.count("/nix/store/")
            if occurrences == 0:
                continue
            category = classify_embedded_reference(value)
            categories[category] = categories.get(category, 0) + occurrences
            references += occurrences

    require(
        not forbidden_text_paths,
        "bundle contains build-host paths outside compiled vendor binaries: "
        + ", ".join(str(path) for path in forbidden_text_paths),
    )
    if references == 0:
        print("No embedded vendor Nix strings found.")
        return

    print(
        "::warning title=Embedded vendor Nix strings::"
        f"{references} inert string(s) remain in {affected_files} compiled file(s); "
        "runtime paths are verified separately and the extracted GUI smoke runs "
        "without /nix/store."
    )
    print("### Embedded vendor Nix-string audit")
    print()
    print(f"- Compiled files containing strings: {affected_files}")
    print(f"- String occurrences: {references}")
    for category, count in sorted(categories.items()):
        print(f"- `{category}`: {count}")


def add_tar_file(
    archive: tarfile.TarFile,
    name: str,
    payload: bytes,
    mode: int = 0o644,
) -> None:
    info = tarfile.TarInfo(name)
    info.size = len(payload)
    info.mode = mode
    with tempfile.SpooledTemporaryFile() as handle:
        handle.write(payload)
        handle.seek(0)
        archive.addfile(info, handle)


def self_test() -> None:
    version = "0.2.0-alpha.1"
    with tempfile.TemporaryDirectory(prefix="inspector-release-test-") as temporary:
        release = Path(temporary)
        linux_name, macos_name = asset_names(version)
        (release / linux_name).write_bytes(b"\x7fELFtest")
        root = "LogosInspector.app/Contents"
        with tarfile.open(release / macos_name, "w:gz") as archive:
            add_tar_file(
                archive,
                f"{root}/MacOS/{MAIN_BINARY}",
                b"#!/bin/sh\nexit 0\n",
                0o755,
            )
            add_tar_file(archive, f"{root}/libexec/{HELPER_BINARY}", b"\x7fELF", 0o755)
            add_tar_file(archive, f"{root}/{QML_ENTRY}", b"import QtQuick\n")
        verify_release(release, version=version, write_checksum_file=True)
        verify_release(release, version=version, write_checksum_file=False)

        valid_link = tarfile.TarInfo(
            "LogosInspector.app/Contents/Frameworks/Qt.framework/Qt"
        )
        valid_link.type = tarfile.SYMTYPE
        valid_link.linkname = "Versions/Current/Qt"
        safe_archive_link(member=valid_link, app_root="LogosInspector.app")

        escaping_link = tarfile.TarInfo(
            "LogosInspector.app/Contents/Frameworks/escape"
        )
        escaping_link.type = tarfile.SYMTYPE
        escaping_link.linkname = "../../../outside"
        try:
            safe_archive_link(member=escaping_link, app_root="LogosInspector.app")
        except ReleaseError:
            pass
        else:
            raise ReleaseError("archive link traversal fixture was accepted")


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    commands = root.add_subparsers(dest="command", required=True)
    source = commands.add_parser("validate-source")
    source.add_argument("--root", type=Path, required=True)
    verify = commands.add_parser("verify")
    verify.add_argument("--input-dir", type=Path, required=True)
    verify.add_argument("--version", required=True)
    verify.add_argument("--write-checksums", action="store_true")
    tree = commands.add_parser("verify-tree")
    tree.add_argument("--root", type=Path, required=True)
    tree.add_argument("--launcher", type=Path, required=True)
    audit = commands.add_parser("audit-binary-refs")
    audit.add_argument("--root", type=Path, required=True)
    commands.add_parser("self-test")
    return root


def main() -> int:
    args = parser().parse_args()
    try:
        if args.command == "validate-source":
            print(validate_source(args.root))
        elif args.command == "verify":
            verify_release(
                args.input_dir,
                version=args.version,
                write_checksum_file=args.write_checksums,
            )
        elif args.command == "verify-tree":
            verify_tree(args.root, args.launcher)
        elif args.command == "audit-binary-refs":
            audit_binary_references(args.root)
        elif args.command == "self-test":
            self_test()
    except (OSError, ReleaseError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
