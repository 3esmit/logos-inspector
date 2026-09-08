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


class FlakeInputTests(unittest.TestCase):
    pin = "a" * 40
    url = f"github:owner/repo?rev={pin}"

    def flake(self, inputs: str) -> str:
        return '{ description = "fixture"; inputs = { ' + inputs + ' }; outputs = _: {}; }'

    def test_own_literal_pin_with_comments_and_follows(self) -> None:
        text = self.flake(
            'wanted = { # comment\n inputs.builder.follows = "builder"; '
            f'/* URL follows */ url = "{self.url}"; }};'
        )
        self.assertEqual(pins.flake_input(text, "wanted"), ("owner/repo", self.pin))

    def test_missing_or_unsupported_url_cannot_borrow_next_input(self) -> None:
        for body in [
            '',
            'inputs.builder.follows = "builder";',
            'url = "https://example.invalid/mutable.tar.gz";',
            f'url = "github:owner/repo/{self.pin}";',
            'url = "github:owner/repo?rev=main";',
        ]:
            with self.subTest(body=body):
                text = self.flake(f'wanted = {{ {body} }}; next = {{ url = "{self.url}"; }};')
                self.assertIsNone(pins.flake_input(text, "wanted"))

    def test_exact_input_name_and_scope(self) -> None:
        for declarations in [
            f'unwanted = {{ url = "{self.url}"; }};',
            f'other.wanted = {{ url = "{self.url}"; }};',
            f'other = {{ wanted = {{ url = "{self.url}"; }}; }};',
            f'# wanted = {{ url = "{self.url}"; }};\n',
            f'/* wanted = {{ url = "{self.url}"; }}; */',
        ]:
            with self.subTest(declarations=declarations):
                self.assertIsNone(pins.flake_input(self.flake(declarations), "wanted"))

    def test_nested_url_is_not_the_inputs_url(self) -> None:
        for body in [
            f'inputs.child = {{ url = "{self.url}"; }};',
            f'inputs.child.url = "{self.url}";',
        ]:
            with self.subTest(body=body):
                self.assertIsNone(pins.flake_input(self.flake(f'wanted = {{ {body} }};'), "wanted"))

    def test_direct_url_wins_over_nested_url(self) -> None:
        text = self.flake(
            'wanted = { inputs.child = { url = "github:wrong/repo?rev=' + ('b' * 40) + '"; }; '
            f'url = "{self.url}"; }};'
        )
        self.assertEqual(pins.flake_input(text, "wanted"), ("owner/repo", self.pin))

    def test_dotted_literal_url(self) -> None:
        self.assertEqual(
            pins.flake_input(self.flake(f'wanted.url = "{self.url}";'), "wanted"),
            ("owner/repo", self.pin),
        )

    def test_quoted_attribute_names_and_string_punctuation(self) -> None:
        text = self.flake(
            'note = "braces { }; and # are not syntax; \\"quoted\\""; '
            f'"wanted" = {{ "url" = "{self.url}"; flake = false; }};'
        )
        self.assertEqual(pins.flake_input(text, "wanted"), ("owner/repo", self.pin))

    def test_truncated_input_sets_fail_closed(self) -> None:
        for text in [
            '{ inputs = { wanted = { url = "' + self.url + '"; };',
            '{ inputs = { wanted = { url = "' + self.url,
            '{ inputs = { wanted = { /* unfinished',
            '{ inputs = { wanted = { url = "' + self.url + '";',
        ]:
            with self.subTest(text=text):
                self.assertIsNone(pins.flake_input(text, "wanted"))

    def test_inputs_expression_cannot_override_a_literal_set(self) -> None:
        text = '{ inputs = { wanted.url = "' + self.url + '"; } // { wanted.url = "wrong"; }; }'
        self.assertIsNone(pins.flake_input(text, "wanted"))

    def test_computed_or_ambiguous_inputs_fail_closed(self) -> None:
        for declarations in [
            f'wanted = {{ url = "{self.url}" + "suffix"; }};',
            f'wanted = {{ url = "{self.url}"; }} // {{ url = "wrong"; }};',
            f'wanted = {{ url = "{self.url}"; url = "wrong"; }};',
            f'wanted = {{ url = "{self.url}"; }}; wanted.url = "wrong";',
            'wanted = { url = "github:${owner}/repo?rev=' + self.pin + '"; };',
        ]:
            with self.subTest(declarations=declarations):
                self.assertIsNone(pins.flake_input(self.flake(declarations), "wanted"))

    def test_does_not_read_input_outside_inputs(self) -> None:
        text = '{ unrelated = { wanted = { url = "' + self.url + '"; }; }; inputs = {}; }'
        self.assertIsNone(pins.flake_input(text, "wanted"))

    def test_all_current_policy_inputs_are_detected(self) -> None:
        text = (Path(__file__).resolve().parents[1] / "flake.nix").read_text(encoding="utf-8")
        for name, expected in {**pins.FORK_INPUTS, **pins.BUNDLER_INPUTS}.items():
            with self.subTest(name=name):
                self.assertEqual(pins.flake_input(text, name), expected)


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
