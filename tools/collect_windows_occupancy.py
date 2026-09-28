#!/usr/bin/env python3
"""Collect bounded failure-time evidence before test-root teardown; never unlock resources."""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from windows_occupancy import WindowsAPI, is_reparse, select_files, normalize_windows_path
from windows_resource_attribution import attribute_files

IS_WINDOWS = os.name == "nt"


def save(path: Path, value: dict) -> None:
    with path.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write("\n")


def matching_traces(directory: Path | None, version: Path) -> dict:
    records, errors, seen, limited = [], [], 0, False
    if directory is None:
        return {"status":"not_enabled", "records":[]}
    for file in directory.glob("*.json"):
        seen += 1
        if seen > 4096:
            limited = True
            break
        try:
            info = file.lstat()
            if is_reparse(info) or info.st_size > 16384:
                continue
            record = json.loads(file.read_text(encoding="utf-8"))
            if isinstance(record, dict) and record.get("kind") == "owned_windows_job_snapshot" and normalize_windows_path(record.get("version_directory", "")) == normalize_windows_path(str(version)):
                records.append(record)
        except (OSError, ValueError) as error:
            errors.append({"file":file.name, "error":str(error)[:200]})
    return {"status":"observed", "records":records, "truncated":limited, "errors":errors}


def correlate(users: dict, traces: dict) -> dict:
    observations = []
    for process in users.get("applications", []):
        matches = [record for record in traces.get("records", [])
                   if process.get("identity_matches_snapshot") is True
                   and record.get("child_pid") == process.get("pid")
                   and record.get("child_created_filetime") is not None
                   and record.get("child_created_filetime") == process.get("restart_manager_created_filetime")]
        observations.append({"pid":process["pid"],
            "created_filetime":process.get("restart_manager_created_filetime"),
            "relationship":"recorded_direct_engine_child" if matches else "not_matched_to_recorded_direct_child",
            "matched_invocations":sorted({record["invocation"] for record in matches})})
    return {"observations":observations, "rename_blocker_proven":False,
            "limits":"Unmatched is not proof of an external process: unrecorded descendants, races and query failures remain possible. File use is not proof of the rename-blocking handle."}


def worker(version: Path, output: Path, traces: Path | None) -> int:
    api = WindowsAPI()
    version = version.resolve(strict=True)
    save(output / "01-path-observation.json", {"observed_unix_ns":str(time.time_ns()),
        "version_directory":str(version), "attributes":api.attributes(str(version)),
        "directory_delete_access":api.delete_access_probe(version),
        "parent_delete_access":api.delete_access_probe(version.parent),
        "permission_cause":"not_established; access probes are not an ACL proof"})
    owned = matching_traces(traces, version)
    save(output / "02-owned-traces.json", owned)
    save(output / "03-version-processes.json", api.version_processes(version))
    selection = select_files(version)
    save(output / "04-selection.json", selection)
    users = api.resource_users(selection["files"])
    save(output / "05-resource-users.json", users)
    save(output / "06-correlation.json", correlate(users, owned))
    # Journal before each dispatch; the unchanged outer timeout retains partial work.
    with (output / "07-file-attribution-events.jsonl").open("x", encoding="utf-8") as stream:
        def record(event):
            stream.write(json.dumps(event, sort_keys=True) + "\n")
            stream.flush()
        attribution = attribute_files(version, selection, users, api.resource_users, record)
    save(output / "08-file-attribution.json", attribution)
    return 0


def capture(version: Path, output: Path, traces: Path | None, runner=None) -> dict:
    # The outer directory is reserved before native calls. A hung API cannot erase the trigger.
    output.mkdir(parents=True, exist_ok=False)
    trigger = {"schema_version":"1", "kind":"post_command_pre_teardown_snapshot",
        "observed_unix_ns":str(time.time_ns()), "version_directory":str(version),
        "root_cause_fixed":False, "public_release_ready":False,
        "directory_handle_owner":"unknown", "snapshot_is_atomic":False,
        "limits":"post-failure sampling may miss short-lived holders; no process/ACL mutation"}
    save(output / "trigger.json", trigger)
    if not IS_WINDOWS:
        report = dict(trigger, status="unsupported_platform")
    else:
        argv = [sys.executable, "-B", str(Path(__file__).resolve()), "--worker", "--version-dir", str(version), "--output-dir", str(output)]
        if traces is not None:
            argv += ["--traces-dir", str(traces)]
        started = time.monotonic()
        try:
            with (output / "collector.log").open("xb") as log:
                result = (runner or subprocess.run)(argv, stdout=log, stderr=subprocess.STDOUT, timeout=10, check=False)
            report = dict(trigger, status="collected" if result.returncode == 0 else "collector_failed", exit_code=result.returncode)
        except subprocess.TimeoutExpired:
            # subprocess.run kills/reaps only its own collector worker; no external PID is targeted.
            report = dict(trigger, status="collector_timeout")
        except OSError as error:
            report = dict(trigger, status="collector_start_failed", error=str(error))
        report["elapsed_ms"] = round((time.monotonic() - started) * 1000)
    save(output / "report.json", report)
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version-dir", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--traces-dir", type=Path)
    parser.add_argument("--worker", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    try:
        if is_reparse(args.version_dir.lstat()) or not args.version_dir.is_dir():
            raise ValueError("version directory is absent or a reparse point")
        version = args.version_dir.resolve(strict=True)
        output = args.output_dir.absolute()
        if args.worker:
            return worker(version, output, args.traces_dir)
        report = capture(version, output, args.traces_dir)
        print(json.dumps({"status":report["status"], "root_cause_fixed":False}))
        return 0 if report["status"] == "collected" else 1
    except (OSError, ValueError) as error:
        print(json.dumps({"error":str(error), "root_cause_fixed":False}), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
