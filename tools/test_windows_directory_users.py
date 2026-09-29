from pathlib import Path
import ctypes as C
import json
import os
import struct
import tempfile
import unittest

import collect_windows_occupancy as collector
import windows_directory_users as directory
import windows_occupancy as occupancy

ROOT = Path(__file__).resolve().parents[1]
CALIBRATION = ROOT / "target" / "windows-directory-calibration"


def record_calibration(name: str, value: dict) -> None:
    CALIBRATION.mkdir(parents=True, exist_ok=True)
    with (CALIBRATION / f"{name}.json").open("x", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write("\n")


class DirectoryQueryTests(unittest.TestCase):
    @staticmethod
    def payload(pids, width):
        return struct.pack("<I", len(pids)) + b"\0" * (width - 4) + b"".join(
            struct.pack("<I" if width == 4 else "<Q", pid) for pid in pids)

    def test_native_pid_list_alignment_and_duplicates(self):
        for width in (4, 8):
            data = self.payload([400, 40, 400], width)
            self.assertEqual(directory.parse_pid_buffer(data, len(data), width), [40, 400])
            empty = self.payload([], width)
            self.assertEqual(directory.parse_pid_buffer(empty, len(empty), width), [])

    def test_native_pid_list_rejects_truncation_and_invalid_lengths(self):
        for width in (4, 8):
            data = self.payload([4, 40], width)
            for returned in (0, width - 1, len(data) - 1, len(data) + 1):
                with self.assertRaises(ValueError):
                    directory.parse_pid_buffer(data, returned, width)
            oversized = self.payload([4] * (directory.MAX_USERS + 1), width)
            with self.assertRaises(ValueError):
                directory.parse_pid_buffer(oversized, len(oversized), width)
        with self.assertRaises(ValueError):
            directory.parse_pid_buffer(b"\0" * 16, 16, 16)

    def test_native_pid_list_rejects_invalid_process_ids(self):
        for pids in ([0], [1 << 32]):
            data = self.payload(pids, 8)
            with self.assertRaises(ValueError):
                directory.parse_pid_buffer(data, len(data), 8)

    def test_confirmation_requires_live_process_handle_and_valid_birth(self):
        query = {"status": "observed", "pids": [12]}
        info = {"created_filetime": "100"}
        self.assertTrue(directory.confirmed_user(12, query, info, 258))
        for pid, result, identity, state in [
            (13, query, info, 258), (12, {"status": "query_failed", "pids": [12]}, info, 258),
            (12, query, {}, 258), (12, query, {"created_filetime": "0"}, 258),
            (12, query, info, 0), (12, query, info, 0xFFFFFFFF),
        ]:
            self.assertFalse(directory.confirmed_user(pid, result, identity, state))

    def test_correlation_never_guesses_reused_or_unconfirmed_identity(self):
        traces = {"records": [{"child_pid": 12, "child_created_filetime": "100", "invocation": "first"}]}
        user = {"pid": 12, "identity_confirmed": True, "created_filetime": "101"}
        self.assertFalse(directory.correlate_directory_users({"applications": [user]}, traces)["observations"][0]["matched_invocations"])
        user["created_filetime"] = "100"
        result = directory.correlate_directory_users({"applications": [user]}, traces)
        self.assertEqual(result["observations"][0]["matched_invocations"], ["first"])
        self.assertFalse(result["rename_blocker_proven"])
        user["identity_confirmed"] = False
        self.assertFalse(directory.correlate_directory_users({"applications": [user]}, traces)["observations"][0]["matched_invocations"])

    def capture_users(self, root, output):
        report = collector.capture(root, output, None)
        self.assertEqual(report["status"], "collected", list(output.iterdir()))
        users = json.loads((output / "02a-directory-users.json").read_text(encoding="utf-8"))
        self.assertEqual(users["status"], "observed", users)
        self.assertEqual(users["scope"], "exact_directory_only")
        self.assertNotEqual(users["collector_pid"], os.getpid(), users)
        self.assertFalse(users["rename_blocker_proven"])
        self.assertFalse(users["complete_handle_inventory"])
        self.assertTrue(all(user["pid"] != users["collector_pid"] for user in users["applications"]))
        return users

    @unittest.skipUnless(os.name == "nt", "requires real Windows exact-directory native query")
    def test_known_directory_holder_is_identified_then_disappears_after_own_release(self):
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary)
            root = parent / "version"
            root.mkdir()
            (root / "sentinel").write_bytes(b"unchanged")
            api = occupancy.WindowsAPI()
            birth = api.process(os.getpid())["created_filetime"]
            held = api.open_file(str(root), 0x80000000, 3, None, 3, 0x02000000, None)
            self.assertNotEqual(held, C.c_void_p(-1).value)
            try:
                users = self.capture_users(root, parent / "held")
                mine = [user for user in users["applications"] if user["pid"] == os.getpid()]
                self.assertEqual(len(mine), 1, users)
                self.assertTrue(mine[0]["identity_confirmed"], users)
                self.assertEqual(mine[0]["created_filetime"], birth)
                self.assertEqual(api.delete_access_probe(root)["os_error"], 32)
                with self.assertRaises(OSError):
                    root.rename(parent / "should-not-rename")
                self.assertEqual((root / "sentinel").read_bytes(), b"unchanged")
            finally:
                api.close(held)  # Only the test releases its own handle.
            after = self.capture_users(root, parent / "released")
            self.assertNotIn(os.getpid(), [user["pid"] for user in after["applications"]], after)
            self.assertEqual(api.delete_access_probe(root)["status"], "opened_and_closed")
            root.rename(parent / "renamed")
            receipt = {"calibration": "directory_holder_release", "holder_pid": os.getpid(),
                "created_filetime": birth, "identity_confirmed": True, "released_holder_absent": True,
                "observer_did_not_unlock": True, "public_release_ready": False,
                "root_cause_fixed": False}
            record_calibration("known-holder-release", receipt)
            print(json.dumps(receipt))

    @unittest.skipUnless(os.name == "nt", "requires real Windows directory sharing")
    def test_delete_sharing_holder_is_a_user_but_not_a_rename_blocker(self):
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary)
            root = parent / "version"
            root.mkdir()
            api = occupancy.WindowsAPI()
            held = api.open_file(str(root), 0x80000000, 7, None, 3, 0x02000000, None)
            self.assertNotEqual(held, C.c_void_p(-1).value)
            try:
                users = self.capture_users(root, parent / "capture")
                self.assertTrue(any(user["pid"] == os.getpid() and user["identity_confirmed"] for user in users["applications"]), users)
                self.assertEqual(api.delete_access_probe(root)["status"], "opened_and_closed")
                root.rename(parent / "renamed-while-held")
            finally:
                api.close(held)
            receipt = {"calibration": "delete_sharing_directory_user", "user_found": True,
                "rename_succeeded_while_held": True, "rename_blocker_proven": False,
                "public_release_ready": False, "root_cause_fixed": False}
            record_calibration("delete-sharing-nonblocker", receipt)
            print(json.dumps(receipt))

    @unittest.skipUnless(os.name == "nt", "requires real Windows exact-directory scoping")
    def test_sibling_only_holder_is_not_attributed_to_target_directory(self):
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary)
            root, sibling = parent / "version", parent / "version-other"
            root.mkdir()
            sibling.mkdir()
            api = occupancy.WindowsAPI()
            held = api.open_file(str(sibling), 0x80000000, 3, None, 3, 0x02000000, None)
            self.assertNotEqual(held, C.c_void_p(-1).value)
            try:
                users = self.capture_users(root, parent / "capture")
                self.assertNotIn(os.getpid(), [user["pid"] for user in users["applications"]], users)
                self.assertEqual(api.delete_access_probe(sibling)["os_error"], 32)
            finally:
                api.close(held)
            record_calibration("sibling-isolation", {
                "calibration": "sibling_only_holder",
                "holder_pid": os.getpid(),
                "target_attributed_holder": False,
                "sibling_still_denied_while_held": True,
                "rename_blocker_proven": False,
                "public_release_ready": False,
                "root_cause_fixed": False,
            })


if __name__ == "__main__":
    unittest.main()
