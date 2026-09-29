"""Experimental, exact-directory native query. Never duplicate/close other processes' handles.

FileProcessIdsUsingFileInformation (47) is reserved for system use by Microsoft.
Keep this in the bounded diagnostic worker, not the CLI's execution/removal path.
Returned directory users are NOT proven rename blockers or a complete handle inventory.
"""
from __future__ import annotations

import ctypes as C
from ctypes import wintypes as W
import os
from pathlib import Path
import struct
import time
from contextlib import ExitStack

from windows_occupancy import WindowsAPI, is_reparse, ticks

MAX_USERS = 128
FILE_PROCESS_IDS_USING_FILE_INFORMATION = 47
QUERY_LIMITED_AND_SYNCHRONIZE = 0x00101000
WAIT_TIMEOUT = 258


class IOStatusValue(C.Union):
    _fields_ = [("status", C.c_int32), ("pointer", C.c_void_p)]


class IOStatusBlock(C.Structure):
    _fields_ = [("value", IOStatusValue), ("information", C.c_size_t)]


class FileTime(C.Structure):
    _fields_ = [("low", C.c_uint32), ("high", C.c_uint32)]


class FileInformation(C.Structure):
    _fields_ = [("attributes", C.c_uint32), ("created", FileTime),
                ("accessed", FileTime), ("written", FileTime),
                ("volume_serial", C.c_uint32), ("size_high", C.c_uint32),
                ("size_low", C.c_uint32), ("links", C.c_uint32),
                ("index_high", C.c_uint32), ("index_low", C.c_uint32)]


def parse_pid_buffer(data: bytes, returned: int, pointer_bytes: int) -> list[int]:
    """Validate ABI-dependent alignment and every bound before interpreting a native result."""
    if pointer_bytes not in (4, 8):
        raise ValueError("unsupported pointer size")
    offset = pointer_bytes  # ULONG followed by an aligned ULONG_PTR array.
    if not offset <= returned <= len(data):
        raise ValueError("invalid returned byte count")
    count = struct.unpack_from("<I", data)[0]
    if count > MAX_USERS or offset + count * pointer_bytes > returned:
        raise ValueError("truncated or oversized directory PID list")
    code = "<I" if pointer_bytes == 4 else "<Q"
    pids = [struct.unpack_from(code, data, offset + index * pointer_bytes)[0]
            for index in range(count)]
    if any(pid <= 0 or pid > 0xFFFFFFFF for pid in pids):
        raise ValueError("invalid Windows process ID")
    return sorted(set(pids))


def confirmed_user(pid: int, confirmation: dict, info: dict, wait_result: int) -> bool:
    return (confirmation.get("status") == "observed"
            and pid in confirmation.get("pids", []) and wait_result == WAIT_TIMEOUT
            and isinstance(info.get("created_filetime"), str)
            and info["created_filetime"].isdigit() and int(info["created_filetime"]) > 0)


class DirectoryAPI:
    def __init__(self, api: WindowsAPI):
        self.api = api
        self.ntdll = C.WinDLL("ntdll.dll", use_last_error=True, winmode=0x800)
        self.query = api.bind(self.ntdll, "NtQueryInformationFile",
            [W.HANDLE, C.POINTER(IOStatusBlock), C.c_void_p, C.c_uint32, C.c_int32], C.c_int32)
        self.file_info = api.bind(api.kernel, "GetFileInformationByHandle",
            [W.HANDLE, C.POINTER(FileInformation)], W.BOOL)
        self.wait = api.bind(api.kernel, "WaitForSingleObject", [W.HANDLE, W.DWORD], W.DWORD)
        if C.sizeof(FileInformation) != 52 or C.sizeof(IOStatusBlock) != 2 * C.sizeof(C.c_void_p):
            raise ValueError("unexpected Windows structure layout")

    def query_pids(self, handle) -> dict:
        pointer_bytes = C.sizeof(C.c_void_p)
        # Pointer-aligned backing storage; native ULONG is always 32-bit, even on 64-bit Windows.
        buffer = (C.c_size_t * (MAX_USERS + 1))()
        status_block = IOStatusBlock()
        started = str(time.time_ns())
        code = self.query(handle, C.byref(status_block), buffer, C.sizeof(buffer),
                          FILE_PROCESS_IDS_USING_FILE_INFORMATION) & 0xFFFFFFFF
        result = {"ntstatus": f"0x{code:08x}", "observed_unix_ns": started,
                  "returned_bytes": status_block.information, "capacity_pids": MAX_USERS}
        if code != 0:
            # Do not grow/retry a changing snapshot or pretend an unsupported class is empty.
            return dict(result, status="truncated" if code in (0x80000005, 0xC0000004, 0xC0000023)
                        else "query_failed", pids=[])
        try:
            pids = parse_pid_buffer(bytes(buffer), status_block.information, pointer_bytes)
        except ValueError as error:
            return dict(result, status="invalid_result", error=str(error), pids=[])
        return dict(result, status="observed", pids=pids)

    def metadata(self, handle) -> dict:
        info = FileInformation()
        if not self.file_info(handle, C.byref(info)):
            raise OSError(C.get_last_error(), "cannot inspect opened directory")
        if not info.attributes & 0x10 or info.attributes & 0x400:
            raise ValueError("opened target is not a non-reparse directory")
        return {"volume_serial": info.volume_serial,
                "file_index": str((info.index_high << 32) | info.index_low),
                "attributes": info.attributes}

    def process_info(self, pid: int, handle) -> dict:
        info = {"pid": pid}
        created, exited, kernel, user = (W.FILETIME() for _ in range(4))
        if self.api.times(handle, C.byref(created), C.byref(exited), C.byref(kernel), C.byref(user)):
            info["created_filetime"] = ticks(created)
        else:
            info["times_error"] = C.get_last_error()
        # Identity and image come from the SAME handle held across the confirmation query.
        name, length = C.create_unicode_buffer(32768), W.DWORD(32768)
        if self.api.image(handle, 0, name, C.byref(length)):
            info["image"] = name.value
        else:
            info["image_error"] = C.get_last_error()
        return info


def directory_users(version: Path, api: WindowsAPI | None = None) -> dict:
    result = {"schema_version": "1", "method": "NtQueryInformationFile/FileProcessIdsUsingFileInformation",
              "api_support": "system_reserved_experimental", "scope": "exact_directory_only",
              "version_directory": str(version), "applications": [],
              "collector_pid": os.getpid(), "observer_excluded": True,
              "complete_handle_inventory": False, "rename_blocker_proven": False,
              "root_cause_fixed": False, "snapshot_is_atomic": False,
              "limits": "No per-handle share mode or handle value. No subtree, kernel-filter or ACL attribution. Empty, denied, unsupported or changing results do not prove absence of occupancy."}
    if os.name != "nt":
        return dict(result, status="unsupported_platform")
    try:
        if is_reparse(version.lstat()) or not version.is_dir():
            return dict(result, status="invalid_target")
        api = api or WindowsAPI()
        native = DirectoryAPI(api)
        with ExitStack() as resources:
            # Metadata-only access, all sharing flags, non-inheritable existing directory.
            # No DELETE access, DELETE_ON_CLOSE, privilege adjustment or data read/write.
            handle = api.open_file(str(version), 0x80, 7, None, 3, 0x02200000, None)
            if handle == C.c_void_p(-1).value:
                return dict(result, status="open_failed", os_error=C.get_last_error())
            resources.callback(api.close, handle)
            result["directory_identity"] = native.metadata(handle)
            initial = native.query_pids(handle)
            result["initial_query"] = initial
            if initial["status"] != "observed":
                return dict(result, status=initial["status"])
            handles = {}
            for pid in initial["pids"]:
                if pid == os.getpid():
                    continue
                process = api.open_process(QUERY_LIMITED_AND_SYNCHRONIZE, False, pid)
                if not process:
                    result["applications"].append({"pid": pid, "identity_confirmed": False,
                        "status": "process_unavailable", "os_error": C.get_last_error()})
                else:
                    resources.callback(api.close, process)
                    handles[pid] = process
            # Deliberate second observation with live identity handles, NOT a retry of failure.
            confirmation = native.query_pids(handle)
            result["confirmation_query"] = confirmation
            result["new_pids_not_identity_confirmed"] = sorted(set(confirmation.get("pids", []))
                                                               - set(initial["pids"]) - {os.getpid()})
            for pid, process in handles.items():
                info = native.process_info(pid, process)
                state = native.wait(process, 0)
                info.update(process_wait_result=state,
                    identity_confirmed=confirmed_user(pid, confirmation, info, state),
                    relationship="reported_directory_user_not_proven_rename_blocker")
                result["applications"].append(info)
            result["confirmed_user_count"] = sum(item["identity_confirmed"] for item in result["applications"])
            result["status"] = "observed" if confirmation["status"] == "observed" else "confirmation_failed"
            result["directory_users_attributed"] = result["confirmed_user_count"] > 0
            return result
    except (OSError, ValueError, AttributeError) as error:
        return dict(result, status="unavailable", error=str(error)[:500])


def correlate_directory_users(users: dict, traces: dict) -> dict:
    observations = []
    for user in users.get("applications", []):
        matches = [record for record in traces.get("records", [])
                   if user.get("identity_confirmed") is True
                   and record.get("child_pid") == user.get("pid")
                   and user.get("created_filetime") is not None
                   and record.get("child_created_filetime") == user["created_filetime"]]
        observations.append({"pid": user["pid"], "created_filetime": user.get("created_filetime"),
            "identity_confirmed": user.get("identity_confirmed", False),
            "relationship": "recorded_direct_engine_child" if matches else "not_matched_to_recorded_direct_child",
            "matched_invocations": sorted({record["invocation"] for record in matches})})
    return {"observations": observations, "rename_blocker_proven": False,
            "limits": "Unmatched is not necessarily external; only confirmed PID plus creation time can match an owned direct child."}
