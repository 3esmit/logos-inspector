#!/usr/bin/env python3
from __future__ import annotations

import base64
import hashlib
import importlib.util
import json
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest import mock


def load_module():
    path = Path(__file__).resolve().parent / "setup-circuits.py"
    spec = importlib.util.spec_from_file_location("setup_circuits", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    # setup-circuits imports build_artifacts from the same directory.
    import sys

    sys.path.insert(0, str(path.parent))
    spec.loader.exec_module(module)
    return module


setup_circuits = load_module()

import build_artifacts


class DecodeSriTests(unittest.TestCase):
    def test_valid_sri(self) -> None:
        digest = hashlib.sha256(b"payload").digest()
        sri = "sha256-" + base64.b64encode(digest).decode("ascii")
        self.assertEqual(setup_circuits.decode_sri_sha256(sri), digest)

    def test_malformed_sri(self) -> None:
        with self.assertRaises(ValueError):
            setup_circuits.decode_sri_sha256("not-a-hash")
        with self.assertRaises(ValueError):
            setup_circuits.decode_sri_sha256("sha512-abc")
        with self.assertRaises(ValueError):
            setup_circuits.decode_sri_sha256("sha256-")
        with self.assertRaises(ValueError):
            setup_circuits.decode_sri_sha256("sha256-@@@")


class SetupCircuitsTests(unittest.TestCase):
    def _archive_bytes(self, payload: bytes = b"circuit-bytes") -> tuple[bytes, str]:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            package = root / "circuits-root"
            package.mkdir()
            (package / "proof.bin").write_bytes(payload)
            archive_path = root / "circuits.tar.gz"
            with tarfile.open(archive_path, "w:gz") as tar:
                tar.add(package, arcname="circuits-root")
            data = archive_path.read_bytes()
        digest = hashlib.sha256(data).digest()
        sri = "sha256-" + base64.b64encode(digest).decode("ascii")
        return data, sri

    def test_hash_match_and_marker(self) -> None:
        data, sri = self._archive_bytes()
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            install_dir = base / "install"
            cache_dir = base / "cache"
            cache_dir.mkdir()
            (cache_dir / "circuits.tar.gz").write_bytes(data)

            setup_circuits.setup_circuits(
                release="v0.5.3",
                target={"os": "linux", "arch": "x86_64"},
                artifact="circuits.tar.gz",
                expected_archive_hash=sri,
                url="https://example.test/circuits.tar.gz",
                install_dir=install_dir,
                cache_dir=cache_dir,
            )
            self.assertTrue((install_dir / "proof.bin").is_file())
            marker = json.loads((install_dir / setup_circuits.MARKER_NAME).read_text(encoding="utf-8"))
            self.assertEqual(marker["release"], "v0.5.3")
            self.assertEqual(marker["archiveHash"], sri)
            self.assertEqual(marker["artifact"], "circuits.tar.gz")

    def test_hash_mismatch(self) -> None:
        data, _ = self._archive_bytes()
        wrong = "sha256-" + base64.b64encode(hashlib.sha256(b"other").digest()).decode("ascii")
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            cache_dir = base / "cache"
            cache_dir.mkdir()
            (cache_dir / "circuits.tar.gz").write_bytes(data)
            with self.assertRaises(RuntimeError):
                setup_circuits.setup_circuits(
                    release="v0.5.3",
                    target={"os": "linux", "arch": "x86_64"},
                    artifact="circuits.tar.gz",
                    expected_archive_hash=wrong,
                    url="https://example.test/circuits.tar.gz",
                    install_dir=base / "install",
                    cache_dir=cache_dir,
                )

    def test_idempotent_when_marker_matches(self) -> None:
        data, sri = self._archive_bytes()
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            install_dir = base / "install"
            cache_dir = base / "cache"
            cache_dir.mkdir()
            (cache_dir / "circuits.tar.gz").write_bytes(data)
            kwargs = dict(
                release="v0.5.3",
                target={"os": "linux", "arch": "x86_64"},
                artifact="circuits.tar.gz",
                expected_archive_hash=sri,
                url="https://example.test/circuits.tar.gz",
                install_dir=install_dir,
                cache_dir=cache_dir,
            )
            setup_circuits.setup_circuits(**kwargs)
            first_mtime = (install_dir / "proof.bin").stat().st_mtime_ns
            with mock.patch.object(setup_circuits, "download_archive") as download:
                setup_circuits.setup_circuits(**kwargs)
                download.assert_not_called()
            self.assertEqual((install_dir / "proof.bin").stat().st_mtime_ns, first_mtime)

    def test_stale_marker_invalidates(self) -> None:
        data, sri = self._archive_bytes(b"fresh")
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            install_dir = base / "install"
            install_dir.mkdir()
            (install_dir / "proof.bin").write_bytes(b"stale")
            (install_dir / setup_circuits.MARKER_NAME).write_text(
                json.dumps(
                    {
                        "release": "v0.0.1",
                        "platform": {"os": "linux", "arch": "x86_64"},
                        "artifact": "old.tar.gz",
                        "archiveHash": "sha256-" + base64.b64encode(b"0" * 32).decode("ascii"),
                    }
                ),
                encoding="utf-8",
            )
            cache_dir = base / "cache"
            cache_dir.mkdir()
            (cache_dir / "circuits.tar.gz").write_bytes(data)
            setup_circuits.setup_circuits(
                release="v0.5.3",
                target={"os": "linux", "arch": "x86_64"},
                artifact="circuits.tar.gz",
                expected_archive_hash=sri,
                url="https://example.test/circuits.tar.gz",
                install_dir=install_dir,
                cache_dir=cache_dir,
            )
            self.assertEqual((install_dir / "proof.bin").read_bytes(), b"fresh")

    def test_main_uses_archive_hash(self) -> None:
        target = {
            "os": "linux",
            "arch": "x86_64",
            "archiveHash": "sha256-archive",
            "sourceHash": "sha256-source",
        }
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            with (
                mock.patch.object(setup_circuits, "load_catalog", return_value={}),
                mock.patch.object(setup_circuits, "circuits_release", return_value="v0.5.3"),
                mock.patch.object(setup_circuits, "current_target", return_value=target),
                mock.patch.object(setup_circuits, "circuit_artifact_name", return_value="circuits.tar.gz"),
                mock.patch.object(setup_circuits, "circuit_artifact_url", return_value="https://example.test/circuits.tar.gz"),
                mock.patch.object(setup_circuits, "setup_circuits") as install,
            ):
                self.assertEqual(
                    setup_circuits.main(
                        [
                            "--install-dir",
                            str(root / "install"),
                            "--cache-dir",
                            str(root / "cache"),
                        ]
                    ),
                    0,
                )

        self.assertEqual(install.call_args.kwargs["expected_archive_hash"], target["archiveHash"])


class BuildArtifactCatalogTests(unittest.TestCase):
    def catalog(self) -> dict:
        return {
            "circuits": {
                "repo": "example/circuits",
                "release": "v0.5.3",
                "targets": {
                    "x86_64-linux": {
                        "os": "linux",
                        "arch": "x86_64",
                        "archiveHash": "sha256-archive",
                        "sourceHash": "sha256-source",
                    }
                },
            },
            "rapidsnark": {
                "version": "0.0.8",
                "cargoRev": "revision",
                "targets": {"x86_64-linux": {"url": "https://example.test/rapidsnark.zip", "hash": "sha256-hash"}},
            },
            "lez": {
                "repo": "example/lez",
                "cargoRev": "revision",
                "revision": "revision",
                "sourceHash": "sha256-source",
            },
        }

    def test_target_exposes_archive_and_source_hashes(self) -> None:
        target = build_artifacts.circuit_target_by_platform(self.catalog(), "linux", "x86_64")
        self.assertEqual(target["archiveHash"], "sha256-archive")
        self.assertEqual(target["sourceHash"], "sha256-source")

    def test_target_requires_both_hash_domains(self) -> None:
        for key in ("archiveHash", "sourceHash"):
            with self.subTest(key=key):
                catalog = self.catalog()
                del catalog["circuits"]["targets"]["x86_64-linux"][key]
                self.assertIn(
                    f"circuits.targets.x86_64-linux.{key} is required",
                    build_artifacts.catalog_shape_errors(catalog),
                )

    def test_nix_circuit_fetch_requires_source_hash(self) -> None:
        source_hash = """
            mkCircuitsArtifact = pkgs:
              pkgs.fetchzip {
                hash = target.sourceHash;
              };
            mkCircuitBuildContext = pkgs: {};
        """
        archive_hash = source_hash.replace("target.sourceHash", "target.archiveHash")

        self.assertEqual(build_artifacts.circuit_nix_hash_errors(source_hash), [])
        self.assertEqual(
            build_artifacts.circuit_nix_hash_errors(archive_hash),
            ["flake.nix circuit fetchzip must use target.sourceHash"],
        )


if __name__ == "__main__":
    unittest.main()
