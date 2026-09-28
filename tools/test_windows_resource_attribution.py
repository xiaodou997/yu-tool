from __future__ import annotations
import ctypes as C
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

import collect_windows_occupancy as collector
import windows_occupancy as occupancy
import windows_resource_attribution as attribution


def snapshot(*keys):
    return {"status": "observed", "os_error": 0, "end_session_error": 0,
            "applications": [{"pid": pid, "restart_manager_created_filetime": birth,
                "created_filetime": birth, "identity_matches_snapshot": True,
                "status": "observed", "exit_filetime": "0"} for pid, birth in keys]}


class AttributionTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve() / "version"
        self.root.mkdir()
        self.files = [self.root / f"file-{n}.bin" for n in range(8)]
        for path in self.files:
            path.write_bytes(b"unchanged fixture")
        self.selection = {"files": list(map(str, self.files)), "truncated": False}

    def test_group_results_are_refined_until_positive_singletons(self):
        calls, journal = [], []
        targets = {str(self.files[2]): (42, "100"), str(self.files[6]): (43, "200")}
        def query(paths):
            calls.append(paths[:])
            return snapshot(*(targets[p] for p in paths if p in targets))
        result = attribution.attribute_files(self.root, self.selection, snapshot(*targets.values()), query, journal.append)
        self.assertEqual(result["status"], "verified_targets_localized", result)
        self.assertEqual({v["relative_path"] for v in result["witnesses"]}, {"file-2.bin", "file-6.bin"})
        self.assertTrue(all(len(calls[v["query"] - 1]) == 1 for v in result["witnesses"]))
        self.assertLessEqual(len(calls), attribution.MAX_QUERIES)
        self.assertEqual(result["queries_completed"], len(calls))
        self.assertFalse(result["rename_blocker_proven"])
        self.assertTrue(all(path.read_bytes() == b"unchanged fixture" for path in self.files))
        self.assertEqual(sum(e["event"] == "query_started" for e in journal), len(calls))

    def test_reused_pid_or_new_unanchored_process_is_not_attributed(self):
        for observed in [snapshot((42, "101")), snapshot((43, "100"))]:
            result = attribution.attribute_files(self.root, self.selection, snapshot((42, "100")), lambda _: observed)
            self.assertEqual(result["witnesses"], [])
            self.assertEqual(result["unresolved_identities"], [{"pid": 42, "created_filetime": "100"}])

    def test_unverified_or_exited_identities_do_not_trigger_queries(self):
        for changes in [{"created_filetime": "101"}, {"identity_matches_snapshot": False},
                        {"exit_filetime": "200"}, {"pid": True}, {"restart_manager_created_filetime": "x"},
                        {"restart_manager_created_filetime": "9" * 5000}]:
            initial = snapshot((42, "100"))
            initial["applications"][0].update(changes)
            result = attribution.attribute_files(self.root, self.selection, initial,
                lambda _: self.fail("unverified identity queried"))
            self.assertEqual(result["status"], "no_verified_targets")

    def test_truncated_or_failed_query_never_becomes_a_singleton_witness(self):
        for status in ["truncated", "query_failed", "register_failed"]:
            value = dict(snapshot((42, "100")), status=status)
            result = attribution.attribute_files(self.root, {"files": [str(self.files[0])]},
                snapshot((42, "100")), lambda _: value)
            self.assertEqual(result["witnesses"], [])
            self.assertEqual(result["query_failures"], 1)

    def test_empty_resamples_do_not_infer_a_user_in_the_complement(self):
        calls = []
        def query(paths):
            calls.append(paths)
            return snapshot()
        result = attribution.attribute_files(self.root, self.selection, snapshot((42, "100")), query)
        self.assertEqual(len(calls), 2)
        self.assertEqual(result["witnesses"], [])
        self.assertFalse(result["directory_handles_covered"])

    def test_query_budget_stops_without_guessing_singletons(self):
        result = attribution.attribute_files(self.root, self.selection, snapshot((42, "100")),
            lambda _: snapshot((42, "100")), max_queries=1)
        self.assertEqual(result["status"], "query_budget_exhausted")
        self.assertEqual(result["queries_started"], 1)
        self.assertEqual(result["witnesses"], [])
        with self.assertRaises(ValueError):
            attribution.attribute_files(self.root, self.selection, snapshot(), lambda _: snapshot(), max_queries=17)

    def test_elapsed_budget_and_session_end_failure_stop_queries(self):
        now = [0.0]
        def slow(_):
            now[0] += 4
            return snapshot((42, "100"))
        result = attribution.attribute_files(self.root, self.selection, snapshot((42, "100")), slow, clock=lambda: now[0])
        self.assertEqual(result["status"], "scheduling_budget_exhausted")
        self.assertEqual(result["queries_started"], 1)
        result = attribution.attribute_files(self.root, self.selection, snapshot((42, "100")),
            lambda _: dict(snapshot((42, "100")), end_session_error=5))
        self.assertEqual(result["status"], "session_cleanup_failed")
        self.assertEqual(result["queries_started"], 1)

    def test_query_start_is_retained_when_native_query_is_interrupted(self):
        journal = []
        def interrupted(_):
            raise KeyboardInterrupt()
        with self.assertRaises(KeyboardInterrupt):
            attribution.attribute_files(self.root, self.selection, snapshot((42, "100")), interrupted, journal.append)
        self.assertEqual(journal[-1]["event"], "query_started")
        self.assertFalse(any(e["event"] == "query_completed" for e in journal))

    def test_scope_rejects_outside_duplicate_and_directory_paths(self):
        for files in [[str(self.root.parent / "outside")], [str(self.root)],
                      [str(self.files[0])] * 2, [str(self.root / ".." / "outside")]]:
            result = attribution.attribute_files(self.root, {"files": files}, snapshot((42, "100")),
                lambda _: self.fail("unsafe selection queried"))
            self.assertEqual(result["status"], "invalid_selection")

    @unittest.skipIf(os.name == "nt", "portable symlink fixture; native Windows calibration uses regular paths")
    def test_reparse_change_is_refused_before_the_next_query(self):
        calls = []
        outside = self.root.parent / "private"
        outside.write_bytes(b"not queried")
        def query(paths):
            calls.append(paths)
            self.files[6].unlink()
            self.files[6].symlink_to(outside)
            return snapshot()
        result = attribution.attribute_files(self.root, self.selection, snapshot((42, "100")), query)
        self.assertEqual(len(calls), 1)
        self.assertEqual(result["path_failures"], 1)
        self.assertEqual(result["witnesses"], [])

    def test_failed_baseline_and_oversized_selection_do_not_dispatch(self):
        result = attribution.attribute_files(self.root, self.selection, {"status": "truncated"},
            lambda _: self.fail("failed baseline queried"))
        self.assertEqual(result["status"], "baseline_unavailable")
        result = attribution.attribute_files(self.root, {"files": ["x"] * 65}, snapshot((42, "100")),
            lambda _: self.fail("oversized selection queried"))
        self.assertEqual(result["status"], "invalid_selection")


@unittest.skipUnless(os.name == "nt", "requires actual Windows Restart Manager")
class NativeAttributionTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve() / "version"
        self.root.mkdir()
        self.api = occupancy.WindowsAPI()
        self.a, self.b = self.root / "held-a.bin", self.root / "held-b.bin"
        for path in [self.a, self.b]:
            path.write_bytes(b"fixture")

    def _preserve(self, label, *, source=None, value=None):
        # Fixed ignored test-artifact directory, not caller-controlled production output.
        output = Path(__file__).resolve().parents[1] / "target/windows-resource-calibration"
        output.mkdir(parents=True, exist_ok=True)
        destination = output / f"{label}-{os.getpid()}-{time.time_ns()}"
        if source is not None:
            shutil.copytree(source, destination)
        else:
            destination.with_suffix(".json").write_text(json.dumps(value, indent=2), encoding="utf-8")

    def _open(self, path, sharing=3):
        handle = self.api.open_file(str(path), 0x80000000, sharing, None, 3, 0, None)
        self.assertNotEqual(handle, C.c_void_p(-1).value)
        return handle

    def test_native_two_processes_map_to_separate_files_without_unlocking(self):
        held = self._open(self.a)
        fixture = None
        ready = self.root.parent / "holder-ready"
        script = """import ctypes as C, sys
from pathlib import Path
from windows_occupancy import WindowsAPI
api = WindowsAPI()
held = api.open_file(sys.argv[1], 0x80000000, 3, None, 3, 0, None)
assert held != C.c_void_p(-1).value
try:
    Path(sys.argv[2]).write_text('ready')
    sys.stdin.buffer.read(1)
finally:
    api.close(held)
"""
        try:
            fixture = subprocess.Popen([sys.executable, "-B", "-c", script, str(self.b), str(ready)],
                cwd=Path(__file__).parent, stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            deadline = time.monotonic() + 10
            while not ready.is_file():
                self.assertIsNone(fixture.poll(), "holder fixture exited before readiness")
                self.assertLess(time.monotonic(), deadline, "holder fixture did not become ready")
                time.sleep(0.01)
            output = self.root.parent / "capture"
            report = collector.capture(self.root, output, None)
            self._preserve(output.name, source=output)
            self.assertEqual(report["status"], "collected", report)
            result = json.loads((output / "08-file-attribution.json").read_text())
            pairs = {(v["pid"], v["relative_path"]) for v in result["witnesses"]}
            self.assertIn((os.getpid(), self.a.name), pairs, result)
            self.assertIn((fixture.pid, self.b.name), pairs, result)
            self.assertNotIn((os.getpid(), self.b.name), pairs)
            self.assertNotIn((fixture.pid, self.a.name), pairs)
            self.assertFalse(result["rename_blocker_proven"])
            for path in [self.a, self.b]:
                self.assertEqual(path.read_bytes(), b"fixture")
                with self.assertRaises(OSError):
                    path.rename(path.with_suffix(".renamed"))
        finally:
            self.api.close(held)
            if fixture is not None:
                try:
                    fixture.communicate(b"x", timeout=5)
                except subprocess.TimeoutExpired:
                    fixture.kill()  # Only this test-created holder fixture.
                    fixture.communicate(timeout=5)
        self.a.rename(self.a.with_suffix(".renamed"))
        self.b.rename(self.b.with_suffix(".renamed"))

    def test_native_queries_do_not_accumulate_previously_registered_paths(self):
        held = self._open(self.a)
        try:
            script = """import json,sys
from windows_occupancy import WindowsAPI
api=WindowsAPI()
print(json.dumps([api.resource_users([p]) for p in sys.argv[1:]]))
"""
            run = subprocess.run([sys.executable, "-B", "-c", script, str(self.a), str(self.b)],
                cwd=Path(__file__).parent, capture_output=True, text=True, timeout=10, check=True)
            first, second = json.loads(run.stdout)
            self._preserve("fresh-sessions", value={"fixture_pid": os.getpid(), "first": first, "second": second})
            self.assertEqual(first["status"], "observed", first)
            self.assertEqual(second["status"], "observed", second)
            self.assertTrue(any(p["pid"] == os.getpid() and p["identity_matches_snapshot"] for p in first["applications"]))
            self.assertFalse(any(p["pid"] == os.getpid() and p["identity_matches_snapshot"] for p in second["applications"]))
            self.assertEqual(first["end_session_error"], 0)
            self.assertEqual(second["end_session_error"], 0)
        finally:
            self.api.close(held)

    def test_native_delete_sharing_is_not_misrepresented_as_a_proven_blocker(self):
        held = self._open(self.a, sharing=7)
        try:
            output = self.root.parent / "permissive-capture"
            report = collector.capture(self.root, output, None)
            self._preserve(output.name, source=output)
            self.assertEqual(report["status"], "collected", report)
            result = json.loads((output / "08-file-attribution.json").read_text())
            self.assertFalse(result["rename_blocker_proven"])
            self.assertFalse(result["directory_handles_covered"])
            # Positive or empty RM observations do not establish a sharing-mode conflict.
            self.a.rename(self.a.with_suffix(".renamed"))
            self.assertEqual(self.a.with_suffix(".renamed").read_bytes(), b"fixture")
        finally:
            self.api.close(held)


if __name__ == "__main__":
    unittest.main()
