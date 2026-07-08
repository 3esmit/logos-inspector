#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
CATALOG_PATH = Path("qml/state/source_routing/SourcePolicyCatalog.generated.js")
TOP_LEVEL_ORDER = ("version", "defaults", "network_profiles", "source_modes")
DEFAULTS_ORDER = (
    "sequencer_endpoint",
    "local_sequencer_endpoint",
    "indexer_endpoint",
    "node_endpoint",
    "delivery_rest_endpoint",
    "delivery_metrics_endpoint",
    "storage_rest_endpoint",
    "storage_metrics_endpoint",
)
NETWORK_PROFILE_ORDER = (
    "id",
    "label",
    "sequencer_endpoint",
    "indexer_endpoint",
    "node_endpoint",
)
SOURCE_MODE_FAMILY_ORDER = ("core", "delivery", "storage")
SOURCE_MODE_ORDER = (
    "key",
    "aliases",
    "effective",
    "label_key",
    "label",
    "source_label",
    "summary",
    "implemented",
    "adapter",
)
SOURCE_ADAPTER_ORDER = (
    "target",
    "uses_rest_endpoint",
    "uses_metrics_endpoint",
    "supports_cid_probe",
    "supports_mutating_diagnostics",
)


def source_policy(root: Path) -> dict[str, Any]:
    env = os.environ.copy()
    env.setdefault("RISC0_SKIP_BUILD", "1")
    completed = subprocess.run(
        ("cargo", "run", "--quiet", "--", "cli", "source-policy"),
        cwd=root,
        env=env,
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    )
    value = json.loads(completed.stdout)
    if not isinstance(value, dict):
        raise TypeError("source-policy output must be a JSON object")
    return value


def render_catalog(policy: dict[str, Any]) -> str:
    policy_json = json.dumps(ordered_source_policy(policy), separators=(",", ":"))
    policy_literal = json.dumps(policy_json, separators=(",", ":"))
    return (
        f"const SOURCE_POLICY_JSON = {policy_literal}\n\n"
        "function sourcePolicy() {\n"
        "    return JSON.parse(SOURCE_POLICY_JSON)\n"
        "}\n"
    )


def ordered_source_policy(policy: dict[str, Any]) -> dict[str, Any]:
    return ordered_mapping(policy, TOP_LEVEL_ORDER, ())


def ordered_value(value: Any, path: tuple[str, ...]) -> Any:
    if isinstance(value, list):
        return [ordered_value(item, path + ("*",)) for item in value]
    if not isinstance(value, dict):
        return value
    return ordered_mapping(value, key_order_for_path(path), path)


def ordered_mapping(
    value: dict[str, Any],
    order: tuple[str, ...],
    path: tuple[str, ...],
) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key in order:
        if key in value:
            result[key] = ordered_value(value[key], path + (key,))
    for key, item in value.items():
        if key not in result:
            result[key] = ordered_value(item, path + (key,))
    return result


def key_order_for_path(path: tuple[str, ...]) -> tuple[str, ...]:
    if path == ("defaults",):
        return DEFAULTS_ORDER
    if path == ("network_profiles", "*"):
        return NETWORK_PROFILE_ORDER
    if path == ("source_modes",):
        return SOURCE_MODE_FAMILY_ORDER
    if path == ("source_modes", "core", "*"):
        return SOURCE_MODE_ORDER
    if path == ("source_modes", "delivery", "*"):
        return SOURCE_MODE_ORDER
    if path == ("source_modes", "storage", "*"):
        return SOURCE_MODE_ORDER
    if path in {
        ("source_modes", "core", "*", "adapter"),
        ("source_modes", "delivery", "*", "adapter"),
        ("source_modes", "storage", "*", "adapter"),
    }:
        return SOURCE_ADAPTER_ORDER
    return ()


def write_catalog(root: Path) -> int:
    target = root / CATALOG_PATH
    target.write_text(render_catalog(source_policy(root)), encoding="utf-8")
    return 0


def check_catalog(root: Path) -> int:
    target = root / CATALOG_PATH
    expected = render_catalog(source_policy(root))
    actual = target.read_text(encoding="utf-8")
    if actual == expected:
        return 0
    print(
        f"error: {CATALOG_PATH} does not match `cargo run -- cli source-policy`",
        file=sys.stderr,
    )
    print("hint: run `python3 scripts/source_policy_artifact.py write`", file=sys.stderr)
    return 1


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate or check QML source policy artifact")
    parser.add_argument("mode", choices=("check", "write"))
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.mode == "write":
        return write_catalog(ROOT)
    return check_catalog(ROOT)


if __name__ == "__main__":
    raise SystemExit(main())
