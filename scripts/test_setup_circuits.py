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
                target={"os": "linux", "arch": "x86_64", "hash": sri},
                artifact="circuits.tar.gz",
                expected_hash=sri,
                url="https://example.test/circuits.tar.gz",
                install_dir=install_dir,
                cache_dir=cache_dir,
            )
            self.assertTrue((install_dir / "proof.bin").is_file())
            marker = json.loads((install_dir / setup_circuits.MARKER_NAME).read_text(encoding="utf-8"))
            self.assertEqual(marker["release"], "v0.5.3")
            self.assertEqual(marker["hash"], sri)
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
                    target={"os": "linux", "arch": "x86_64", "hash": wrong},
                    artifact="circuits.tar.gz",
                    expected_hash=wrong,
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
                target={"os": "linux", "arch": "x86_64", "hash": sri},
                artifact="circuits.tar.gz",
                expected_hash=sri,
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
                        "hash": "sha256-" + base64.b64encode(b"0" * 32).decode("ascii"),
                    }
                ),
                encoding="utf-8",
            )
            cache_dir = base / "cache"
            cache_dir.mkdir()
            (cache_dir / "circuits.tar.gz").write_bytes(data)
            setup_circuits.setup_circuits(
                release="v0.5.3",
                target={"os": "linux", "arch": "x86_64", "hash": sri},
                artifact="circuits.tar.gz",
                expected_hash=sri,
                url="https://example.test/circuits.tar.gz",
                install_dir=install_dir,
                cache_dir=cache_dir,
            )
            self.assertEqual((install_dir / "proof.bin").read_bytes(), b"fresh")


if __name__ == "__main__":
    unittest.main()
