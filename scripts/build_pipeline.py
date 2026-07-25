from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
import time
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
    needs_circuits: bool = False

    def display(self) -> str:
        prefix = " ".join(f"{key}={value}" for key, value in sorted(self.env.items()))
        command = " ".join(self.command)
        return f"{prefix} {command}".strip()

    def with_env(self, extra: dict[str, str]) -> BuildStep:
        merged = dict(self.env)
        merged.update(extra)
        return BuildStep(self.name, self.command, merged, self.needs_circuits)


def default_circuits_dir() -> Path:
    cache_home = Path(os.environ.get("XDG_CACHE_HOME") or (Path.home() / ".cache"))
    if os.environ.get("RUNNER_TEMP"):
        return Path(os.environ["RUNNER_TEMP"]) / "logos-blockchain-circuits"
    return cache_home / "logos-blockchain-circuits" / "install"


def default_native_test_dir() -> Path:
    if os.environ.get("RUNNER_TEMP"):
        return Path(os.environ["RUNNER_TEMP"]) / "logos-inspector-core-async-tests"
    return Path(tempfile.gettempdir()) / "logos-inspector-core-async-tests"


def circuit_env(circuits_dir: Path) -> dict[str, str]:
    return {
        "RISC0_SKIP_BUILD": "1",
        "LOGOS_BLOCKCHAIN_CIRCUITS": str(circuits_dir),
    }


def rust_skip_env() -> dict[str, str]:
    return {"RISC0_SKIP_BUILD": "1"}


def profile_steps(profile: str, root: Path = ROOT) -> list[BuildStep]:
    circuits_dir = default_circuits_dir()
    native_test_dir = default_native_test_dir()
    circuits_version = circuits_release()
    rust = rust_skip_env()

    tracked_build_inputs = BuildStep(
        "tracked build inputs",
        (sys.executable, "scripts/check-tracked-build-inputs.py"),
    )
    source_policy_artifact = BuildStep(
        "source policy artifact",
        (sys.executable, "scripts/source_policy_artifact.py", "check"),
        rust,
        needs_circuits=True,
    )
    actionlint_step = BuildStep(
        "actionlint",
        (sys.executable, "scripts/run-actionlint.py"),
    )
    circuit_setup = BuildStep(
        "circuits",
        (
            sys.executable,
            "scripts/setup-circuits.py",
            circuits_version,
            str(circuits_dir),
        ),
    )
    circuit_setup_tests = BuildStep(
        "circuit setup tests",
        (sys.executable, "scripts/test_setup_circuits.py"),
    )
    action_pin_tests = BuildStep(
        "action pin tests",
        (sys.executable, "scripts/test_action_pins.py"),
    )
    clippy = BuildStep(
        "clippy workspace",
        (
            "cargo",
            "clippy",
            "--locked",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ),
        rust,
        needs_circuits=True,
    )
    tests = BuildStep(
        "cargo test workspace",
        ("cargo", "test", "--locked", "--workspace", "--no-fail-fast"),
        {**rust, "QT_QPA_PLATFORM": os.environ.get("QT_QPA_PLATFORM", "offscreen")},
        needs_circuits=True,
    )
    basecamp_isolation = BuildStep(
        "Basecamp wallet runtime isolation",
        (
            "cargo",
            "test",
            "--locked",
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
        rust,
        needs_circuits=True,
    )
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
    web = BuildStep("web UI", ("npm", "--prefix", "ui", "run", "check"))
    qml_static = BuildStep("QML static", ("scripts/qml-static-check.sh",))
    qml_visual = BuildStep("QML visual", ("scripts/zones-visual-smoke.sh",))

    phases: dict[str, list[BuildStep]] = {
        "ci-policy": [
            actionlint_step,
            BuildStep("rustfmt", ("cargo", "fmt", "--all", "--", "--check")),
            tracked_build_inputs,
            BuildStep("package identity", (sys.executable, "scripts/check-package-identity.py")),
            BuildStep("release workflow", (sys.executable, "scripts/check-release-workflow.py")),
            BuildStep("build artifacts", (sys.executable, "scripts/check-build-artifacts.py")),
            circuit_setup_tests,
            action_pin_tests,
        ],
        "ci-generated": [
            circuit_setup,
            source_policy_artifact,
        ],
        "ci-rust": [
            clippy,
            tests,
            basecamp_isolation,
        ],
        "ci-native": native_steps,
        "ci-web": [web],
        "ci-qml-static": [qml_static],
        "ci-qml-visual": [qml_visual],
        "identity": [
            BuildStep("package identity", (sys.executable, "scripts/check-package-identity.py")),
            BuildStep("release workflow", (sys.executable, "scripts/check-release-workflow.py")),
            circuit_setup,
            source_policy_artifact,
            BuildStep("build artifacts", (sys.executable, "scripts/check-build-artifacts.py")),
        ],
        "artifacts": [
            circuit_setup,
            source_policy_artifact,
            BuildStep("build artifacts", (sys.executable, "scripts/check-build-artifacts.py")),
        ],
        "native": native_steps,
        "web": [web],
        "qml": [qml_static, qml_visual],
        "rust": [
            BuildStep("rustfmt", ("cargo", "fmt", "--all", "--", "--check")),
            tracked_build_inputs,
            clippy,
            tests,
            basecamp_isolation,
        ],
    }

    composed = {
        "ci": (
            "ci-policy",
            "ci-generated",
            "ci-rust",
            "ci-native",
            "ci-web",
            "ci-qml-static",
        ),
        "local": (
            "ci-policy",
            "ci-generated",
            "ci-rust",
            "ci-native",
            "ci-web",
            "ci-qml-static",
            "ci-qml-visual",
        ),
        "ci-main": (
            "ci",
            "ci-qml-visual",
        ),
    }

    if profile in composed:
        steps: list[BuildStep] = []
        for part in composed[profile]:
            if part in composed:
                for nested in composed[part]:
                    steps.extend(phases[nested])
            else:
                steps.extend(phases[part])
        return apply_circuits(with_root(root, steps), circuits_dir)

    try:
        steps = phases[profile]
    except KeyError as err:
        choices = ", ".join(sorted({*phases, *composed}))
        raise ValueError(f"unknown build profile `{profile}`; expected one of: {choices}") from err
    return apply_circuits(with_root(root, steps), circuits_dir)


def apply_circuits(steps: list[BuildStep], circuits_dir: Path) -> list[BuildStep]:
    env = circuit_env(circuits_dir)
    result: list[BuildStep] = []
    for step in steps:
        if step.needs_circuits:
            result.append(step.with_env(env))
        else:
            result.append(step)
    return result


def run_profile(profile: str, *, dry_run: bool = False, root: Path = ROOT) -> int:
    steps = profile_steps(profile, root)
    timings: list[tuple[str, float, bool]] = []
    in_github = bool(os.environ.get("GITHUB_ACTIONS"))
    exit_code = 0

    for step in steps:
        print(f"==> {step.name}: {step.display()}")
        if in_github:
            print(f"::group::{step.name}")
        started = time.monotonic()
        passed = True
        if not dry_run:
            env = os.environ.copy()
            env.update(step.env)
            completed = subprocess.run(step.command, cwd=root, env=env, check=False)
            if completed.returncode != 0:
                passed = False
                exit_code = completed.returncode
        elapsed = time.monotonic() - started
        timings.append((step.name, elapsed, passed))
        status = "pass" if passed else "fail"
        print(f"    [{status}] {elapsed:.2f}s")
        if in_github:
            print("::endgroup::")
        if not passed:
            write_timing_summary(timings)
            return exit_code

    write_timing_summary(timings)
    return 0


def write_timing_summary(timings: list[tuple[str, float, bool]]) -> None:
    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if not summary_path:
        return
    lines = [
        "### Build pipeline timing",
        "",
        "| Step | Duration | Status |",
        "| --- | ---: | --- |",
    ]
    for name, elapsed, passed in timings:
        lines.append(f"| {name} | {elapsed:.2f}s | {'pass' if passed else 'fail'} |")
    lines.append("")
    with open(summary_path, "a", encoding="utf-8") as handle:
        handle.write("\n".join(lines))


def list_profiles() -> Iterable[str]:
    return (
        "artifacts",
        "ci",
        "ci-generated",
        "ci-main",
        "ci-native",
        "ci-policy",
        "ci-qml-static",
        "ci-qml-visual",
        "ci-rust",
        "ci-web",
        "identity",
        "local",
        "native",
        "qml",
        "rust",
        "web",
    )


def with_root(root: Path, steps: list[BuildStep]) -> list[BuildStep]:
    if root == ROOT:
        return steps
    return [
        BuildStep(
            step.name,
            tuple(str(root / item) if item.startswith("scripts/") else item for item in step.command),
            step.env,
            step.needs_circuits,
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
