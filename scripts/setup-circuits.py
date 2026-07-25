#!/usr/bin/env python3
"""Install Logos blockchain circuits with hash verification and cache reuse."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import platform
import shutil
import stat
import sys
import tarfile
import tempfile
import urllib.request
from pathlib import Path, PurePosixPath
from typing import Any, Optional

from build_artifacts import (
    circuit_artifact_name,
    circuit_artifact_url,
    circuit_target_by_platform,
    circuits_release,
    load_catalog,
)

MARKER_NAME = ".circuits-install.json"
DEFAULT_CACHE_ROOT = Path.home() / ".cache" / "logos-blockchain-circuits"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Install Logos blockchain circuits")
    parser.add_argument("version", nargs="?")
    parser.add_argument("install_dir", nargs="?")
    parser.add_argument(
        "--install-dir",
        dest="install_dir_option",
        help="installation directory; keeps version optional",
    )
    parser.add_argument(
        "--cache-dir",
        dest="cache_dir",
        help="persistent archive cache directory",
    )
    args = parser.parse_args(argv)

    catalog = load_catalog()
    release = args.version or circuits_release(catalog)
    target = current_target(catalog)
    artifact = circuit_artifact_name(release, target)
    expected_hash = str(target["hash"])
    url = circuit_artifact_url(catalog, release, target)
    install_dir = Path(
        args.install_dir_option or args.install_dir or str(DEFAULT_CACHE_ROOT / "install")
    ).expanduser()
    cache_dir = Path(
        args.cache_dir or os.environ.get("LOGOS_CIRCUITS_CACHE_DIR") or str(DEFAULT_CACHE_ROOT / "archives")
    ).expanduser()

    setup_circuits(
        release=release,
        target=target,
        artifact=artifact,
        expected_hash=expected_hash,
        url=url,
        install_dir=install_dir,
        cache_dir=cache_dir,
    )
    print(f"installed {release} at {install_dir.resolve()}")
    print(f"LOGOS_BLOCKCHAIN_CIRCUITS={install_dir.resolve()}")
    print(f"POSIX: export LOGOS_BLOCKCHAIN_CIRCUITS={install_dir.resolve()}")
    print(f"PowerShell: $env:LOGOS_BLOCKCHAIN_CIRCUITS='{install_dir.resolve()}'")
    return 0


def setup_circuits(
    *,
    release: str,
    target: dict[str, str],
    artifact: str,
    expected_hash: str,
    url: str,
    install_dir: Path,
    cache_dir: Path,
) -> Path:
    install_dir = install_dir.expanduser()
    cache_dir = cache_dir.expanduser()
    cache_dir.mkdir(parents=True, exist_ok=True)

    expected_digest = decode_sri_sha256(expected_hash)
    marker = marker_payload(
        release=release,
        target=target,
        artifact=artifact,
        expected_hash=expected_hash,
    )
    if installation_matches(install_dir, marker):
        print(f"circuits already verified at {install_dir.resolve()}")
        return install_dir

    archive_path = cache_dir / artifact
    if archive_path.is_file():
        actual = sha256_file(archive_path)
        if actual != expected_digest:
            archive_path.unlink(missing_ok=True)
            archive_path = download_archive(url, cache_dir / artifact)
            actual = sha256_file(archive_path)
    else:
        archive_path = download_archive(url, cache_dir / artifact)
        actual = sha256_file(archive_path)

    if actual != expected_digest:
        raise RuntimeError(
            f"circuit archive digest mismatch for {artifact}: "
            f"expected {expected_hash}, got sha256-{base64.b64encode(actual).decode('ascii')}"
        )

    with tempfile.TemporaryDirectory(prefix="logos-circuits-extract-") as tmp:
        extract_root = Path(tmp) / "root"
        extract_root.mkdir(parents=True, exist_ok=True)
        extract_archive(archive_path, extract_root)
        if install_dir.exists():
            shutil.rmtree(install_dir)
        install_dir.parent.mkdir(parents=True, exist_ok=True)
        shutil.move(str(extract_root), str(install_dir))

    write_marker(install_dir, marker)
    return install_dir


def decode_sri_sha256(value: str) -> bytes:
    if not value or "-" not in value:
        raise ValueError(f"malformed SRI hash: {value!r}")
    algorithm, encoded = value.split("-", 1)
    if algorithm != "sha256":
        raise ValueError(f"unsupported hash algorithm: {algorithm}")
    if not encoded:
        raise ValueError(f"malformed SRI hash: {value!r}")
    try:
        digest = base64.b64decode(encoded, validate=True)
    except Exception as err:  # noqa: BLE001 - surface clear hash errors
        raise ValueError(f"malformed SRI hash: {value!r}") from err
    if len(digest) != 32:
        raise ValueError(f"malformed SRI hash length for {value!r}")
    return digest


def sha256_file(path: Path) -> bytes:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
    return digest.digest()


def download_archive(url: str, destination: Path) -> Path:
    destination.parent.mkdir(parents=True, exist_ok=True)
    partial = destination.with_suffix(destination.suffix + ".partial")
    print(f"downloading {url}")
    try:
        with urllib.request.urlopen(url) as response, partial.open("wb") as output:
            shutil.copyfileobj(response, output)
    except Exception as err:  # noqa: BLE001
        partial.unlink(missing_ok=True)
        raise RuntimeError(f"failed to download circuit archive from {url}: {err}") from err
    partial.replace(destination)
    return destination


def marker_payload(
    *,
    release: str,
    target: dict[str, str],
    artifact: str,
    expected_hash: str,
) -> dict[str, Any]:
    return {
        "release": release,
        "platform": {
            "os": target["os"],
            "arch": target["arch"],
        },
        "artifact": artifact,
        "hash": expected_hash,
    }


def installation_matches(install_dir: Path, expected: dict[str, Any]) -> bool:
    marker_path = install_dir / MARKER_NAME
    if not install_dir.is_dir() or not marker_path.is_file():
        return False
    try:
        actual = json.loads(marker_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return False
    return actual == expected


def write_marker(install_dir: Path, marker: dict[str, Any]) -> None:
    path = install_dir / MARKER_NAME
    path.write_text(json.dumps(marker, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def current_target(catalog: dict) -> dict[str, str]:
    system = platform.system().lower()
    machine = platform.machine().lower()

    if system == "linux":
        os_name = "linux"
    elif system == "darwin":
        os_name = "macos"
    elif system == "windows" or system.startswith(("msys", "mingw", "cygwin")):
        os_name = "windows"
    else:
        raise SystemExit(f"unsupported OS: {platform.system()}")

    if machine in {"x86_64", "amd64"}:
        arch = "x86_64"
    elif machine in {"aarch64", "arm64"}:
        arch = "aarch64"
    else:
        raise SystemExit(f"unsupported architecture: {platform.machine()}")

    return circuit_target_by_platform(catalog, os_name, arch)


def extract_archive(archive: Path, install_dir: Path) -> None:
    with tarfile.open(archive, "r:gz") as tar:
        for member in tar.getmembers():
            stripped = strip_first_path_component(member.name)
            if stripped is None:
                continue

            target = safe_target_path(install_dir, stripped)
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
                continue

            if member.isfile():
                target.parent.mkdir(parents=True, exist_ok=True)
                source = tar.extractfile(member)
                if source is None:
                    raise RuntimeError(f"failed to read archive member {member.name}")
                with source, target.open("wb") as output:
                    shutil.copyfileobj(source, output)
                target.chmod(member.mode & (stat.S_IRWXU | stat.S_IRWXG | stat.S_IRWXO))
                continue

            if member.issym():
                link = PurePosixPath(member.linkname)
                if link.is_absolute() or ".." in link.parts:
                    raise RuntimeError(f"unsafe symlink target in archive: {member.linkname}")
                target.parent.mkdir(parents=True, exist_ok=True)
                if target.exists() or target.is_symlink():
                    target.unlink()
                os.symlink(member.linkname, target)
                continue

            raise RuntimeError(f"unsupported archive member type: {member.name}")


def strip_first_path_component(name: str) -> Optional[Path]:
    path = PurePosixPath(name)
    if path.is_absolute() or ".." in path.parts:
        raise RuntimeError(f"unsafe archive path: {name}")

    parts = path.parts[1:]
    if not parts:
        return None
    return Path(*parts)


def safe_target_path(root: Path, relative: Path) -> Path:
    target = (root / relative).resolve()
    try:
        target.relative_to(root.resolve())
    except ValueError as err:
        raise RuntimeError(f"unsafe archive path: {relative}") from err
    return target


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as err:
        print(f"error: {err}", file=sys.stderr)
        raise SystemExit(1)
