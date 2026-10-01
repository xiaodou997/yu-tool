import json
from pathlib import Path
import tempfile
import unittest
import zipfile

from distribution_notices import bounded_text, cli_notices, dependency_closure, engine_notice_inventory


class NoticeTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.package = self.root / "dep"
        self.package.mkdir()
        (self.package / "LICENSE-MIT").write_text("fixture license text\n", encoding="utf-8")
        self.metadata = {"workspace_members": ["cli"], "packages": [
            {"id": "cli", "name": "yu-cli", "version": "0.1.0", "source": None},
            {"id": "dep", "name": "fixture", "version": "1.0.0", "source": "registry+fixture", "license": "MIT", "license_file": None, "manifest_path": str(self.package / "Cargo.toml")},
            {"id": "dev", "name": "dev-only", "version": "1.0.0", "source": "registry+fixture"}],
            "resolve": {"nodes": [
                {"id": "cli", "deps": [{"pkg": "dep", "dep_kinds": [{"kind": None}]}, {"pkg": "dev", "dep_kinds": [{"kind": "dev"}]}]},
                {"id": "dep", "deps": []}, {"id": "dev", "deps": []}]}}
        self.lock = {"package": [{"name": "fixture", "version": "1.0.0", "source": "registry+fixture", "checksum": "abc"}]}

    def test_resolved_closure_excludes_dev_and_unreachable_packages(self):
        deps, own = dependency_closure(self.metadata)
        self.assertEqual([p["name"] for p in deps], ["fixture"])
        self.assertEqual([p["name"] for p in own], ["yu-cli"])

    def test_nested_notices_and_upstream_declaration_are_preserved(self):
        (self.package / "vendor").mkdir()
        (self.package / "vendor/NOTICE").write_text("native notice\n", encoding="utf-8")
        a = cli_notices(self.metadata, self.lock, "a" * 40, "target")
        self.assertEqual(a, cli_notices(self.metadata, self.lock, "a" * 40, "target"))
        text, inventory = a
        data = json.loads(inventory)
        self.assertIn(b"native notice", text)
        self.assertNotIn(str(self.root), inventory.decode())
        self.assertEqual(len(data["components"][0]["notice_files"]), 2)
        self.assertFalse(data["redistribution_review_accepted"])
        self.assertEqual(data["components"][0]["package_checksum_from_lock"], "abc")

    def test_missing_notice_is_recorded_not_manufactured(self):
        (self.package / "LICENSE-MIT").unlink()
        text, inventory = cli_notices(self.metadata, self.lock, "a" * 40, "target")
        self.assertEqual(json.loads(inventory)["missing_notice_evidence"][0]["reason"], "no_notice_text_discovered")
        self.assertNotIn(b"fixture license text", text)

    def test_unknown_graph_and_external_license_paths_fail(self):
        self.metadata["resolve"]["nodes"][0]["deps"][0]["pkg"] = "missing"
        with self.assertRaises(ValueError):
            dependency_closure(self.metadata)
        outside = self.root / "LICENSE"
        outside.write_text("outside")
        with self.assertRaises(ValueError):
            bounded_text(outside, self.package)
        with self.assertRaises(ValueError):
            bounded_text(self.package / "../LICENSE", self.package)

    def test_notice_symlink_is_not_followed(self):
        link = self.package / "NOTICE"
        try:
            link.symlink_to(self.package / "LICENSE-MIT")
        except OSError as error:
            self.skipTest(f"symlink creation unavailable: {error}")
        with self.assertRaises(ValueError):
            cli_notices(self.metadata, self.lock, "a" * 40, "target")

    def engine(self, missing=False):
        path = self.root / ("missing.zip" if missing else "engine.zip")
        with zipfile.ZipFile(path, "w") as z:
            z.writestr("engine/yu-engine.json", json.dumps({"engine_id": "ag-psd", "engine_version": "fixture", "node_version": "fixture"}))
            if not missing:
                z.writestr("licenses/node-LICENSE", "node aggregate notice fixture")
            for name in ("ag-psd", "base64-js", "pako"):
                z.writestr(f"engine/node_modules/{name}/package.json", json.dumps({"name": name, "version": "fixture", "license": "MIT"}))
                z.writestr(f"engine/node_modules/{name}/LICENSE", "package notice fixture")
        return path

    def test_existing_engine_notice_inspection_never_changes_archive(self):
        path = self.engine()
        before = path.read_bytes()
        report = engine_notice_inventory(path)
        self.assertEqual(len(report["components"]), 4)
        self.assertFalse(report["redistribution_review_accepted"])
        self.assertEqual(path.read_bytes(), before)
        with self.assertRaises(KeyError):
            engine_notice_inventory(self.engine(missing=True))


if __name__ == "__main__":
    unittest.main()
