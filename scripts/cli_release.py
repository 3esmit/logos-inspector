#!/usr/bin/env python3
"""Build, validate, and smoke-test portable Logos Inspector CLI releases."""

from __future__ import annotations

import argparse
import hashlib
import io
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
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
PRODUCT = "logos-inspector-cli"
BINARY_NAME = "logos-inspector"
BUILD_INFO_NAME = "BUILD-INFO.json"
BUILD_INFO_SCHEMA_VERSION = 1
BUNDLE_FORMAT = "portable-directory-v1"
SEMVER = re.compile(
    r"^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z][0-9A-Za-z.-]*)?(?:\+[0-9A-Za-z.-]+)?$"
)
COMMIT = re.compile(r"^[0-9a-f]{40}$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")


class CliReleaseError(ValueError):
    """The source, archive, or executable violates the CLI release contract."""


@dataclass(frozen=True)
class Platform:
    name: str
    target: str


PLATFORMS = {
    "linux-amd64": Platform(
        name="linux-amd64",
        target="x86_64-unknown-linux-gnu",
    ),
    "darwin-arm64": Platform(
        name="darwin-arm64",
        target="aarch64-apple-darwin",
    ),
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise CliReleaseError(message)


def platform(name: str) -> Platform:
    try:
        return PLATFORMS[name]
    except KeyError as error:
        choices = ", ".join(sorted(PLATFORMS))
        raise CliReleaseError(f"unsupported platform `{name}`; expected one of: {choices}") from error


def validate_version(version: str) -> None:
    require(bool(SEMVER.fullmatch(version)), f"invalid release version `{version}`")


def validate_commit(commit: str) -> None:
    require(bool(COMMIT.fullmatch(commit)), f"invalid source commit `{commit}`")


def source_version(root: Path) -> str:
    cargo_path = root / "Cargo.toml"
    try:
        with cargo_path.open("rb") as handle:
            cargo = tomllib.load(handle)
    except OSError as error:
        raise CliReleaseError(f"failed to read {cargo_path}: {error}") from error
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


def asset_name(version: str, item: Platform) -> str:
    validate_version(version)
    return f"{PRODUCT}-{version}-{item.name}.tar.gz"


def archive_root(version: str, item: Platform) -> str:
    validate_version(version)
    return f"{PRODUCT}-{version}-{item.name}"


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
    require(bool(path.parts), f"archive member has an empty path: {name!r}")
    require(not path.is_absolute(), f"archive member is absolute: {name}")
    require(".." not in path.parts, f"archive member escapes root: {name}")
    return path


def safe_archive_link(member: tarfile.TarInfo, root: str) -> PurePosixPath:
    target = PurePosixPath(member.linkname)
    require(bool(member.linkname), f"archive link has an empty target: {member.name}")
    require(not target.is_absolute(), f"archive link is absolute: {member.name}")
    if member.issym():
        combined = safe_archive_path(member.name).parent / target
    else:
        combined = target
    normalized = PurePosixPath(posixpath.normpath(combined.as_posix()))
    require(
        bool(normalized.parts) and normalized.parts[0] == root,
        f"archive link escapes bundle root: {member.name}",
    )
    return normalized


def build_info(
    *,
    version: str,
    item: Platform,
    commit: str,
) -> dict[str, Any]:
    validate_version(version)
    validate_commit(commit)
    return {
        "binary": BINARY_NAME,
        "bundle_format": BUNDLE_FORMAT,
        "commit": commit,
        "platform": item.name,
        "product": PRODUCT,
        "schema_version": BUILD_INFO_SCHEMA_VERSION,
        "target": item.target,
        "version": version,
    }


def validate_build_info(
    payload: bytes,
    *,
    version: str,
    item: Platform,
    expected_commit: str | None,
) -> dict[str, Any]:
    try:
        value = json.loads(payload.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CliReleaseError(f"{BUILD_INFO_NAME} is not valid UTF-8 JSON: {error}") from error
    require(isinstance(value, dict), f"{BUILD_INFO_NAME} must contain a JSON object")
    expected = build_info(
        version=version,
        item=item,
        commit=value.get("commit") if isinstance(value.get("commit"), str) else "",
    )
    require(value == expected, f"{BUILD_INFO_NAME} does not match the release contract")
    if expected_commit is not None:
        validate_commit(expected_commit)
        require(
            value["commit"] == expected_commit,
            f"{BUILD_INFO_NAME} commit does not match expected source commit",
        )
    return value


def read_archive(
    archive_path: Path,
    *,
    version: str,
    item: Platform,
    expected_commit: str | None,
) -> None:
    root = archive_root(version, item)
    launcher_name = f"{root}/bin/{BINARY_NAME}"
    info_name = f"{root}/{BUILD_INFO_NAME}"
    try:
        with tarfile.open(archive_path, "r:gz") as archive:
            members = archive.getmembers()
            require(bool(members), f"{archive_path.name} is empty")
            files: dict[str, tarfile.TarInfo] = {}
            seen: set[str] = set()
            for member in members:
                path = safe_archive_path(member.name)
                name = path.as_posix()
                require(name not in seen, f"{archive_path.name} repeats {name}")
                seen.add(name)
                require(
                    name == root or name.startswith(f"{root}/"),
                    f"{archive_path.name} has a member outside its bundle root",
                )
                if member.isdir():
                    continue
                if member.isreg():
                    files[name] = member
                    continue
                require(
                    member.issym() or member.islnk(),
                    f"{archive_path.name} contains an unsupported member: {name}",
                )
                safe_archive_link(member, root)
            require(
                launcher_name in files,
                f"{archive_path.name} is missing its CLI launcher",
            )
            require(
                info_name in files,
                f"{archive_path.name} is missing {BUILD_INFO_NAME}",
            )
            require(
                files[launcher_name].mode & 0o111 != 0,
                f"{archive_path.name} launcher is not executable",
            )
            require(
                any(
                    name.startswith(f"{root}/lib/") and "python" in Path(name).name.lower()
                    for name in files
                ),
                f"{archive_path.name} is missing its Python runtime library",
            )
            require(
                any(
                    name.startswith(f"{root}/python/lib/python3.")
                    and name.endswith("/encodings/__init__.py")
                    for name in files
                ),
                f"{archive_path.name} is missing the Python standard library",
            )
            metadata = archive.extractfile(files[info_name])
            require(metadata is not None, f"{archive_path.name} build metadata is unreadable")
            validate_build_info(
                metadata.read(),
                version=version,
                item=item,
                expected_commit=expected_commit,
            )
    except (OSError, tarfile.TarError) as error:
        raise CliReleaseError(f"{archive_path.name} is not a readable gzip tar archive: {error}") from error


def package_archive(
    *,
    bundle_root: Path,
    output: Path,
    version: str,
    item: Platform,
    commit: str,
) -> None:
    try:
        bundle_root = bundle_root.resolve(strict=True)
    except OSError as error:
        raise CliReleaseError(f"failed to resolve CLI bundle {bundle_root}: {error}") from error
    launcher = bundle_root / "bin" / BINARY_NAME
    require(bundle_root.is_dir(), f"CLI bundle does not exist: {bundle_root}")
    require(launcher.is_file(), f"CLI bundle is missing its launcher: {launcher}")
    require(os.access(launcher, os.X_OK), f"CLI bundle launcher is not executable: {launcher}")
    require(
        not (bundle_root / BUILD_INFO_NAME).exists(),
        f"CLI bundle must not predefine {BUILD_INFO_NAME}",
    )
    metadata = json.dumps(
        build_info(version=version, item=item, commit=commit),
        indent=2,
        sort_keys=True,
    ).encode("utf-8") + b"\n"
    root = archive_root(version, item)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(output, "w:gz") as archive:
        archive.dereference = False
        archive.add(str(bundle_root), arcname=root, recursive=True)
        metadata_info = tarfile.TarInfo(f"{root}/{BUILD_INFO_NAME}")
        metadata_info.mode = 0o644
        metadata_info.size = len(metadata)
        archive.addfile(metadata_info, fileobj=io.BytesIO(metadata))
    read_archive(output, version=version, item=item, expected_commit=commit)


def extract_archive(
    archive_path: Path,
    *,
    destination: Path,
    version: str,
    item: Platform,
    expected_commit: str | None,
) -> Path:
    read_archive(
        archive_path,
        version=version,
        item=item,
        expected_commit=expected_commit,
    )
    root = archive_root(version, item)
    root_path = destination / root
    destination.mkdir(parents=True, exist_ok=True)
    try:
        with tarfile.open(archive_path, "r:gz") as archive:
            members = archive.getmembers()
            symlinks: list[tuple[tarfile.TarInfo, Path]] = []
            hardlinks: list[tuple[tarfile.TarInfo, Path]] = []
            for member in members:
                path = safe_archive_path(member.name)
                target = destination / path
                require(
                    target.resolve().is_relative_to(destination.resolve()),
                    f"archive member escapes extraction directory: {member.name}",
                )
                if member.isdir():
                    target.mkdir(parents=True, exist_ok=True)
                    continue
                if member.issym():
                    safe_archive_link(member, root)
                    symlinks.append((member, target))
                    continue
                if member.islnk():
                    safe_archive_link(member, root)
                    hardlinks.append((member, target))
                    continue
                require(member.isreg(), f"archive contains an unsupported member: {member.name}")
                target.parent.mkdir(parents=True, exist_ok=True)
                payload = archive.extractfile(member)
                require(payload is not None, f"archive member is unreadable: {member.name}")
                with target.open("wb") as handle:
                    shutil.copyfileobj(payload, handle)
                target.chmod(member.mode)
            for member, target in symlinks:
                target.parent.mkdir(parents=True, exist_ok=True)
                os.symlink(member.linkname, target)
            for member, target in hardlinks:
                target.parent.mkdir(parents=True, exist_ok=True)
                source = destination / safe_archive_link(member, root)
                require(source.exists(), f"archive hardlink target is missing: {member.name}")
                os.link(source, target, follow_symlinks=False)
            for _, target in symlinks:
                resolved = target.resolve(strict=False)
                require(
                    resolved.is_relative_to(destination.resolve()) and resolved.exists(),
                    f"archive has a broken or escaping symlink: {target.relative_to(destination)}",
                )
    except (OSError, tarfile.TarError) as error:
        raise CliReleaseError(f"failed to extract {archive_path.name}: {error}") from error
    launcher = root_path / "bin" / BINARY_NAME
    require(launcher.is_file(), f"archive did not extract {BINARY_NAME}")
    require(os.access(launcher, os.X_OK), f"archive extracted a non-executable {BINARY_NAME}")
    return launcher


def run_checked(command: list[str]) -> str:
    try:
        result = subprocess.run(command, check=False, capture_output=True, text=True)
    except OSError as error:
        raise CliReleaseError(f"failed to execute {' '.join(command)}: {error}") from error
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip()
        raise CliReleaseError(
            f"{' '.join(command)} failed with exit code {result.returncode}: {detail}"
        )
    return result.stdout


def smoke_binary(binary: Path, version: str) -> None:
    require(binary.is_file(), f"CLI binary does not exist: {binary}")
    require(os.access(binary, os.X_OK), f"CLI binary is not executable: {binary}")
    expected_version = f"{BINARY_NAME} {version}\n"
    for arguments in (["--version"], ["cli", "--version"]):
        output = run_checked([str(binary), *arguments])
        require(
            output == expected_version,
            f"{' '.join(arguments)} returned an unexpected version: {output!r}",
        )
    help_output = run_checked([str(binary), "cli", "--help"])
    require("Usage:" in help_output, "CLI help has no usage section")
    require(
        "Usage: logos-inspector cli" in help_output,
        "CLI help exposes an internal launcher name",
    )
    require("source-policy" in help_output, "CLI help is missing source-policy")
    source_policy = run_checked([str(binary), "cli", "source-policy"])
    try:
        report = json.loads(source_policy)
    except json.JSONDecodeError as error:
        raise CliReleaseError(f"source-policy did not emit JSON: {error}") from error
    require(isinstance(report, dict), "source-policy did not emit a JSON object")
    require(isinstance(report.get("version"), int), "source-policy JSON has no version")
    require(isinstance(report.get("defaults"), dict), "source-policy JSON has no defaults")
    require(
        isinstance(report.get("source_modes"), dict),
        "source-policy JSON has no source mode catalog",
    )


def verify_release(
    directory: Path,
    *,
    version: str,
    write_checksum_file: bool,
    expected_commit: str | None,
) -> None:
    require(directory.is_dir(), f"release input is not a directory: {directory}")
    items = sorted(path.name for path in directory.iterdir())
    asset_names = tuple(asset_name(version, item) for item in PLATFORMS.values())
    expected = sorted(asset_names if write_checksum_file else (*asset_names, "SHA256SUMS"))
    require(items == expected, "release contains missing or unexpected assets")
    for item in PLATFORMS.values():
        read_archive(
            directory / asset_name(version, item),
            version=version,
            item=item,
            expected_commit=expected_commit,
        )
    if write_checksum_file:
        write_checksums(directory, asset_names)
    else:
        verify_checksums(directory, asset_names)


def self_test() -> None:
    version = "0.2.0-alpha.1"
    commit = "a" * 40
    with tempfile.TemporaryDirectory(prefix="inspector-cli-release-test-") as temporary:
        root = Path(temporary)
        fixtures = root / "fixtures"
        directory = root / "release"
        fixtures.mkdir()
        directory.mkdir()
        for item in PLATFORMS.values():
            bundle = fixtures / item.name
            launcher = bundle / "bin" / BINARY_NAME
            runtime_library = bundle / "lib" / "libpython-fixture.so"
            encodings = bundle / "python" / "lib" / "python3.14" / "encodings"
            launcher.parent.mkdir(parents=True)
            runtime_library.parent.mkdir(parents=True)
            encodings.mkdir(parents=True)
            launcher.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            launcher.chmod(0o755)
            runtime_library.write_bytes(b"fixture")
            (encodings / "__init__.py").write_text("", encoding="utf-8")
            linked_bundle = fixtures / f"{item.name}-link"
            os.symlink(bundle, linked_bundle)
            package_archive(
                bundle_root=linked_bundle,
                output=directory / asset_name(version, item),
                version=version,
                item=item,
                commit=commit,
            )
        verify_release(
            directory,
            version=version,
            write_checksum_file=True,
            expected_commit=commit,
        )
        verify_release(
            directory,
            version=version,
            write_checksum_file=False,
            expected_commit=commit,
        )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate Logos Inspector CLI release artifacts")
    subcommands = parser.add_subparsers(dest="command", required=True)

    source = subcommands.add_parser("validate-source")
    source.add_argument("--root", type=Path, default=ROOT)

    package = subcommands.add_parser("package")
    package.add_argument("--bundle-root", required=True, type=Path)
    package.add_argument("--output", required=True, type=Path)
    package.add_argument("--version", required=True)
    package.add_argument("--platform", required=True, choices=tuple(sorted(PLATFORMS)))
    package.add_argument("--commit", required=True)

    verify = subcommands.add_parser("verify")
    verify.add_argument("--input-dir", required=True, type=Path)
    verify.add_argument("--version", required=True)
    verify.add_argument("--write-checksums", action="store_true")
    verify.add_argument("--commit")

    smoke = subcommands.add_parser("smoke")
    smoke.add_argument("--binary", required=True, type=Path)
    smoke.add_argument("--version", required=True)

    archive_smoke = subcommands.add_parser("smoke-archive")
    archive_smoke.add_argument("--archive", required=True, type=Path)
    archive_smoke.add_argument("--version", required=True)
    archive_smoke.add_argument("--platform", required=True, choices=tuple(sorted(PLATFORMS)))
    archive_smoke.add_argument("--commit")

    subcommands.add_parser("self-test")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.command == "validate-source":
            print(source_version(args.root))
        elif args.command == "package":
            package_archive(
                bundle_root=args.bundle_root,
                output=args.output,
                version=args.version,
                item=platform(args.platform),
                commit=args.commit,
            )
        elif args.command == "verify":
            verify_release(
                args.input_dir,
                version=args.version,
                write_checksum_file=args.write_checksums,
                expected_commit=args.commit,
            )
        elif args.command == "smoke":
            smoke_binary(args.binary, args.version)
        elif args.command == "smoke-archive":
            item = platform(args.platform)
            with tempfile.TemporaryDirectory(prefix="inspector-cli-release-smoke-") as temporary:
                binary = extract_archive(
                    args.archive,
                    destination=Path(temporary),
                    version=args.version,
                    item=item,
                    expected_commit=args.commit,
                )
                smoke_binary(binary, args.version)
        elif args.command == "self-test":
            self_test()
        else:
            raise CliReleaseError(f"unsupported command `{args.command}`")
    except CliReleaseError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
