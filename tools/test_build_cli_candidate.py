import json
from pathlib import Path
import tempfile
import unittest
import zipfile

from build_cli_candidate import archive_bytes, digest, unpack_verified, write_archive


class CandidateTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.binary = b"fixture binary (never executed)"
        self.metadata = {"target":"aarch64-apple-darwin", "binary_sha256":digest(self.binary), "public_release_ready":False}

    def test_deterministic_container_and_verified_extraction(self):
        a, b = self.root / "a.zip", self.root / "b.zip"
        write_archive(a, self.binary, self.metadata)
        write_archive(b, self.binary, self.metadata)
        self.assertEqual(a.read_bytes(), b.read_bytes())
        binary, metadata = unpack_verified(a, self.root / "unpacked")
        self.assertEqual(binary.read_bytes(), self.binary)
        self.assertFalse(metadata["public_release_ready"])

    def test_archive_and_output_conflicts_are_rejected(self):
        a = self.root / "a.zip"
        write_archive(a, self.binary, self.metadata)
        with self.assertRaises(FileExistsError):
            write_archive(a, self.binary, self.metadata)
        with self.assertRaises(FileExistsError):
            unpack_verified(a, self.root)

    def test_false_release_claim_and_hash_mismatch_are_rejected(self):
        with self.assertRaises(ValueError):
            archive_bytes(self.binary, dict(self.metadata, public_release_ready=True))
        with self.assertRaises(ValueError):
            archive_bytes(b"changed", self.metadata)

    def test_unknown_paths_are_never_extracted(self):
        a = self.root / "bad.zip"
        with zipfile.ZipFile(a, "w") as archive:
            archive.writestr("build-info.json", json.dumps(self.metadata))
            archive.writestr("../escape", b"bad")
            archive.writestr("bin/yu", self.binary)
        with self.assertRaises(ValueError):
            unpack_verified(a, self.root / "out")
        self.assertFalse((self.root / "out").exists())

    def test_payload_tampering_is_rejected_before_extraction(self):
        a = self.root / "bad.zip"
        from build_cli_candidate import NOTICE
        import stat
        with zipfile.ZipFile(a, "w") as archive:
            for name, data in {"build-info.json":json.dumps(self.metadata).encode(), "bin/yu":b"changed", "RELEASE-STATUS.txt":NOTICE.encode()}.items():
                info = zipfile.ZipInfo(name)
                info.external_attr = (stat.S_IFREG | 0o644) << 16
                archive.writestr(info, data)
        with self.assertRaises(ValueError):
            unpack_verified(a, self.root / "out")
        self.assertFalse((self.root / "out").exists())


if __name__ == "__main__":
    unittest.main()
