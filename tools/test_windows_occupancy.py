from pathlib import Path
import ctypes as C
import json
import os
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import collect_windows_occupancy as collector
import windows_occupancy as occupancy


class OccupancyTests(unittest.TestCase):
    def test_file_selection_is_bounded_and_does_not_follow_links(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "version"
            root.mkdir()
            (root / "runtime").mkdir()
            (root / "runtime/node.exe").write_bytes(b"fixture")
            (root / ".yu-install.json").write_text("{}")
            outside = Path(temporary) / "outside"
            outside.mkdir()
            (outside / "private.txt").write_text("not selected")
            if os.name != "nt":
                (root / "link").symlink_to(outside, target_is_directory=True)
            for index in range(100):
                (root / "runtime" / f"item-{index}.bin").write_bytes(b"")
            selection = occupancy.select_files(root)
            self.assertLessEqual(len(selection["files"]), occupancy.MAX_FILES)
            self.assertTrue(selection["truncated"])
            self.assertFalse(any("private.txt" in file for file in selection["files"]))
            self.assertTrue(all(Path(file).is_file() for file in selection["files"]))

    def test_windows_paths_are_component_bound_and_extended_prefix_aware(self):
        root = Path("C:\\version")
        self.assertTrue(occupancy.inside("\\\\?\\C:\\version\\runtime\\node.exe", root))
        self.assertFalse(occupancy.inside("C:\\version-elsewhere\\node.exe", root))
        self.assertEqual(occupancy.normalize_windows_path("\\\\?\\UNC\\host\\share\\dir"), "\\\\host\\share\\dir")

    def test_trace_matching_preserves_missing_cleanup_and_rejects_unrelated_records(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            record = {"kind":"owned_windows_job_snapshot", "version_directory":"\\\\?\\C:\\version", "stage":"started"}
            (directory / "trace.json").write_text(json.dumps(record))
            (directory / "other.json").write_text(json.dumps(dict(record, version_directory="C:\\other")))
            found = collector.matching_traces(directory, Path("C:\\version"))
            self.assertEqual(found["records"], [record])
            self.assertEqual(found["records"][0]["stage"], "started")
            self.assertNotIn("root_cause_fixed", found)

    def test_pid_reuse_never_claims_a_known_owned_process(self):
        traces = {"records":[{"child_pid":42, "child_created_filetime":"100", "invocation":"first"}]}
        user = {"pid":42, "identity_matches_snapshot":True, "restart_manager_created_filetime":"101"}
        result = collector.correlate({"applications":[user]}, traces)
        self.assertEqual(result["observations"][0]["relationship"], "not_matched_to_recorded_direct_child")
        user["restart_manager_created_filetime"] = "100"
        result = collector.correlate({"applications":[user]}, traces)
        self.assertEqual(result["observations"][0]["matched_invocations"], ["first"])
        self.assertFalse(result["rename_blocker_proven"])
        user["identity_matches_snapshot"] = False
        self.assertFalse(collector.correlate({"applications":[user]}, traces)["observations"][0]["matched_invocations"])

    def test_capture_never_overwrites_previous_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaises(FileExistsError):
                collector.capture(Path(temporary), Path(temporary), None)

    def test_native_query_timeout_retains_trigger_and_is_not_success(self):
        with tempfile.TemporaryDirectory() as temporary:
            target = Path(temporary) / "capture"
            def runner(*args, **kwargs):
                raise subprocess.TimeoutExpired(args[0], 10)
            with patch.object(collector, "IS_WINDOWS", True):
                report = collector.capture(Path(temporary), target, None, runner)
            self.assertEqual(report["status"], "collector_timeout")
            self.assertFalse(report["root_cause_fixed"])
            self.assertTrue((target / "trigger.json").is_file())

    @unittest.skipUnless(os.name == "nt", "requires real Windows resource APIs")
    def test_windows_file_holder_is_identified_without_unlocking(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "version"
            root.mkdir()
            file = root / "held.txt"
            file.write_bytes(b"keep intact")
            api = occupancy.WindowsAPI()
            held = api.open_file(str(file), 0x80000000, 3, None, 3, 0, None)
            self.assertNotEqual(held, C.c_void_p(-1).value)
            try:
                out = Path(temporary) / "capture"
                report = collector.capture(root, out, None)
                self.assertEqual(report["status"], "collected", list(out.iterdir()))
                users = json.loads((out / "05-resource-users.json").read_text())
                self.assertEqual(users["status"], "observed", users)
                self.assertTrue(any(p["pid"] == os.getpid() and p["identity_matches_snapshot"] for p in users["applications"]), users)
                self.assertFalse(users["directory_handles_covered"])
                self.assertEqual(file.read_bytes(), b"keep intact")
                with self.assertRaises(OSError):
                    file.rename(root / "renamed.txt")
            finally:
                api.close(held)
            file.rename(root / "renamed.txt")

    @unittest.skipUnless(os.name == "nt", "requires real Windows directory sharing")
    def test_windows_directory_probe_keeps_unknown_owner_and_preserves_handle(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "version"
            root.mkdir()
            api = occupancy.WindowsAPI()
            held = api.open_file(str(root), 0x80000000, 3, None, 3, 0x02000000, None)
            self.assertNotEqual(held, C.c_void_p(-1).value)
            try:
                probe = api.delete_access_probe(root)
                self.assertEqual(probe["status"], "open_failed", probe)
                self.assertEqual(probe["os_error"], 32, probe)
                self.assertTrue(root.is_dir())
            finally:
                api.close(held)
            self.assertEqual(api.delete_access_probe(root)["status"], "opened_and_closed")


if __name__ == "__main__":
    unittest.main()
