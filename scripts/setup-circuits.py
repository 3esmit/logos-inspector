#!/usr/bin/env python3
import argparse
import os
import platform
import shutil
import stat
import sys
import tarfile
import tempfile
import urllib.request
from pathlib import Path, PurePosixPath
from typing import Optional

from build_artifacts import (
    circuit_artifact_name,
    circuit_artifact_url,
    circuit_target_by_platform,
    circuits_release,
    load_catalog,
)


def main() -> int:
    parser = argparse.ArgumentParser(description="Install Logos blockchain circuits")
    parser.add_argument("version", nargs="?")
    parser.add_argument(
        "install_dir",
        nargs="?",
    )
    parser.add_argument(
        "--install-dir",
        dest="install_dir_option",
        help="installation directory; keeps version optional",
    )
    args = parser.parse_args()

    catalog = load_catalog()
    release = args.version or circuits_release(catalog)
    target = current_target(catalog)
    artifact = circuit_artifact_name(release, target)
    url = circuit_artifact_url(catalog, release, target)
    install_dir_arg = args.install_dir_option or args.install_dir or str(
        Path.home() / ".logos-blockchain-circuits"
    )
    install_dir = Path(install_dir_arg).expanduser().resolve()

    with tempfile.TemporaryDirectory(prefix="logos-circuits-") as tmp:
        archive = Path(tmp) / artifact
        print(f"downloading {url}")
        urllib.request.urlretrieve(url, archive)

        if install_dir.exists():
            shutil.rmtree(install_dir)
        install_dir.mkdir(parents=True, exist_ok=True)
        extract_archive(archive, install_dir)

    print(f"installed {release} at {install_dir}")
    print(f"LOGOS_BLOCKCHAIN_CIRCUITS={install_dir}")
    print(f"POSIX: export LOGOS_BLOCKCHAIN_CIRCUITS={install_dir}")
    print(f"PowerShell: $env:LOGOS_BLOCKCHAIN_CIRCUITS='{install_dir}'")
    return 0


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
        target.relative_to(root)
    except ValueError as err:
        raise RuntimeError(f"unsafe archive path: {relative}") from err
    return target


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as err:
        print(f"error: {err}", file=sys.stderr)
        raise SystemExit(1)
