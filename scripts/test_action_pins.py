#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path


def load_module():
    path = Path(__file__).resolve().parent / "check-release-workflow.py"
    spec = importlib.util.spec_from_file_location("check_release_workflow", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


pins = load_module()


class ActionPinTests(unittest.TestCase):
    def test_full_sha_passes(self) -> None:
        errors: list[str] = []
        text = "uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1\n"
        pins.check_pinned_actions(text, "sample", errors)
        self.assertEqual(errors, [])

    def test_tag_branch_and_unpinned_fail(self) -> None:
        cases = [
            "uses: actions/checkout@v4\n",
            "uses: actions/checkout@main\n",
            "uses: actions/checkout@v4.2.2\n",
            "uses: actions/checkout\n",
        ]
        for text in cases:
            errors: list[str] = []
            pins.check_pinned_actions(text, "sample", errors)
            self.assertTrue(errors, msg=text)

    def test_local_action_allowed(self) -> None:
        errors: list[str] = []
        pins.check_pinned_actions("uses: ./.github/actions/foo\n", "sample", errors)
        pins.check_pinned_actions(
            "uses: ./.github/workflows/release-core.yml\n", "sample", errors
        )
        self.assertEqual(errors, [])

    def test_docker_requires_digest(self) -> None:
        errors: list[str] = []
        pins.check_pinned_actions("uses: docker://alpine:3.20\n", "sample", errors)
        self.assertTrue(errors)
        errors = []
        pins.check_pinned_actions(
            "uses: docker://alpine@sha256:" + ("a" * 64) + "\n",
            "sample",
            errors,
        )
        self.assertEqual(errors, [])

    def test_scanner_covers_yaml_and_composite(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            workflows = root / ".github" / "workflows"
            actions = root / ".github" / "actions" / "example"
            workflows.mkdir(parents=True)
            actions.mkdir(parents=True)
            (workflows / "ci.yaml").write_text(
                "jobs:\n  x:\n    steps:\n      - uses: actions/checkout@v4\n",
                encoding="utf-8",
            )
            (actions / "action.yml").write_text(
                "runs:\n  using: composite\n  steps:\n    - uses: actions/cache@main\n",
                encoding="utf-8",
            )
            files = pins.iter_action_definition_files(root)
            self.assertEqual(
                {path.name for path in files},
                {"ci.yaml", "action.yml"},
            )
            errors: list[str] = []
            for path in files:
                pins.check_pinned_actions(
                    path.read_text(encoding="utf-8"),
                    path.name,
                    errors,
                )
            self.assertEqual(len(errors), 2)


if __name__ == "__main__":
    unittest.main()
