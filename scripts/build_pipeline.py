from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterable

from build_artifacts import circuits_release


ROOT = Path(__file__).resolve().parents[1]


@dataclass(frozen=True)
class BuildStep:
    name: str
    command: tuple[str, ...]
    env: dict[str, str] = field(default_factory=dict)

    def display(self) -> str:
        prefix = " ".join(f"{key}={value}" for key, value in sorted(self.env.items()))
        command = " ".join(self.command)
        return f"{prefix} {command}".strip()


def profile_steps(profile: str, root: Path = ROOT) -> list[BuildStep]:
    circuits_dir = Path(os.environ.get("RUNNER_TEMP") or tempfile.gettempdir()) / "logos-blockchain-circuits"
    native_test_dir = Path(os.environ.get("RUNNER_TEMP") or tempfile.gettempdir()) / "logos-inspector-core-async-tests"
    circuits_version = circuits_release()
    rust_env = {"RISC0_SKIP_BUILD": "1"}
    cargo_workspace = ("cargo", "check", "--workspace")
    clippy_workspace = ("cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings")
    test_workspace = ("cargo", "test", "--workspace")
    tracked_build_inputs = BuildStep(
        "tracked build inputs",
        (sys.executable, "scripts/check-tracked-build-inputs.py"),
    )
    source_policy_artifact = (sys.executable, "scripts/source_policy_artifact.py", "check")
    native_steps = [
        BuildStep(
            "configure native async bridge tests",
            (
                "cmake",
                "-S",
                "core/tests",
                "-B",
                str(native_test_dir),
                "-DCMAKE_BUILD_TYPE=Debug",
            ),
        ),
        BuildStep(
            "build native async bridge tests",
            ("cmake", "--build", str(native_test_dir), "--parallel"),
        ),
        BuildStep(
            "run native async bridge tests",
            ("ctest", "--test-dir", str(native_test_dir), "--output-on-failure"),
        ),
    ]

    profiles: dict[str, list[BuildStep]] = {
        "ci": [
            BuildStep("rustfmt", ("cargo", "fmt", "--all", "--", "--check")),
            tracked_build_inputs,
            BuildStep("package identity", (sys.executable, "scripts/check-package-identity.py")),
            BuildStep("release workflow", (sys.executable, "scripts/check-release-workflow.py")),
            BuildStep("source policy artifact", source_policy_artifact, rust_env),
            BuildStep("build artifacts", (sys.executable, "scripts/check-build-artifacts.py")),
            BuildStep(
                "circuits",
                (
                    sys.executable,
                    "scripts/setup-circuits.py",
                    circuits_version,
                    str(circuits_dir),
                ),
            ),
            BuildStep("cargo check", ("cargo", "check"), rust_env),
            BuildStep(
                "Basecamp wallet runtime isolation",
                (
                    "cargo",
                    "test",
                    "-p",
                    "logos-inspector",
                    "--no-default-features",
                    "--features",
                    "basecamp-wallet-provider",
                    "--lib",
                    "wallet::instruction::basecamp_build_tests::instruction_submission_does_not_fall_back_to_a_local_wallet_runtime",
                    "--",
                    "--exact",
                ),
                rust_env,
            ),
            BuildStep("clippy", ("cargo", "clippy", "--all-targets", "--", "-D", "warnings"), rust_env),
            *native_steps,
        ],
        "local": [
            BuildStep("rustfmt", ("cargo", "fmt", "--all", "--", "--check")),
            tracked_build_inputs,
            BuildStep("package identity", (sys.executable, "scripts/check-package-identity.py")),
            BuildStep("release workflow", (sys.executable, "scripts/check-release-workflow.py")),
            BuildStep("source policy artifact", source_policy_artifact, rust_env),
            BuildStep("build artifacts", (sys.executable, "scripts/check-build-artifacts.py")),
            BuildStep("cargo check workspace", cargo_workspace, rust_env),
            BuildStep("clippy workspace", clippy_workspace, rust_env),
            BuildStep("cargo test workspace", test_workspace, rust_env),
            *native_steps,
            BuildStep("web UI", ("npm", "--prefix", "ui", "run", "check")),
            BuildStep("QML smoke", ("scripts/gui-visual-action-smoke.sh",)),
        ],
        "rust": [
            BuildStep("rustfmt", ("cargo", "fmt", "--all", "--", "--check")),
            tracked_build_inputs,
            BuildStep("cargo check workspace", cargo_workspace, rust_env),
            BuildStep("clippy workspace", clippy_workspace, rust_env),
            BuildStep("cargo test workspace", test_workspace, rust_env),
        ],
        "qml": [
            BuildStep("QML smoke", ("scripts/gui-visual-action-smoke.sh",)),
        ],
        "native": native_steps,
        "web": [
            BuildStep("web UI", ("npm", "--prefix", "ui", "run", "check")),
        ],
        "identity": [
            BuildStep("package identity", (sys.executable, "scripts/check-package-identity.py")),
            BuildStep("release workflow", (sys.executable, "scripts/check-release-workflow.py")),
            BuildStep("source policy artifact", source_policy_artifact, rust_env),
            BuildStep("build artifacts", (sys.executable, "scripts/check-build-artifacts.py")),
        ],
        "artifacts": [
            BuildStep("source policy artifact", source_policy_artifact, rust_env),
            BuildStep("build artifacts", (sys.executable, "scripts/check-build-artifacts.py")),
        ],
    }

    try:
        steps = profiles[profile]
    except KeyError as err:
        choices = ", ".join(sorted(profiles))
        raise ValueError(f"unknown build profile `{profile}`; expected one of: {choices}") from err

    if profile == "ci":
        steps = with_ci_circuit_env(steps, circuits_dir)
    return with_root(root, steps)


def run_profile(profile: str, *, dry_run: bool = False, root: Path = ROOT) -> int:
    steps = profile_steps(profile, root)
    for step in steps:
        print(f"==> {step.name}: {step.display()}")
        if dry_run:
            continue
        env = os.environ.copy()
        env.update(step.env)
        completed = subprocess.run(step.command, cwd=root, env=env, check=False)
        if completed.returncode != 0:
            return completed.returncode
    return 0


def list_profiles() -> Iterable[str]:
    return ("artifacts", "ci", "identity", "local", "native", "qml", "rust", "web")


def with_ci_circuit_env(steps: list[BuildStep], circuits_dir: Path) -> list[BuildStep]:
    result: list[BuildStep] = []
    for step in steps:
        if step.name in {"cargo check", "clippy"}:
            env = dict(step.env)
            env["LOGOS_BLOCKCHAIN_CIRCUITS"] = str(circuits_dir)
            result.append(BuildStep(step.name, step.command, env))
        else:
            result.append(step)
    return result


def with_root(root: Path, steps: list[BuildStep]) -> list[BuildStep]:
    if root == ROOT:
        return steps
    return [
        BuildStep(
            step.name,
            tuple(str(root / item) if item.startswith("scripts/") else item for item in step.command),
            step.env,
        )
        for step in steps
    ]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run Logos Inspector build verification")
    parser.add_argument("profile", nargs="?", default="local", choices=tuple(list_profiles()))
    parser.add_argument("--dry-run", action="store_true", help="print commands without running them")
    parser.add_argument("--list", action="store_true", help="list available profiles")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.list:
        for profile in list_profiles():
            print(profile)
        return 0
    return run_profile(args.profile, dry_run=args.dry_run)


if __name__ == "__main__":
    raise SystemExit(main())
