"""Read-only Windows resource observations. No shutdown, unlock, privilege or ACL changes."""
from __future__ import annotations

import ctypes as C
from ctypes import wintypes as W
import os
import ntpath
from pathlib import Path
import stat

MAX_FILES = 64
MAX_ENTRIES = 512
MAX_PROCESSES = 4096


def is_reparse(info: os.stat_result) -> bool:
    return stat.S_ISLNK(info.st_mode) or bool(getattr(info, "st_file_attributes", 0) & 0x400)


def select_files(root: Path) -> dict:
    """Bounded walk of only this version; never follow junctions or read payload contents."""
    if is_reparse(root.lstat()) or not root.is_dir():
        raise ValueError("version directory must be an existing non-reparse directory")
    root = root.resolve(strict=True)
    selected, skipped, errors = [], 0, []
    stack = [(root, 0)]
    seen = 0
    limited = False
    # Prioritize runtime/metadata before optional node_modules; coverage remains partial.
    priority = {".yu-install.json": 0, "runtime": 1, "engine": 2}
    while stack and len(selected) < MAX_FILES and seen < MAX_ENTRIES:
        directory, depth = stack.pop()
        children = []
        try:
            with os.scandir(directory) as entries:
                for entry in entries:
                    seen += 1
                    if seen > MAX_ENTRIES:
                        limited = True
                        break
                    try:
                        info = entry.stat(follow_symlinks=False)
                        if is_reparse(info):
                            skipped += 1
                        elif stat.S_ISREG(info.st_mode):
                            if len(selected) < MAX_FILES:
                                selected.append(str(Path(entry.path)))
                            else:
                                limited = True
                        elif stat.S_ISDIR(info.st_mode):
                            if depth < 5:
                                children.append(Path(entry.path))
                            else:
                                limited = True
                    except OSError as error:
                        errors.append({"path":str(Path(entry.path).relative_to(root)), "errno":error.errno})
        except OSError as error:
            errors.append({"path":str(directory.relative_to(root)), "errno":error.errno})
        stack.extend((path, depth + 1) for path in sorted(children, key=lambda p: (priority.get(p.name, 3), p.name), reverse=True))
    return {"files":selected, "entries_observed":seen, "reparse_points_skipped":skipped,
            "truncated":limited or bool(stack), "errors":errors,
            "coverage":"bounded_regular_files_only; directory-only handles are not covered"}


def normalize_windows_path(path: str) -> str:
    if path.startswith("\\\\?\\UNC\\"):
        path = "\\\\" + path[8:]
    elif path.startswith("\\\\?\\"):
        path = path[4:]
    return ntpath.normcase(ntpath.normpath(path))


def inside(image: str, root: Path) -> bool:
    # Windows-native paths are compared component-wise, not by vulnerable string prefix.
    try:
        return ntpath.commonpath([normalize_windows_path(image), normalize_windows_path(str(root))]) == normalize_windows_path(str(root))
    except ValueError:
        return False


def ticks(value: W.FILETIME) -> str:
    return str((value.dwHighDateTime << 32) | value.dwLowDateTime)


class WindowsAPI:
    def __init__(self):
        if os.name != "nt":
            raise OSError("Windows resource observation is unsupported on this platform")
        self.kernel = C.WinDLL("kernel32.dll", use_last_error=True, winmode=0x800)
        self.rm = C.WinDLL("rstrtmgr.dll", use_last_error=True, winmode=0x800)  # System32 only.
        self.open_process = self.bind(self.kernel, "OpenProcess", [W.DWORD, W.BOOL, W.DWORD], W.HANDLE)
        self.close = self.bind(self.kernel, "CloseHandle", [W.HANDLE], W.BOOL)
        self.image = self.bind(self.kernel, "QueryFullProcessImageNameW", [W.HANDLE, W.DWORD, W.LPWSTR, C.POINTER(W.DWORD)], W.BOOL)
        self.times = self.bind(self.kernel, "GetProcessTimes", [W.HANDLE] + [C.POINTER(W.FILETIME)] * 4, W.BOOL)
        self.enum = self.bind(self.kernel, "K32EnumProcesses", [C.POINTER(W.DWORD), W.DWORD, C.POINTER(W.DWORD)], W.BOOL)
        self.open_file = self.bind(self.kernel, "CreateFileW", [W.LPCWSTR, W.DWORD, W.DWORD, C.c_void_p, W.DWORD, W.DWORD, W.HANDLE], W.HANDLE)
        self.attributes = self.bind(self.kernel, "GetFileAttributesW", [W.LPCWSTR], W.DWORD)

    @staticmethod
    def bind(dll, name, args, result):
        function = getattr(dll, name)
        function.argtypes, function.restype = args, result
        return function

    def process(self, pid: int) -> dict:
        result = {"pid":pid}
        handle = self.open_process(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION only.
        if not handle:
            return dict(result, status="unavailable", os_error=C.get_last_error())
        try:
            created, exited, kernel, user = (W.FILETIME() for _ in range(4))
            if self.times(handle, C.byref(created), C.byref(exited), C.byref(kernel), C.byref(user)):
                result["created_filetime"] = ticks(created)
                result["exit_filetime"] = ticks(exited)
            else:
                result["times_error"] = C.get_last_error()
            buffer, length = C.create_unicode_buffer(32768), W.DWORD(32768)
            if self.image(handle, 0, buffer, C.byref(length)):
                result.update(status="observed", image=buffer.value)
            else:
                result.update(status="image_unavailable", os_error=C.get_last_error())
        finally:
            self.close(handle)
        return result

    def version_processes(self, root: Path) -> dict:
        ids, size = (W.DWORD * MAX_PROCESSES)(), W.DWORD()
        if not self.enum(ids, C.sizeof(ids), C.byref(size)):
            return {"status":"query_failed", "os_error":C.get_last_error()}
        matches, inaccessible, omitted_matches = [], 0, 0
        count = min(size.value // C.sizeof(W.DWORD), MAX_PROCESSES)
        for pid in ids[:count]:
            info = self.process(pid)
            if "image" not in info:
                inaccessible += 1
            elif inside(info["image"], root):
                if len(matches) < 128:
                    matches.append(info)
                else:
                    omitted_matches += 1
        return {"status":"observed", "matches":matches, "pids_observed":count,
                "unavailable_image_count":inaccessible, "omitted_matches":omitted_matches, "truncated":size.value >= C.sizeof(ids) or omitted_matches > 0,
                "coverage":"only matching executable paths; absence does not exclude DLL or directory holders"}

    def delete_access_probe(self, path: Path) -> dict:
        # Request DELETE access but NEVER delete/rename/set disposition. Share every access.
        # OPEN_EXISTING + BACKUP_SEMANTICS + OPEN_REPARSE_POINT, no delete-on-close flag.
        handle = self.open_file(str(path), 0x10000, 0x7, None, 3, 0x02200000, None)
        if handle == C.c_void_p(-1).value:
            return {"status":"open_failed", "os_error":C.get_last_error()}
        self.close(handle)
        return {"status":"opened_and_closed", "delete_performed":False}

    def resource_users(self, files: list[str]) -> dict:
        class UniqueProcess(C.Structure):
            _fields_ = [("pid", W.DWORD), ("created", W.FILETIME)]
        class ProcessInfo(C.Structure):
            _fields_ = [("process", UniqueProcess), ("app_name", W.WCHAR * 256),
                        ("service", W.WCHAR * 64), ("type", C.c_int), ("status", W.ULONG),
                        ("session", W.DWORD), ("restartable", W.BOOL)]
        if C.sizeof(ProcessInfo) != 668:
            raise ValueError("unexpected Restart Manager structure layout")
        start = self.bind(self.rm, "RmStartSession", [C.POINTER(W.DWORD), W.DWORD, W.LPWSTR], W.DWORD)
        end = self.bind(self.rm, "RmEndSession", [W.DWORD], W.DWORD)
        register = self.bind(self.rm, "RmRegisterResources", [W.DWORD, W.UINT, C.POINTER(W.LPCWSTR), W.UINT, C.c_void_p, W.UINT, C.c_void_p], W.DWORD)
        get_list = self.bind(self.rm, "RmGetList", [W.DWORD, C.POINTER(W.UINT), C.POINTER(W.UINT), C.POINTER(ProcessInfo), C.POINTER(W.DWORD)], W.DWORD)
        result = {"coverage":"registered_regular_files_only", "directory_handles_covered":False,
                  "applications":[], "complete_handle_inventory":False}
        if not files:
            return dict(result, status="no_files_selected")
        session, key = W.DWORD(), C.create_unicode_buffer(33)
        code = start(C.byref(session), 0, key)
        if code:
            return dict(result, status="start_failed", os_error=code)
        try:
            paths = (W.LPCWSTR * len(files))(*files)
            code = register(session, len(files), paths, 0, None, 0, None)
            if code:
                result.update(status="register_failed", os_error=code)
                return result
            needed, count, reboot = W.UINT(), W.UINT(128), W.DWORD()
            items = (ProcessInfo * 128)()
            code = get_list(session, C.byref(needed), C.byref(count), items, C.byref(reboot))
            result.update(os_error=code, needed=needed.value, reboot_reasons=reboot.value)
            if code:
                # No retry that could make a changing snapshot look complete.
                result["status"] = "truncated" if code == 234 else "query_failed"
                return result
            for item in items[:min(count.value, 128)]:
                info = self.process(item.process.pid)
                original_birth = ticks(item.process.created)
                info["restart_manager_created_filetime"] = original_birth
                info["identity_matches_snapshot"] = info.get("created_filetime") == original_birth
                info["resource_relationship"] = "reported_user_of_at_least_one_registered_file_not_proven_rename_blocker"
                result["applications"].append(info)
            result["status"] = "observed"
            return result
        finally:
            result["end_session_error"] = end(session)
