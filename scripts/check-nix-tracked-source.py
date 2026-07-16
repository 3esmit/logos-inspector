#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SENTINELS = (
    Path(".nix-untracked-source-sentinel"),
    Path("qml/.nix-untracked-source-sentinel"),
    Path("core/tests/.nix-untracked-source-sentinel"),
    Path("nix/logos-protocol-overlay/cpp/.nix-untracked-source-sentinel"),
)


def run(*command: str, cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=cwd,
        check=True,
        capture_output=True,
        text=True,
    )


def tracked_files() -> set[Path]:
    output = subprocess.run(
        ("git", "ls-files", "-z"),
        cwd=ROOT,
        check=True,
        capture_output=True,
    ).stdout
    return {Path(item.decode()) for item in output.split(b"\0") if item}


def copy_tracked_tree(destination: Path) -> None:
    for relative_path in sorted(tracked_files()):
        source = ROOT / relative_path
        target = destination / relative_path
        target.parent.mkdir(parents=True, exist_ok=True)
        if source.is_symlink():
            target.symlink_to(os.readlink(source))
        elif source.is_file():
            shutil.copy2(source, target)
        else:
            raise RuntimeError(f"tracked path is not a file or symlink: {relative_path}")


def initialize_repository(snapshot_root: Path) -> None:
    run("git", "init", "--quiet", cwd=snapshot_root)
    run("git", "add", "--all", cwd=snapshot_root)
    run(
        "git",
        "-c",
        "user.name=Build Input Check",
        "-c",
        "user.email=build-input-check@example.invalid",
        "-c",
        "commit.gpgsign=false",
        "-c",
        "core.hooksPath=/dev/null",
        "commit",
        "--quiet",
        "--no-gpg-sign",
        "--no-verify",
        "-m",
        "test fixture",
        cwd=snapshot_root,
    )


def create_sentinels(snapshot_root: Path) -> None:
    for relative_path in SENTINELS:
        path = snapshot_root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("untracked\n", encoding="utf-8")


def verify_flake_source(snapshot_root: Path) -> None:
    metadata = json.loads(
        run(
            "nix",
            "flake",
            "metadata",
            "--json",
            "--no-write-lock-file",
            ".",
            cwd=snapshot_root,
        ).stdout
    )
    resolved_url = metadata.get("resolvedUrl", "")
    if not resolved_url.startswith("git+file:"):
        raise RuntimeError(f"flake did not resolve through Git: {resolved_url}")

    archive = json.loads(
        run(
            "nix",
            "flake",
            "archive",
            "--json",
            "--no-write-lock-file",
            ".",
            cwd=snapshot_root,
        ).stdout
    )
    store_source = Path(archive["path"])
    leaked = [path for path in SENTINELS if (store_source / path).exists()]
    if leaked:
        paths = ", ".join(str(path) for path in leaked)
        raise RuntimeError(f"untracked files leaked into the Nix source: {paths}")
    if not (store_source / "flake.nix").is_file():
        entries = ", ".join(sorted(path.name for path in store_source.iterdir()))
        raise RuntimeError(
            f"tracked flake.nix is missing from Nix source {store_source}: {entries}"
        )


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="logos-inspector-nix-source-") as temporary:
        snapshot_root = Path(temporary) / "repo"
        snapshot_root.mkdir()
        copy_tracked_tree(snapshot_root)
        initialize_repository(snapshot_root)
        create_sentinels(snapshot_root)
        verify_flake_source(snapshot_root)

    print("Nix flake source is restricted to Git-tracked files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
