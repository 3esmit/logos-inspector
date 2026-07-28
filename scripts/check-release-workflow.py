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
    "complete": ROOT / ".github" / "workflows" / "release.yml",
    "core": ROOT / ".github" / "workflows" / "release-core.yml",
    "ui": ROOT / ".github" / "workflows" / "release-ui.yml",
    "standalone": ROOT / ".github" / "workflows" / "release-standalone.yml",
    "cli": ROOT / ".github" / "workflows" / "release-cli.yml",
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


# Match both mapping-form and list-item-form `uses:` keys.
USES_LINE = re.compile(r"^\s*-?\s*uses:\s*(.+?)\s*(?:#.*)?$")


def iter_action_definition_files(root: Path = ROOT) -> list[Path]:
    paths: list[Path] = []
    workflows = root / ".github" / "workflows"
    actions = root / ".github" / "actions"
    if workflows.is_dir():
        paths.extend(sorted(workflows.glob("*.yml")))
        paths.extend(sorted(workflows.glob("*.yaml")))
    if actions.is_dir():
        paths.extend(sorted(actions.glob("**/action.yml")))
        paths.extend(sorted(actions.glob("**/action.yaml")))
    return paths


def check_pinned_actions(text: str, label: str, errors: list[str]) -> None:
    for line in text.splitlines():
        match = USES_LINE.match(line)
        if match is None:
            continue
        reference = match.group(1).strip().strip("\"'")
        if not reference:
            continue
        if reference.startswith("./") or reference.startswith(".\\"):
            # Local reusable workflows and composite actions are repository-relative.
            continue
        if reference.startswith("docker://"):
            if "@sha256:" not in reference:
                errors.append(
                    f"{label} uses mutable Docker action reference `{reference}`"
                )
            continue
        if "@" not in reference:
            errors.append(f"{label} uses unpinned action reference `{reference}`")
            continue
        pin = reference.rsplit("@", 1)[1]
        if not ACTION_SHA.fullmatch(pin):
            errors.append(f"{label} uses mutable action reference `{pin}`")


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
    complete = texts["complete"]
    core = texts["core"]
    ui = texts["ui"]
    standalone = texts["standalone"]
    cli = texts["cli"]
    release_action = (
        "3esmit/logos-modules-release-action/.github/workflows/release.yml"
        f"@{RELEASE_ACTION_SHA}"
    )

    common_lgx = (
        "workflow_dispatch:",
        "workflow_call:",
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
            "workflow_call:",
            ".#standalone-appimage",
            ".#standalone-macos-app",
            "Select and verify macOS Metal Toolchain",
            "xcode_developer=/Applications/Xcode_26.3.app/Contents/Developer",
            'test -x "$xcode_developer/usr/bin/xcodebuild"',
            'sudo xcode-select --switch "$xcode_developer"',
            'test "$(xcode-select --print-path)" = "$xcode_developer"',
            'toolchain_help="$(env -u DEVELOPER_DIR -u SDKROOT xcodebuild -help 2>&1)"',
            'case "$toolchain_help" in',
            "*-downloadComponent*) ;;",
            "xcodebuild -downloadComponent MetalToolchain",
            "xcrun --sdk macosx --find metal",
            "xcrun --sdk macosx --find metallib",
            "unshare --mount",
            "mount -t tmpfs tmpfs /nix/store",
            "Install Linux host graphics runtime for smoke",
            (
                "apt-get install --yes --no-install-recommends "
                "libegl1 libglx0 libopengl0 libvulkan1"
            ),
            "for library in libEGL.so.1 libGLX.so.0 libOpenGL.so.0 "
            "libGLdispatch.so.0 libvulkan.so.1",
            'ldd "$native"',
            "standalone-linux-dynamic-dependencies.txt",
            "grep -F 'not found'",
            "Ubuntu packages: \\`libvulkan1 libegl1 libglx0 libopengl0\\`",
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
    require(
        cli,
        (
            "workflow_dispatch:",
            "workflow_call:",
            "cli-v$VERSION",
            "DeterminateSystems/nix-installer-action@ef8a148080ab6020fd15196c2084a2eea5ff2d25",
            ".#cli-bundle",
            "--bundle-root result-cli",
            "test \"$(uname -m)\" = arm64",
            "scripts/cli_release.py package",
            "scripts/cli_release.py smoke-archive",
            "scripts/cli_release.py verify",
            "Smoke extracted Linux CLI bundle without the Nix store",
            "unshare --mount",
            "mount -t tmpfs tmpfs /nix/store",
            "--write-checksums",
            "draft: true",
            "prerelease: true",
            "gh release download",
            "gh release edit",
            "target_commitish: ${{ github.sha }}",
        ),
        "CLI release workflow",
        errors,
    )
    for forbidden in (
        "Install Qt",
        "qt6",
        "qml",
        "standalone",
        "musl",
    ):
        if forbidden.lower() in cli.lower():
            errors.append(
                f"CLI release workflow must not install or package desktop dependency `{forbidden}`"
            )
    require(
        complete,
        (
            "workflow_dispatch:",
            "uses: ./.github/workflows/release-core.yml",
            "uses: ./.github/workflows/release-ui.yml",
            "uses: ./.github/workflows/release-standalone.yml",
            "confirm: true",
            "logos_inspector-v",
            "logos_inspector_ui-v",
            "standalone-v",
            "standalone_release.py verify",
            'test -f "published/core/logos_inspector-$VERSION.lgx"',
            'test -f "published/ui/logos_inspector_ui-$VERSION.lgx"',
        ),
        "complete release workflow",
        errors,
    )
    if "release-cli.yml" in complete:
        errors.append("complete release workflow must not include the independent CLI stream")
    linux_build = standalone.find("- name: Build AppImage")
    linux_graphics = standalone.find(
        "- name: Install Linux host graphics runtime for smoke"
    )
    linux_smoke = standalone.find(
        "- name: Smoke extracted AppImage without the Nix store"
    )
    if not 0 <= linux_build < linux_graphics < linux_smoke:
        errors.append(
            "standalone release workflow must install the host graphics runtime "
            "after building and before smoking the Linux AppImage"
        )
    linux_extract = standalone.find("--appimage-extract", linux_smoke)
    linux_ldd = standalone.find('ldd "$native"', linux_smoke)
    linux_verify = standalone.find(
        "standalone_release.py verify-tree",
        linux_smoke,
    )
    linux_unshare = standalone.find("unshare --mount", linux_smoke)
    if not 0 <= linux_extract < linux_ldd < linux_verify < linux_unshare:
        errors.append(
            "standalone release workflow must extract, audit dynamic "
            "dependencies, verify the tree, then smoke with the Nix store hidden"
        )
    macos_metal = standalone.find("- name: Select and verify macOS Metal Toolchain")
    macos_help = standalone.find("toolchain_help=", macos_metal)
    macos_download = standalone.find(
        "xcodebuild -downloadComponent MetalToolchain", macos_metal
    )
    macos_metal_binary = standalone.find("xcrun --sdk macosx --find metal", macos_metal)
    macos_metallib_binary = standalone.find(
        "xcrun --sdk macosx --find metallib", macos_metal
    )
    macos_build = standalone.find("- name: Build and archive app")
    if not (
        0
        <= macos_metal
        < macos_help
        < macos_download
        < macos_metal_binary
        < macos_metallib_binary
        < macos_build
    ):
        errors.append(
            "standalone release workflow must select a Metal-capable Xcode, "
            "verify component-download support, install the Metal Toolchain, "
            "and verify metal and metallib before building"
        )
    if "xcodebuild -help " + chr(92) in standalone:
        errors.append(
            "standalone release workflow must not pipe xcodebuild help into grep under pipefail"
        )
    if 'toolchain_help="$(env -u DEVELOPER_DIR -u SDKROOT xcodebuild -help)"' in standalone:
        errors.append(
            "standalone release workflow must capture xcodebuild help from standard error"
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
    for path in iter_action_definition_files(ROOT):
        relative = path.relative_to(ROOT)
        check_pinned_actions(
            read(path, errors),
            str(relative),
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
        (
            "mkCliBinary",
            '"--no-default-features"',
            '"cli,local-wallet-runtime"',
            "mkCliPackage",
            "unset PYTHONPATH PYTHONSTARTUP PYTHONUSERBASE",
            'export PYTHONHOME="$root/python"',
            "export PYTHONNOUSERSITE=1",
            "extraDirs = [ \"python\" ];",
            "nix-bundle-dir.lib.${system}.mkBundle",
            "cli-bundle = cliBundles.${system};",
        ),
        "flake CLI package",
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
    metal_wrapper = flake.find('metalXcrun = pkgs.writeShellScriptBin "xcrun"')
    metal_cache_bypass = flake.find("export xcrun_nocache=1", metal_wrapper)
    metal_exec = flake.find("exec /usr/bin/xcrun", metal_wrapper)
    if not 0 <= metal_wrapper < metal_cache_bypass < metal_exec:
        errors.append(
            "Darwin Metal xcrun wrapper must bypass the stale Xcode tool cache "
            "before invoking the system xcrun"
        )

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
        ("source-owned", "AppImage", "Apple silicon", "CLI"),
        "CHANGELOG.md",
        errors,
    )
    require(
        process,
        (
            "logos_inspector-v<version>",
            "logos_inspector_ui-v<version>",
            "standalone-v<version>",
            "cli-v<version>",
            "release.yml",
            "release-cli.yml",
            "gh workflow run release.yml -f confirm=true --ref main",
            "gh workflow run release-cli.yml -f confirm=true --ref main",
            "AppImage",
            "Apple silicon",
            "x86_64-unknown-linux-gnu",
            "`bin/logos-inspector`",
            "embedded Python runtime",
            "BUILD-INFO.json",
            "`/nix/store` hidden",
            "`libEGL.so.1`",
            "`libGLX.so.0`",
            "`libOpenGL.so.0`",
            "`libGLdispatch.so.0`",
            "`libvulkan.so.1`",
            "`ldd`",
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
    run_check(
        [sys.executable, "scripts/cli_release.py", "validate-source", "--root", "."],
        "CLI source validation",
        errors,
    )
    run_check(
        [sys.executable, "scripts/cli_release.py", "self-test"],
        "CLI artifact fixture",
        errors,
    )

    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
