#!/usr/bin/env python3
"""Download a checksum-verified actionlint binary and lint GitHub workflows."""

from __future__ import annotations

import hashlib
import os
import platform
import stat
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
from pathlib import Path
from shutil import which

VERSION = "1.7.12"
# Official checksum published with the v1.7.12 release assets.
LINUX_AMD64_SHA256 = "8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8"


def main() -> int:
    existing = which("actionlint")
    if existing is not None:
        return subprocess.call([existing, "-color", *workflow_args()])

    system = platform.system().lower()
    machine = platform.machine().lower()
    if system != "linux":
        print("actionlint bootstrap currently supports Linux CI runners only", file=sys.stderr)
        return 2
    if machine in {"x86_64", "amd64"}:
        target = "linux_amd64"
        expected = LINUX_AMD64_SHA256
    else:
        # Resolve other Linux architectures from the official checksum file.
        target = f"linux_{'arm64' if machine in {'aarch64', 'arm64'} else machine}"
        checksums = fetch_official_checksums()
        expected = checksums.get(f"actionlint_{VERSION}_{target}.tar.gz")
        if not expected:
            print(f"unsupported architecture for actionlint: {machine}", file=sys.stderr)
            return 2

    # Always cross-check the published checksum list for the pinned AMD64 digest.
    published = fetch_official_checksums()
    if published.get(f"actionlint_{VERSION}_linux_amd64.tar.gz") != LINUX_AMD64_SHA256:
        raise RuntimeError("actionlint linux_amd64 checksum does not match the pinned release digest")
    if target != "linux_amd64":
        expected = published[f"actionlint_{VERSION}_{target}.tar.gz"]

    url = (
        f"https://github.com/rhysd/actionlint/releases/download/v{VERSION}/"
        f"actionlint_{VERSION}_{target}.tar.gz"
    )
    cache_root = Path(os.environ.get("XDG_CACHE_HOME") or (Path.home() / ".cache"))
    binary = cache_root / "actionlint" / VERSION / target / "actionlint"
    if not binary.is_file():
        install_actionlint(url, expected, binary)

    return subprocess.call([str(binary), "-color", *workflow_args()])


def workflow_args() -> list[str]:
    root = Path(__file__).resolve().parents[1]
    paths: list[str] = []
    for pattern in ("*.yml", "*.yaml"):
        paths.extend(str(path) for path in sorted((root / ".github" / "workflows").glob(pattern)))
    return paths


def fetch_official_checksums() -> dict[str, str]:
    url = (
        f"https://github.com/rhysd/actionlint/releases/download/v{VERSION}/"
        f"actionlint_{VERSION}_checksums.txt"
    )
    with urllib.request.urlopen(url) as response:
        text = response.read().decode("utf-8")
    values: dict[str, str] = {}
    for line in text.splitlines():
        parts = line.split()
        if len(parts) == 2:
            values[parts[1]] = parts[0]
    return values


def install_actionlint(url: str, expected_sha256: str, binary: Path) -> None:
    binary.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="actionlint-") as tmp:
        archive = Path(tmp) / "actionlint.tar.gz"
        with urllib.request.urlopen(url) as response, archive.open("wb") as output:
            output.write(response.read())
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        if digest != expected_sha256:
            raise RuntimeError(
                f"actionlint archive digest mismatch: expected {expected_sha256}, got {digest}"
            )
        with tarfile.open(archive, "r:gz") as tar:
            member = tar.getmember("actionlint")
            extracted = tar.extractfile(member)
            if extracted is None:
                raise RuntimeError("actionlint binary missing from archive")
            binary.write_bytes(extracted.read())
        binary.chmod(binary.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as err:  # noqa: BLE001
        print(f"error: {err}", file=sys.stderr)
        raise SystemExit(1)
