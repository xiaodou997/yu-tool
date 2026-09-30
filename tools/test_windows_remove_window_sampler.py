from pathlib import Path
import ctypes as C
import json
import os
import subprocess
import sys
import tempfile
import threading
import time
import unittest

import windows_occupancy as occupancy

ROOT = Path(__file__).resolve().parents[1]
SAMPLER = ROOT / "tools" / "windows_remove_window_sampler.py"
CALIBRATION = ROOT / "target" / "windows-remove-window-calibration"


def start_sampler(version: Path, output: Path):
    stop = output.parent / (output.name + ".stop")
    ready = output.parent / (output.name + ".ready")
    process = subprocess.Popen([
        sys.executable, "-B", str(SAMPLER),
        "--version-dir", str(version),
        "--output-dir", str(output),
        "--stop-file", str(stop),
        "--ready-file", str(ready),
        "--interval-ms", "20",
        "--max-ms", "4000",
    ])
    deadline = time.monotonic() + 3
    while not ready.exists():
        if process.poll() is not None:
            raise AssertionError(f"sampler exited before ready: {process.returncode}")
        if time.monotonic() >= deadline:
            process.kill()
            process.wait()
            raise AssertionError("sampler did not become ready")
        time.sleep(0.005)
    return process, stop


def finish_sampler(process, stop: Path, output: Path):
    stop.write_text("stop\n", encoding="utf-8")
    process.wait(timeout=3)
    if process.returncode != 0:
        raise AssertionError(f"sampler failed: {process.returncode}")
    return json.loads((output / "sampling-report.json").read_text(encoding="utf-8"))


def retry_rename(source: Path, destination: Path, budget_ms: int):
    started = time.monotonic()
    attempts = 0
    last = None
    while (time.monotonic() - started) * 1000 < budget_ms:
        attempts += 1
        try:
            source.rename(destination)
            return {
                "status": "success",
                "attempts": attempts,
                "elapsed_ms": round((time.monotonic() - started) * 1000),
            }
        except OSError as error:
            last = error
            time.sleep(0.025)
    return {
        "status": "failure",
        "attempts": attempts,
        "elapsed_ms": round((time.monotonic() - started) * 1000),
        "os_error": getattr(last, "winerror", None),
    }


@unittest.skipUnless(os.name == "nt", "requires real Windows directory sharing")
class RemoveWindowCalibration(unittest.TestCase):
    def setUp(self):
        CALIBRATION.mkdir(parents=True, exist_ok=True)

    def write_receipt(self, name, value):
        path = CALIBRATION / f"{name}-{time.time_ns()}.json"
        path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    def identities_for_me(self, report):
        return [item for item in report["identities"] if item["pid"] == os.getpid()]

    def test_transient_holder_is_seen_before_successful_retry(self):
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary)
            version, renamed = parent / "version", parent / "renamed"
            version.mkdir()
            api = occupancy.WindowsAPI()
            birth = api.process(os.getpid())["created_filetime"]
            held = api.open_file(str(version), 0x80000000, 3, None, 3, 0x02000000, None)
            self.assertNotEqual(held, C.c_void_p(-1).value)
            output = parent / "sampling"
            process, stop = start_sampler(version, output)
            release = threading.Thread(target=lambda: (time.sleep(0.5), api.close(held)))
            release.start()
            removal = retry_rename(version, renamed, 2000)
            release.join(timeout=2)
            report = finish_sampler(process, stop, output)
            mine = self.identities_for_me(report)
            self.assertEqual(removal["status"], "success", removal)
            self.assertGreater(removal["attempts"], 1, removal)
            self.assertEqual(len(mine), 1, report)
            self.assertEqual(mine[0]["created_filetime"], birth)
            self.assertLessEqual(mine[0]["first_seen_ms"], removal["elapsed_ms"])
            receipt = {
                "calibration": "transient_holder_success",
                "remove": removal,
                "holder_pid": os.getpid(),
                "created_filetime": birth,
                "holder_observed": True,
                "rename_blocker_proven": False,
                "root_cause_fixed": False,
                "public_release_ready": False,
            }
            self.write_receipt("transient-success", receipt)
            print(json.dumps(receipt))

    def test_holder_past_two_seconds_is_seen_with_failed_retry(self):
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary)
            version, renamed = parent / "version", parent / "renamed"
            version.mkdir()
            api = occupancy.WindowsAPI()
            birth = api.process(os.getpid())["created_filetime"]
            held = api.open_file(str(version), 0x80000000, 3, None, 3, 0x02000000, None)
            self.assertNotEqual(held, C.c_void_p(-1).value)
            output = parent / "sampling"
            process, stop = start_sampler(version, output)
            try:
                removal = retry_rename(version, renamed, 2050)
                report = finish_sampler(process, stop, output)
                mine = self.identities_for_me(report)
                self.assertEqual(removal["status"], "failure", removal)
                self.assertGreaterEqual(removal["elapsed_ms"], 2000)
                self.assertEqual(len(mine), 1, report)
                self.assertEqual(mine[0]["created_filetime"], birth)
            finally:
                api.close(held)
            receipt = {
                "calibration": "persistent_holder_failure",
                "remove": removal,
                "holder_pid": os.getpid(),
                "created_filetime": birth,
                "holder_observed": True,
                "rename_blocker_proven": False,
                "root_cause_fixed": False,
                "public_release_ready": False,
            }
            self.write_receipt("persistent-failure", receipt)
            print(json.dumps(receipt))

    def test_sibling_holder_is_not_attributed_to_remove_target(self):
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary)
            version, renamed, sibling = parent / "version", parent / "renamed", parent / "version-other"
            version.mkdir()
            sibling.mkdir()
            api = occupancy.WindowsAPI()
            held = api.open_file(str(sibling), 0x80000000, 3, None, 3, 0x02000000, None)
            self.assertNotEqual(held, C.c_void_p(-1).value)
            output = parent / "sampling"
            process, stop = start_sampler(version, output)
            try:
                removal = retry_rename(version, renamed, 500)
                report = finish_sampler(process, stop, output)
                self.assertEqual(removal["status"], "success", removal)
                self.assertEqual(removal["attempts"], 1, removal)
                self.assertFalse(self.identities_for_me(report), report)
                self.assertEqual(api.delete_access_probe(sibling)["os_error"], 32)
            finally:
                api.close(held)
            receipt = {
                "calibration": "sibling_isolation",
                "remove": removal,
                "target_attributed_holder": False,
                "sibling_still_held": True,
                "rename_blocker_proven": False,
                "root_cause_fixed": False,
                "public_release_ready": False,
            }
            self.write_receipt("sibling-isolation", receipt)
            print(json.dumps(receipt))


if __name__ == "__main__":
    unittest.main()
