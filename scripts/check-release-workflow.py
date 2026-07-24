#!/usr/bin/env python3
"""Check source-owned LGX and standalone release workflow contracts."""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKFLOWS = {
    "core": ROOT / ".github" / "workflows" / "release-core.yml",
    "ui": ROOT / ".github" / "workflows" / "release-ui.yml",
    "standalone": ROOT / ".github" / "workflows" / "release-standalone.yml",
}
RELEASE_ACTION_SHA = "81f506530c56e8757e6d99ee7f9d4c092e74411c"
ACTION_SHA = re.compile(r"^[0-9a-f]{40}$")
FORK_INPUTS = {
    "blockchain_module": (
        "3esmit/logos-blockchain-module",
        "c81cdd5f349430cff3765d6631e285de6b5c7a50",
    ),
    "storage_module": (
        "3esmit/logos-storage-module",
        "cb1f934a13e35016553c670489af5fc1df8169e6",
    ),
    "delivery_module": (
        "3esmit/logos-delivery-module",
        "ca77bcb8b59f960fcc5040412dc4e3a755161631",
    ),
    "lez_core": (
        "3esmit/logos-execution-zone-module",
        "930262a80f7d934acd88244ba130ced786bff83b",
    ),
}
BUNDLER_INPUTS = {
    "nix-bundle-dir": (
        "logos-co/nix-bundle-dir",
        "4f72d7a64dd83979d771c17161f23ebc9dbedb40",
    ),
    "nix-bundle-appimage": (
        "logos-co/nix-bundle-appimage",
        "8fcc56b5afcc313ca917cf3487be082ae2f0184c",
    ),
    "nix-bundle-macos-app": (
        "logos-co/nix-bundle-macos-app",
        "d6b0cc518e599ab7a52258bf3e1f8123c8a01d31",
    ),
}


def read(path: Path, errors: list[str]) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as error:
        errors.append(f"failed to read {path.relative_to(ROOT)}: {error}")
        return ""


def require(text: str, needles: tuple[str, ...], label: str, errors: list[str]) -> None:
    for needle in needles:
        if needle not in text:
            errors.append(f"{label} is missing `{needle}`")


def check_pinned_actions(text: str, label: str, errors: list[str]) -> None:
    for line in text.splitlines():
        match = re.search(r"\buses:\s*[^@\s]+@([^\s#]+)", line)
        if match is None:
            continue
        reference = match.group(1)
        if not ACTION_SHA.fullmatch(reference):
            errors.append(f"{label} uses mutable action reference `{reference}`")


def flake_input(text: str, name: str) -> tuple[str, str] | None:
    pattern = re.compile(
        rf"{re.escape(name)}\s*=\s*\{{.*?"
        r'url\s*=\s*"github:([^"?]+)\?rev=([0-9a-f]{40})";',
        re.DOTALL,
    )
    match = pattern.search(text)
    if match is None:
        return None
    return match.group(1), match.group(2)


def run_check(command: list[str], label: str, errors: list[str]) -> None:
    result = subprocess.run(command, cwd=ROOT, check=False, capture_output=True, text=True)
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip()
        errors.append(f"{label} failed: {detail}")


def main() -> int:
    errors: list[str] = []
    texts = {name: read(path, errors) for name, path in WORKFLOWS.items()}
    core = texts["core"]
    ui = texts["ui"]
    standalone = texts["standalone"]
    release_action = (
        "3esmit/logos-modules-release-action/.github/workflows/release.yml"
        f"@{RELEASE_ACTION_SHA}"
    )

    common_lgx = (
        "workflow_dispatch:",
        release_action,
        "variants: linux-amd64,darwin-arm64",
        "require_all_variants: true",
        "dispatch_rebuild_index: false",
        "prerelease: true",
        "signing_mode: none",
    )
    require(core, common_lgx, "core release workflow", errors)
    require(
        core,
        (
            "metadata_path: core/metadata.json",
            "build_attr: core-lgx-portable",
            "install_macos_metal_toolchain: true",
        ),
        "core release workflow",
        errors,
    )
    require(ui, common_lgx, "UI release workflow", errors)
    require(
        ui,
        (
            "metadata_path: metadata.json",
            "build_attr: lgx-portable",
            "install_macos_metal_toolchain: false",
        ),
        "UI release workflow",
        errors,
    )
    require(
        standalone,
        (
            "workflow_dispatch:",
            ".#standalone-appimage",
            ".#standalone-macos-app",
            "unshare --mount",
            "mount -t tmpfs tmpfs /nix/store",
            "verify-tree",
            "audit-binary-refs",
            "standalone_release.py verify",
            "draft: true",
            "prerelease: true",
            "gh release download",
            "gh release edit",
            "--draft=false",
        ),
        "standalone release workflow",
        errors,
    )
    for name, text in texts.items():
        label = f"{name} release workflow"
        for forbidden in (
            "catalog_e2e_evidence_url",
            "dispatch_rebuild_index: true",
            "logos-3esmit-release",
        ):
            if forbidden in text:
                errors.append(f"{label} retains catalog-coupled input `{forbidden}`")
        if "\npush:" in text:
            errors.append(f"{label} must remain manual during alpha")
    for path in sorted((ROOT / ".github" / "workflows").glob("*.yml")):
        check_pinned_actions(
            read(path, errors),
            f"{path.name} workflow",
            errors,
        )

    if (ROOT / ".github" / "workflows" / "release-alpha.yml").exists():
        errors.append("obsolete combined alpha release workflow still exists")
    if (ROOT / "scripts" / "release_artifacts.py").exists():
        errors.append("obsolete combined release artifact tool still exists")

    flake = read(ROOT / "flake.nix", errors)
    expected_inputs = {**FORK_INPUTS, **BUNDLER_INPUTS}
    for name, expected in expected_inputs.items():
        actual = flake_input(flake, name)
        if actual != expected:
            errors.append(
                f"flake input {name} must be github:{expected[0]}?rev={expected[1]}; "
                f"found {actual}"
            )
    try:
        flake_lock = json.loads((ROOT / "flake.lock").read_text(encoding="utf-8"))
        root_node = flake_lock["nodes"][flake_lock["root"]]
        root_inputs = root_node["inputs"]
    except (KeyError, OSError, json.JSONDecodeError, TypeError) as error:
        errors.append(f"failed to read root flake lock inputs: {error}")
    else:
        for name, (repository, revision) in expected_inputs.items():
            node_name = root_inputs.get(name)
            node = flake_lock["nodes"].get(node_name, {})
            locked = node.get("locked", {})
            owner, repo = repository.split("/", maxsplit=1)
            actual = (locked.get("owner"), locked.get("repo"), locked.get("rev"))
            expected = (owner, repo, revision)
            if actual != expected:
                errors.append(
                    f"locked input {name} must resolve to "
                    f"github:{repository}/{revision}; found {actual}"
                )
    require(
        flake,
        (
            "standalone-bundle-dir = standaloneBundles.${system};",
            "standalone-appimage = standaloneAppImages.${system};",
            "standalone-macos-app = standaloneMacApps.${system};",
            "nix-bundle-dir.bundlers.${system}.qtApp standalone",
            "mkStandalonePortablePackage",
            'unwrapped="${binary}/bin/.logos-inspector-standalone-gui-wrapped"',
            'extraDirs = [ "libexec" "share" ];',
            'for framework in "$qtDir"/lib/*.framework; do',
            'module="$(basename "$framework" .framework)"',
            'ln -sfn "$framework/Headers" "$qtBuildRoot/include/$module"',
        ),
        "flake standalone package",
        errors,
    )
    require(
        flake,
        ('artifactsLink="$NIX_BUILD_TOP/cargo-vendor-dir/artifacts"',),
        "flake program artifact linker",
        errors,
    )
    if "/build/cargo-vendor-dir" in flake:
        errors.append("flake program artifact linker assumes `/build` is the build root")

    try:
        ui_metadata = json.loads((ROOT / "metadata.json").read_text(encoding="utf-8"))
        core_metadata = json.loads(
            (ROOT / "core" / "metadata.json").read_text(encoding="utf-8")
        )
    except (OSError, json.JSONDecodeError) as error:
        errors.append(f"failed to read module metadata: {error}")
    else:
        if ui_metadata.get("display_name") != "Logos Inspector":
            errors.append("UI metadata display_name must be `Logos Inspector`")
        if ui_metadata.get("dependencies") != ["logos_inspector"]:
            errors.append("UI dependency name must remain `logos_inspector`")
        expected_core = list(FORK_INPUTS)
        if core_metadata.get("dependencies") != expected_core:
            errors.append(f"core dependency names must be {expected_core}")

    changelog = read(ROOT / "CHANGELOG.md", errors)
    process = read(ROOT / "docs" / "release-process.md", errors)
    require(
        changelog,
        ("source-owned", "AppImage", "Apple silicon"),
        "CHANGELOG.md",
        errors,
    )
    require(
        process,
        (
            "logos_inspector-v<version>",
            "logos_inspector_ui-v<version>",
            "standalone-v<version>",
            "AppImage",
            "Apple silicon",
            "with `/nix/store` hidden",
        ),
        "release process",
        errors,
    )

    run_check(
        [sys.executable, "scripts/standalone_release.py", "validate-source", "--root", "."],
        "standalone source validation",
        errors,
    )
    run_check(
        [sys.executable, "scripts/standalone_release.py", "self-test"],
        "standalone artifact fixture",
        errors,
    )

    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
