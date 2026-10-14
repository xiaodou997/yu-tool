"""Hermetic tests for offline Mac first-user transfer kit packaging."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import build_first_user_mac_kit as kit


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


class FirstUserMacKitTests(unittest.TestCase):
    def setUp(self):
        temp=tempfile.TemporaryDirectory(prefix="yu-first-user-kit-test-")
        self.addCleanup(temp.cleanup)
        self.root=Path(temp.name)
        self.preview=self.root/"preview"
        self.preview.mkdir()
        self.out=self.root/"MacKit"
        self.repo=Path(__file__).resolve().parent.parent
        self.sample=b"fake nested Mac ZIP bytes for packaging test"
        (self.preview/kit.MAC_ZIP).write_bytes(self.sample)

    def stage(self):
        with patch.object(kit,"MAC_SHA",sha(self.sample)), patch.object(
                kit,"verify_preview",return_value={"status":"PASS","bundles":3}):
            return kit.build(self.preview,self.repo,self.out)

    def test_build_and_verify_frozen_manifest_without_any_air_pass_claim(self):
        result=self.stage()
        with patch.object(kit,"MAC_SHA",sha(self.sample)):
            evidence=kit.verify(self.out)
        self.assertEqual(evidence["status"],"PASS")
        self.assertEqual(evidence["checked_members"],8)
        self.assertEqual(evidence["air_first_user_acceptance"],"PENDING")
        self.assertEqual(result["air_gate"],"PENDING")
        folder=self.out/"YuTool-First-User-Mac"
        self.assertEqual((folder/kit.MAC_ZIP).read_bytes(),self.sample)
        self.assertEqual((folder/"test-fixtures/sample.png").stat().st_size,75)
        self.assertIn("不全局关闭 Gatekeeper",kit.QUICKSTART)

    def test_source_hash_mismatch_fails_before_creating_output(self):
        with patch.object(kit,"verify_preview",return_value={"status":"PASS","bundles":3}):
            with self.assertRaisesRegex(ValueError,"bundle SHA mismatch"):
                kit.build(self.preview,self.repo,self.out)
        self.assertFalse(self.out.exists())

    def test_existing_output_refused_and_contents_preserved(self):
        self.out.mkdir()
        marker=self.out/"private-user.txt"
        marker.write_text("do not overwrite")
        with patch.object(kit,"MAC_SHA",sha(self.sample)),patch.object(
                kit,"verify_preview",return_value={"status":"PASS","bundles":3}):
            with self.assertRaisesRegex(ValueError,"already exists"):
                kit.build(self.preview,self.repo,self.out)
        self.assertEqual(marker.read_text(),"do not overwrite")

    def test_tampering_outer_zip_rejected(self):
        self.stage()
        bundle=next(self.out.glob("*.zip"))
        with bundle.open("ab") as stream:stream.write(b"tamper")
        with self.assertRaisesRegex(ValueError,"SHA256SUMS changed"):
            kit.verify(self.out)

    def test_fabricated_macbook_air_accepted_status_refused_even_with_rehashed_zip(self):
        self.stage()
        bundle=next(self.out.glob("*.zip"))
        with zipfile.ZipFile(bundle) as z:
            entries={name:z.read(name) for name in z.namelist()}
        name="YuTool-First-User-Mac/KIT-MANIFEST.json"
        meta=json.loads(entries[name])
        meta["automated_macbook_air_real_first_run"]="PASS"
        entries[name]=json.dumps(meta).encode()
        with zipfile.ZipFile(bundle,"w",compression=zipfile.ZIP_STORED) as z:
            for name,data in entries.items():z.writestr(name,data)
        (self.out/(bundle.name+".sha256")).write_text(f"{sha(bundle.read_bytes())}  {bundle.name}\n")
        with self.assertRaisesRegex(ValueError,"unapproved FUA claim"):
            kit.verify(self.out)

    def test_missing_psd_sample_and_developer_tools_are_not_required(self):
        self.stage()
        with zipfile.ZipFile(next(self.out.glob("*.zip"))) as z:
            script=z.read("YuTool-First-User-Mac/First-User-Test.command")
            self.assertTrue(script.startswith(b"#!/bin/bash"))
            self.assertNotIn(b"cargo ",script.lower())
            self.assertNotIn(b"spctl --master-disable",script)
            self.assertNotIn(b"xattr -d ",script)
            self.assertIn(b"/usr/bin/ditto",script)
        (self.repo/"fixtures/psd/upstream/psd-tools/2layers.psd").stat()

    @unittest.skipUnless(sys.platform=="darwin","macOS-only shell preflight")
    def test_mac_shell_refuses_wrong_sha_before_any_cli_execution(self):
        folder=self.root/"invalid-mac-kit"
        (folder/"test-fixtures").mkdir(parents=True)
        (folder/"First-User-Test.command").write_bytes(
            (self.repo/"tools/first_user_mac_acceptance.sh").read_bytes())
        (folder/kit.MAC_ZIP).write_bytes(b"tampered test ZIP")
        (folder/"test-fixtures/2layers.psd").write_bytes(
            (self.repo/"fixtures/psd/upstream/psd-tools/2layers.psd").read_bytes())
        (folder/"test-fixtures/sample.png").write_bytes(kit.tiny_png())
        result=subprocess.run(["/bin/bash",str(folder/"First-User-Test.command"),
                               "--verify-only"],capture_output=True,text=True,timeout=20)
        self.assertEqual(result.returncode,2)
        self.assertIn("SHA-256",result.stderr)
        self.assertEqual(list(folder.glob("yu-first-user-*")),[])


if __name__=="__main__":
    unittest.main()
