#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SENTINEL_SOURCE = 'compile_error!("untracked Cargo target was discovered");\n'
SENTINELS = (
    Path("build.rs"),
    Path("crates/core-ffi/build.rs"),
    Path("crates/standalone-gui/src/lib.rs"),
    Path("src/bin/__untracked_build_input.rs"),
    Path("tests/__untracked_build_input.rs"),
    Path("examples/__untracked_build_input.rs"),
    Path("benches/__untracked_build_input.rs"),
    Path("crates/core-ffi/src/bin/__untracked_build_input.rs"),
    Path("crates/core-ffi/tests/__untracked_build_input.rs"),
    Path("crates/core-ffi/examples/__untracked_build_input.rs"),
    Path("crates/core-ffi/benches/__untracked_build_input.rs"),
    Path("crates/standalone-gui/src/bin/__untracked_build_input.rs"),
    Path("crates/standalone-gui/tests/__untracked_build_input.rs"),
    Path("crates/standalone-gui/examples/__untracked_build_input.rs"),
    Path("crates/standalone-gui/benches/__untracked_build_input.rs"),
)


def tracked_files() -> set[Path]:
    output = subprocess.run(
        ("git", "ls-files", "-z"),
        cwd=ROOT,
        check=True,
        capture_output=True,
    ).stdout
    return {Path(item.decode()) for item in output.split(b"\0") if item}


def copy_tracked_tree(destination: Path, tracked: set[Path]) -> None:
    for relative_path in sorted(tracked):
        source = ROOT / relative_path
        target = destination / relative_path
        target.parent.mkdir(parents=True, exist_ok=True)
        if source.is_symlink():
            target.symlink_to(os.readlink(source))
        elif source.is_file():
            shutil.copy2(source, target)
        else:
            raise RuntimeError(f"tracked path is not a file or symlink: {relative_path}")


def discovered_target_sources(snapshot_root: Path) -> set[Path]:
    metadata = json.loads(
        subprocess.run(
            (
                "cargo",
                "metadata",
                "--locked",
                "--no-deps",
                "--format-version",
                "1",
            ),
            cwd=snapshot_root,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    )
    return {
        Path(target["src_path"]).resolve().relative_to(snapshot_root)
        for package in metadata["packages"]
        for target in package["targets"]
    }


def create_sentinels(snapshot_root: Path) -> None:
    for relative_path in SENTINELS:
        path = snapshot_root / relative_path
        if path.exists() or path.is_symlink():
            raise RuntimeError(f"sentinel path is already Git-tracked: {relative_path}")
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(SENTINEL_SOURCE, encoding="utf-8")


def main() -> int:
    tracked = tracked_files()
    with tempfile.TemporaryDirectory(prefix="logos-inspector-build-inputs-") as temporary:
        snapshot_root = Path(temporary) / "repo"
        copy_tracked_tree(snapshot_root, tracked)
        create_sentinels(snapshot_root)
        untracked_targets = discovered_target_sources(snapshot_root) - tracked
        if untracked_targets:
            paths = "\n".join(f"- {path}" for path in sorted(untracked_targets))
            raise RuntimeError(f"Cargo discovered untracked build targets:\n{paths}")

    print("Cargo target discovery is restricted to Git-tracked files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
