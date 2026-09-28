from pathlib import Path
import json
import subprocess
import tempfile
import unittest
from types import SimpleNamespace
import run_lifecycle_probe as probe


class LifecycleProbeTests(unittest.TestCase):
    def test_candidate_identity_rejects_stale_source_and_changed_binary(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "bin").mkdir()
            binary = root / "bin" / "yu"
            binary.write_bytes(b"candidate")
            metadata = {"kind": "developer_candidate", "source_commit": "a" * 40, "public_release_ready": False, "binary_sha256": probe.hash_path(binary)}
            (root / "build-info.json").write_text(json.dumps(metadata))
            self.assertEqual(probe.verify_candidate(binary, "a" * 40), metadata)
            with self.assertRaises(ValueError):
                probe.verify_candidate(binary, "b" * 40)
            binary.write_bytes(b"changed")
            with self.assertRaises(ValueError):
                probe.verify_candidate(binary, "a" * 40)

    def test_success_requires_every_step_and_keeps_release_blocked(self):
        with tempfile.TemporaryDirectory() as temporary:
            calls = []
            def runner(argv, **kwargs):
                calls.append(argv)
                kwargs["stdout"].write(b"test result: ok. 5 passed; 0 failed; 0 ignored;\n")
                return SimpleNamespace(returncode=0)
            path = Path(temporary) / "report"
            report = probe.run_plan(path, 2, {}, "a" * 40, runner)
            self.assertEqual(len(calls), 4)
            self.assertEqual(report["status"], "passed")
            self.assertFalse(report["public_release_ready"])
            self.assertFalse(report["root_cause_fixed"])
            self.assertEqual(json.loads((path / "report.json").read_text()), report)

    def test_failure_is_retained_and_never_retried(self):
        with tempfile.TemporaryDirectory() as temporary:
            calls = []
            def runner(argv, **kwargs):
                calls.append(argv)
                kwargs["stdout"].write(b"first failure retained\n")
                return SimpleNamespace(returncode=1)
            path = Path(temporary) / "report"
            report = probe.run_plan(path, 3, {}, "a" * 40, runner)
            self.assertEqual(len(calls), 1)
            self.assertEqual(report["status"], "failed")
            self.assertEqual(report["failed_case_retries"], 0)
            self.assertIn("first failure", (path / "01-transport.log").read_text())

    def test_zero_executed_tests_is_not_success(self):
        with tempfile.TemporaryDirectory() as temporary:
            def runner(argv, **kwargs):
                kwargs["stdout"].write(b"test result: ok. 0 passed; 0 failed;\n")
                return SimpleNamespace(returncode=0)
            report = probe.run_plan(Path(temporary) / "report", 1, {}, "a" * 40, runner)
            self.assertEqual(report["steps"][0]["reason"], "missing_executed_test_evidence")
            self.assertEqual(report["status"], "failed")

    def test_timeout_is_a_failure_not_an_empty_pass(self):
        with tempfile.TemporaryDirectory() as temporary:
            def runner(argv, **kwargs):
                raise subprocess.TimeoutExpired(argv, 600)
            report = probe.run_plan(Path(temporary) / "report", 1, {}, "a" * 40, runner)
            self.assertEqual(report["status"], "failed")
            self.assertEqual(report["steps"][0]["reason"], "outer_command_timeout")

    def test_existing_evidence_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary)
            sentinel = path / "report.json"
            sentinel.write_text("prior evidence")
            with self.assertRaises(FileExistsError):
                probe.run_plan(path, 1, {}, "a" * 40)
            self.assertEqual(sentinel.read_text(), "prior evidence")

    def test_invalid_repetition_does_not_start_work(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "report"
            with self.assertRaises(ValueError):
                probe.run_plan(path, 0, {}, "a" * 40)
            self.assertFalse(path.exists())


if __name__ == "__main__":
    unittest.main()
