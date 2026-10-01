import json
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch
import zipfile

from accept_cli_candidate import accept, candidate_archive, isolated_environment, tiny_png
from build_cli_candidate import archive_bytes, digest, unpack_verified, write_archive


class DistributionTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.binary = b"inert fixture, never executed"
        self.meta = {"target": "aarch64-apple-darwin", "source_commit": "a" * 40, "binary_sha256": digest(self.binary), "public_release_ready": False, "archive_layout_version": 2}
        self.extra = {"THIRD-PARTY-NOTICES.txt": b"fixture text", "USAGE.md": b"fixture usage", "dependency-inventory.json": json.dumps({"kind": "resolved_dependency_notice_inventory", "source_commit": "a" * 40, "target": "aarch64-apple-darwin", "redistribution_review_accepted": False, "public_release_ready": False}).encode()}
        self.meta["auxiliary_sha256"] = {k: digest(v) for k, v in self.extra.items()}

    def test_layout_v2_is_deterministic_and_all_distribution_files_round_trip(self):
        a, b = self.root / "a.zip", self.root / "b.zip"
        write_archive(a, self.binary, self.meta, self.extra)
        write_archive(b, self.binary, self.meta, self.extra)
        self.assertEqual(a.read_bytes(), b.read_bytes())
        executable, _ = unpack_verified(a, self.root / "out")
        self.assertEqual(executable.read_bytes(), self.binary)
        for name, data in self.extra.items():
            self.assertEqual((self.root / "out" / name).read_bytes(), data)

    def raw_archive(self, data, symlink=None):
        archive = self.root / "bad.zip"
        with zipfile.ZipFile(archive, "w") as z:
            for name, content in data.items():
                info = zipfile.ZipInfo(name)
                info.external_attr = ((stat.S_IFLNK if name == symlink else stat.S_IFREG) | 0o644) << 16
                z.writestr(info, content)
        return archive

    def test_notice_tamper_is_rejected_before_any_extraction(self):
        members = archive_bytes(self.binary, self.meta, self.extra)
        members["THIRD-PARTY-NOTICES.txt"] = b"modified"
        with self.assertRaises(ValueError):
            unpack_verified(self.raw_archive(members), self.root / "out")
        self.assertFalse((self.root / "out").exists())

    def test_symlink_notice_is_rejected_before_any_extraction(self):
        with self.assertRaises(ValueError):
            unpack_verified(self.raw_archive(archive_bytes(self.binary, self.meta, self.extra), "USAGE.md"), self.root / "out")
        self.assertFalse((self.root / "out").exists())

    def test_missing_extra_unknown_layout_and_false_inventory_identity_fail(self):
        with self.assertRaises(ValueError):
            archive_bytes(self.binary, self.meta, {})
        with self.assertRaises(ValueError):
            archive_bytes(self.binary, dict(self.meta, archive_layout_version=99), self.extra)
        value = json.loads(self.extra["dependency-inventory.json"])
        value["source_commit"] = "b" * 40
        self.extra["dependency-inventory.json"] = json.dumps(value).encode()
        self.meta["auxiliary_sha256"] = {k: digest(v) for k, v in self.extra.items()}
        with self.assertRaises(ValueError):
            archive_bytes(self.binary, self.meta, self.extra)

    def test_environment_excludes_developer_credentials_and_trace(self):
        env = isolated_environment(self.root, {"SystemRoot": "C:/Windows", "PATH": "tools", "TOKEN": "secret", "CARGO_HOME": "cargo", "YU_WINDOWS_LIFECYCLE_TRACE_DIR": "trace", "HOME": "old"})
        self.assertEqual(env["PATH"], "")
        self.assertEqual(env["SystemRoot"], "C:/Windows")
        self.assertNotIn("TOKEN", env)
        self.assertNotIn("CARGO_HOME", env)
        self.assertNotIn("YU_WINDOWS_LIFECYCLE_TRACE_DIR", env)
        self.assertNotEqual(env["HOME"], "old")
        self.assertTrue(Path(env["TEMP"]).is_dir())

    def test_receipt_traversal_and_digest_mismatch_never_execute(self):
        for name in ("../evil.zip", "dir/evil.zip", "dir\\evil.zip"):
            (self.root / "candidate.json").write_text(json.dumps({"archive": name, "public_release_ready": False, "sha256": "a" * 64}))
            with self.assertRaises(ValueError):
                candidate_archive(self.root)
        (self.root / "candidate.json").write_text(json.dumps({"archive": "bad.zip", "public_release_ready": False, "sha256": "a" * 64}))
        (self.root / "bad.zip").write_bytes(b"not trusted")
        with patch("accept_cli_candidate.subprocess.run") as runner:
            result = accept(self.root, self.root / "unused", self.root / "unused", self.root / "unused", self.root / "report", "a" * 40)
            self.assertEqual(result["status"], "failed")
            runner.assert_not_called()
        self.assertTrue((self.root / "report/report.json").is_file())

    def test_tiny_fixture_is_a_two_by_two_rgba_png(self):
        import struct, zlib
        data = tiny_png()
        self.assertEqual(data[:8], b"\x89PNG\r\n\x1a\n")
        self.assertEqual(struct.unpack(">II", data[16:24]), (2, 2))
        offset = 8
        while offset < len(data):
            size = int.from_bytes(data[offset:offset + 4], "big")
            kind_and_data = data[offset + 4:offset + 8 + size]
            self.assertEqual(zlib.crc32(kind_and_data), int.from_bytes(data[offset + 8 + size:offset + 12 + size], "big"))
            offset += 12 + size
        self.assertEqual(offset, len(data))


if __name__ == "__main__":
    unittest.main()
