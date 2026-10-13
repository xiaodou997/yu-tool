"""Self-contained tests for private YuTool v0.1.0-dev internal bundle staging."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import audit_v01_postmerge_artifacts as audit
import build_v01_dev_private_preview as preview


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


class PrivatePreviewTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="yu-private-bundle-test-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.source = self.root/"source"
        self.source.mkdir()
        self.dest = self.root/"preview"
        cli, engine = {}, {}
        for platform in preview.PLATFORMS:
            folder = self.source/platform
            folder.mkdir()
            c = folder/f"cli-{platform}.zip"
            with zipfile.ZipFile(c,"w") as z:
                z.writestr("LICENSE-MIT",b"MIT")
                z.writestr("LICENSE-APACHE",b"Apache")
                z.writestr("THIRD-PARTY-NOTICES.txt",b"Source notices")
            (folder/"candidate.json").write_text(json.dumps({"source_commit":audit.MAIN}))
            engfolder=self.source/f"managed-{platform}"
            engfolder.mkdir()
            m=engfolder/f"engine-{platform}.zip"
            with zipfile.ZipFile(m,"w") as z:
                z.writestr("licenses/node-LICENSE",b"Node license")
            (engfolder/f"manifest-{platform}.json").write_text(json.dumps({"packages":[]}))
            (engfolder/f"metadata-{platform}.json").write_text(json.dumps({"version":"dev"}))
            cli[platform]={"archive_sha256":sha(c.read_bytes())}
            engine[platform]={"archive_sha256":sha(m.read_bytes())}
        self.fake={
            "technical_artifacts_audit":"PASS",
            "cli_candidate":cli,
            "managed_engine":engine,
        }

    def test_bundle_three_platforms_exact_nested_bytes_with_no_release_promotion(self):
        with patch.object(audit,"audit",return_value=self.fake):
            manifest=preview.build(self.source,self.dest,self.root)
        checked=preview.verify(self.dest,self.root)
        self.assertEqual(checked["bundles"],3)
        self.assertEqual(checked["status"],"PASS")
        self.assertFalse(checked["public_release_ready"])
        self.assertFalse(manifest["psd_mutation_allowed"])
        self.assertFalse(manifest["underlying_cli_and_engine_archives_modified"])
        self.assertFalse(manifest["public_release_authorized"])
        for platform,row in manifest["platform_bundles"].items():
            with zipfile.ZipFile(self.dest/row["file"]) as z:
                self.assertTrue(z.read("README-DEV.zh-CN.md").startswith(b"# YuTool"))
                self.assertEqual(json.loads(z.read("BUNDLE.json"))["source_commit"],audit.MAIN)
                self.assertIn("engine/manifest.json",z.namelist())
                self.assertIn("THIRD-PARTY-NOTICES.txt",z.namelist())

    def test_existing_destination_refused_without_modifying_it(self):
        self.dest.mkdir()
        marker=self.dest/"important.txt"
        marker.write_text("preserve me")
        with patch.object(audit,"audit",return_value=self.fake):
            with self.assertRaises(FileExistsError):
                preview.build(self.source,self.dest,self.root)
        self.assertEqual(marker.read_text(),"preserve me")

    def test_wrong_artifact_checksum_fails_before_stage(self):
        platform="linux"
        f=next((self.source/platform).glob("*.zip"))
        with f.open("ab") as stream:stream.write(b"bad")
        with patch.object(audit,"audit",return_value=self.fake):
            with self.assertRaisesRegex(ValueError,"hash changed"):
                preview.build(self.source,self.dest,self.root)
        # Other bundles could be staged if input changed mid-build,
        # but they must never be represented as successfully verified.
        self.assertFalse((self.dest/"INTERNAL-DEV-MANIFEST.json").exists())

    def test_corrupt_outer_zip_detected(self):
        with patch.object(audit,"audit",return_value=self.fake):
            manifest=preview.build(self.source,self.dest,self.root)
        f=self.dest/manifest["platform_bundles"]["macos"]["file"]
        with f.open("ab") as stream:stream.write(b"tamper")
        with self.assertRaisesRegex(ValueError,"modified platform internal ZIP"):
            preview.verify(self.dest,self.root)

    def test_tampered_release_manifest_flag_is_rejected(self):
        with patch.object(audit,"audit",return_value=self.fake):
            preview.build(self.source,self.dest,self.root)
        f=self.dest/"INTERNAL-DEV-MANIFEST.json"
        manifest=json.loads(f.read_text())
        manifest["public_release_authorized"]=True
        f.write_text(json.dumps(manifest))
        with self.assertRaisesRegex(ValueError,"falsely upgraded"):
            preview.verify(self.dest,self.root)


if __name__=="__main__":
    unittest.main()
