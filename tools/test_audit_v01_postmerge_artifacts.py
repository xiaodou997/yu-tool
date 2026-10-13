"""Hermetic tests for exact-main artifact integrity and release flag refusal."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import audit_v01_postmerge_artifacts as a


def encode(obj: dict) -> bytes:
    return json.dumps(obj,sort_keys=True).encode()


class ExactMainArtifactAuditTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory(prefix="yu-rc-audit-")
        self.addCleanup(self.tmp.cleanup)
        self.dir = Path(self.tmp.name)

    def build_cli(self) -> Path:
        entries = {
            "LICENSE-MIT": b"license-mit\n",
            "LICENSE-APACHE": b"license-apache\n",
            "RELEASE-STATUS.txt": b"Developer validation artifact only",
            "THIRD-PARTY-NOTICES.txt": b"notices",
            "USAGE.md": b"usage",
            "bin/yu": b"ELF-placeholder",
        }
        inventory = {
            "source_commit": a.MAIN,
            "target": a.TARGETS["linux"],
            "components": [{"name":f"dependency{i}"} for i in range(169)],
            "missing_notice_evidence": [],
            "public_release_ready": False,
            "redistribution_review_accepted": False,
            "project_components": [{"name":f"yu{i}",
                                    "declared_license":"MIT OR Apache-2.0"} for i in range(8)],
        }
        entries["dependency-inventory.json"]=encode(inventory)
        info = {
            "archive_layout_version":3,
            "source_commit":a.MAIN, "source_tree":a.TREE,
            "target":a.TARGETS["linux"],
            "project_license":"MIT OR Apache-2.0",
            "public_release_ready":False,
            "redistribution_review_accepted":False,
            "signing_notarization_accepted":False,
            "binary_sha256":a.digest(entries["bin/yu"]),
            "auxiliary_sha256":{k:a.digest(entries[k]) for k in
                ("LICENSE-MIT","LICENSE-APACHE","THIRD-PARTY-NOTICES.txt",
                 "USAGE.md","dependency-inventory.json")},
        }
        entries["build-info.json"]=encode(info)
        f=self.dir/"candidate-linux.zip"
        with zipfile.ZipFile(f,"w") as z:
            for name, data in entries.items():z.writestr(name,data)
        digest=hashlib.sha256(f.read_bytes()).hexdigest()
        (self.dir/"candidate.json").write_text(json.dumps({
            "archive":f.name,
            "source_commit":a.MAIN,
            "public_release_ready":False,
            "packaged_cli_smoke":"passed",
            "sha256":digest,
            "bytes":f.stat().st_size,
        }))
        (self.dir/"SHA256SUMS").write_text(f"{digest}  {f.name}\n")
        return f

    def test_cli_good_integrity_but_never_legal_acceptance(self):
        self.build_cli()
        with patch.object(a,"source_blob",side_effect=lambda _repo,p:
            b"license-mit\n" if p=="LICENSE-MIT" else b"license-apache\n"):
            report=a.cli(self.dir,"linux",self.dir)
        self.assertEqual(report["resolved_third_party_components"],169)
        self.assertTrue(report["binary_and_all_auxiliary_hashes_match"])
        self.assertFalse(report["independent_legal_redistribution_acceptance"])
        self.assertFalse(report["public_release_ready"])

    def test_cli_wrong_source_rejected(self):
        self.build_cli()
        p=self.dir/"candidate.json"
        manifest=json.loads(p.read_text())
        manifest["source_commit"]="0"*40
        p.write_text(json.dumps(manifest))
        with self.assertRaisesRegex(ValueError,"candidate commit mismatch"):
            a.cli(self.dir,"linux",self.dir)

    def test_cli_wrong_archive_hash_rejected(self):
        f=self.build_cli()
        with f.open("ab") as out:out.write(b"not included in approved ZIP")
        with self.assertRaisesRegex(ValueError,"archive SHA mismatch"):
            a.cli(self.dir,"linux",self.dir)

    def test_cli_missing_notice_evidence_rejected(self):
        self.build_cli()
        archive=next(self.dir.glob("*.zip"))
        with zipfile.ZipFile(archive) as z:
            old={n:z.read(n) for n in z.namelist()}
        inv=json.loads(old["dependency-inventory.json"])
        inv["missing_notice_evidence"]=[{"component":"example"}]
        old["dependency-inventory.json"]=encode(inv)
        info=json.loads(old["build-info.json"])
        info["auxiliary_sha256"]["dependency-inventory.json"] = a.digest(
            old["dependency-inventory.json"])
        old["build-info.json"]=encode(info)
        with zipfile.ZipFile(archive,"w") as z:
            for name,data in old.items():z.writestr(name,data)
        meta_path=self.dir/"candidate.json"
        meta=json.loads(meta_path.read_text())
        meta["sha256"]=a.digest(archive.read_bytes())
        meta["bytes"]=archive.stat().st_size
        meta_path.write_text(json.dumps(meta))
        (self.dir/"SHA256SUMS").write_text(f"{meta['sha256']}  {archive.name}\n")
        with patch.object(a,"source_blob",side_effect=lambda _repo,p:
            b"license-mit\n" if p=="LICENSE-MIT" else b"license-apache\n"):
            with self.assertRaisesRegex(ValueError,"missing notice"):
                a.cli(self.dir,"linux",self.dir)

    def test_managed_engine_requirements(self):
        names={
            "licenses/node-LICENSE":b"node aggregate license",
            "engine/node_modules/ag-psd/LICENSE":b"MIT",
            "engine/node_modules/base64-js/LICENSE":b"MIT",
            "engine/node_modules/pako/LICENSE":b"MIT AND Zlib",
        }
        for package,ver,lic in [
            ("ag-psd","31.0.2","MIT"),
            ("base64-js","1.5.1","MIT"),
            ("pako","2.1.0","(MIT AND Zlib)"),
        ]:
            names[f"engine/node_modules/{package}/package.json"]=encode({
                "name":package,"version":ver,"license":lic
            })
        f=self.dir/"engine.zip"
        with zipfile.ZipFile(f,"w") as z:
            for name,data in names.items():z.writestr(name,data)
        meta={
            "sha256":a.digest(f.read_bytes()),"bytes":f.stat().st_size,
            "engine_version":a.ENGINE,"engine_id":"ag-psd",
            "url":"https://example.invalid/engine.zip",
        }
        (self.dir/"metadata-test.json").write_text(json.dumps(meta))
        result=a.managed(self.dir,"linux")
        self.assertEqual(set(result["notice_components"]),a.ENGINE_NAMES)
        self.assertFalse(result["public_release_ready"])
        self.assertTrue(result["placeholder_url_in_prototype_manifest"])

    def test_zip_traversal_rejected_without_extraction(self):
        archive=self.dir/"traversal.zip"
        with zipfile.ZipFile(archive,"w") as z:
            z.writestr("../outside",b"bad")
        with self.assertRaisesRegex(ValueError,"unsafe ZIP path"):
            a.safe_archive(archive)


if __name__=="__main__":
    unittest.main()
