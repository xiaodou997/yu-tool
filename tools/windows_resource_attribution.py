"""Bounded, post-failure single-file witnesses; never a rename-blocker diagnosis.

Only refine identities already reported by the initial batch query. Each native query
uses WindowsAPI.resource_users' fresh Restart Manager session, never accumulating paths.
Query scheduling is bounded; the existing outer collector supervises blocking APIs.
"""
from __future__ import annotations

from pathlib import Path
import stat
import time
from typing import Callable

from windows_occupancy import MAX_FILES, is_reparse

MAX_QUERIES = 16
MAX_IDENTITIES = 16
SCHEDULING_SECONDS = 3.0


def identity(process: object) -> tuple[int, str] | None:
    if not isinstance(process, dict):
        return None
    pid = process.get("pid")
    birth = process.get("restart_manager_created_filetime")
    if (type(pid) is not int or not 0 < pid <= 0xFFFFFFFF
            or not isinstance(birth, str) or not 1 <= len(birth) <= 20
            or not birth.isascii() or not birth.isdigit()
            or not 0 < int(birth) <= 0xFFFFFFFFFFFFFFFF
            or process.get("created_filetime") != birth
            or process.get("identity_matches_snapshot") is not True
            or process.get("status") != "observed"
            or process.get("exit_filetime") != "0"):
        return None
    return pid, birth


def _snapshot_identities(snapshot: object) -> set[tuple[int, str]] | None:
    if (not isinstance(snapshot, dict) or snapshot.get("status") != "observed"
            or snapshot.get("os_error") != 0 or snapshot.get("end_session_error") != 0):
        return None
    applications = snapshot.get("applications")
    if not isinstance(applications, list) or len(applications) > 128:
        return None
    return {key for process in applications if (key := identity(process)) is not None}


def _processes(keys: set[tuple[int, str]]) -> list[dict]:
    return [{"pid": pid, "created_filetime": birth} for pid, birth in sorted(keys)]


def _validate_paths(root: Path, files: list[str]) -> None:
    """Metadata only. Recheck every component before querying; no reparse traversal."""
    info = root.lstat()
    if not root.is_absolute() or is_reparse(info) or not stat.S_ISDIR(info.st_mode):
        raise ValueError("invalid version directory")
    for value in files:
        if not isinstance(value, str) or "\0" in value or len(value) > 32767:
            raise ValueError("invalid selected path")
        path = Path(value)
        relative = path.relative_to(root)
        if (not path.is_absolute() or not relative.parts or len(relative.parts) > 6
                or any(part in (".", "..") for part in relative.parts)):
            raise ValueError("selected file is not within the bounded version tree")
        current = root
        for index, part in enumerate(relative.parts):
            current = current / part
            info = current.lstat()
            expected_type = stat.S_ISREG if index == len(relative.parts) - 1 else stat.S_ISDIR
            if is_reparse(info) or not expected_type(info.st_mode):
                raise ValueError("selected file or ancestor is not a regular non-reparse path")


def attribute_files(
    root: Path,
    selection: dict,
    baseline: dict,
    query: Callable[[list[str]], dict],
    record: Callable[[dict], None] = lambda _: None,
    *,
    max_queries: int = MAX_QUERIES,
    scheduling_seconds: float = SCHEDULING_SECONDS,
    clock: Callable[[], float] = time.monotonic,
) -> dict:
    """Seek one positive singleton witness per verified baseline identity, not all handles.

    Empty/changing/error results only stop refinement of that partition. They never prove
    absence, infer the complement's user, or establish a directory-rename blocker.
    The callback journals starts AND completions so an outer timeout retains partial work.
    """
    if (type(max_queries) is not int or not 1 <= max_queries <= MAX_QUERIES
            or not 0 < scheduling_seconds <= SCHEDULING_SECONDS):
        raise ValueError("attribution budget exceeds fixed limits")
    started = clock()
    result = {
        "schema_version": "1", "kind": "single_file_resource_witnesses",
        "snapshot_is_atomic": False, "directory_handles_covered": False,
        "rename_blocker_proven": False, "root_cause_fixed": False,
        "coverage": "at_most_one_file_witness_per_verified_batch_identity; not_a_handle_inventory",
        "max_queries": max_queries, "scheduling_seconds": scheduling_seconds,
        "native_calls_interruptible": False, "queries_started": 0,
        "queries_completed": 0, "query_failures": 0, "path_failures": 0,
        "witnesses": [], "selection_truncated": bool(selection.get("truncated", False)),
    }
    record(dict(result, event="begin", observed_unix_ns=str(time.time_ns())))
    keys = _snapshot_identities(baseline)
    if keys is None:
        result.update(status="baseline_unavailable", unresolved_identities=[])
        return result
    targets = set(sorted(keys)[:MAX_IDENTITIES])
    result["baseline_verified_identities"] = _processes(targets)
    result["baseline_targets_truncated"] = len(keys) > MAX_IDENTITIES
    files = selection.get("files")
    try:
        if (not isinstance(files, list) or not 1 <= len(files) <= MAX_FILES
                or not all(isinstance(path, str) for path in files)
                or len(set(files)) != len(files)):
            raise ValueError("invalid file selection")
        _validate_paths(root, files)
    except (OSError, ValueError):
        result.update(status="invalid_selection", unresolved_identities=_processes(targets))
        return result
    if not targets:
        result.update(status="no_verified_targets", unresolved_identities=[])
        return result
    found: set[tuple[int, str]] = set()
    pending: list[tuple[list[str], set[tuple[int, str]]]] = []

    def schedule(paths: list[str], candidates: set[tuple[int, str]]) -> None:
        if len(paths) == 1:
            pending.append((paths, candidates))
        else:
            middle = len(paths) // 2
            pending.extend([(paths[middle:], candidates), (paths[:middle], candidates)])

    schedule(files, targets)
    stop = "partitions_exhausted"
    while pending and targets - found:
        if result["queries_started"] >= max_queries:
            stop = "query_budget_exhausted"
            break
        if clock() - started >= scheduling_seconds:
            stop = "scheduling_budget_exhausted"
            break
        paths, candidates = pending.pop()
        candidates = candidates - found
        if not candidates:
            continue
        relative = [str(Path(path).relative_to(root)) for path in paths]
        try:
            _validate_paths(root, paths)
        except (OSError, ValueError):
            result["path_failures"] += 1
            record({"event": "paths_unavailable", "paths": relative,
                    "observed_unix_ns": str(time.time_ns())})
            continue
        result["queries_started"] += 1
        number = result["queries_started"]
        began = clock()
        record({"event": "query_started", "query": number, "paths": relative,
                "candidate_identities": _processes(candidates),
                "observed_unix_ns": str(time.time_ns())})
        try:
            snapshot = query(paths)
        except (OSError, ValueError) as error:
            snapshot = {"status": "query_exception", "exception_type": type(error).__name__}
        matches = _snapshot_identities(snapshot)
        result["queries_completed"] += 1
        observed = str(time.time_ns())
        matched = (matches or set()) & candidates
        record({"event": "query_completed", "query": number, "paths": relative,
                "snapshot": snapshot, "matched_identities": _processes(matched),
                "elapsed_ms": round((clock() - began) * 1000), "observed_unix_ns": observed})
        if matches is None:
            result["query_failures"] += 1
            # Do not create more sessions after failing to end one.
            if isinstance(snapshot, dict) and snapshot.get("end_session_error", 0) != 0:
                stop = "session_cleanup_failed"
                break
            continue
        if len(paths) == 1:
            for pid, birth in sorted(matched):
                result["witnesses"].append({
                    "relative_path": relative[0], "pid": pid, "created_filetime": birth,
                    "query": number, "observed_unix_ns": observed,
                    "relationship": "same_process_reported_for_this_single_registered_file",
                    "rename_blocker_proven": False,
                })
            found.update(matched)
        elif matched:
            schedule(paths, matched)
    if targets == found:
        stop = "verified_targets_localized"
    result.update(status=stop, unresolved_identities=_processes(targets - found),
                  elapsed_ms=round((clock() - started) * 1000))
    record(dict(result, event="end", observed_unix_ns=str(time.time_ns())))
    return result
