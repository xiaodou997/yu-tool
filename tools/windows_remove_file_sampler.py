#!/usr/bin/env python3
"""Sample regular-file Restart Manager users while a test-owned remove command is in flight."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import sys
import time

from windows_occupancy import WindowsAPI, is_reparse, select_files
from windows_resource_attribution import attribute_files, identity

DEFAULT_INTERVAL_MS = 50
DEFAULT_MAX_MS = 5000
MAX_SAMPLES = 128
MAX_ATTRIBUTION_QUERIES = 12
ATTRIBUTION_SECONDS = 0.75


def write_new(path: Path, value: dict) -> None:
    with path.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write("\n")


def verified_identities(snapshot: dict) -> list[dict]:
    result = []
    for process in snapshot.get("applications", []):
        key = identity(process)
        if key is None:
            continue
        result.append({
            "pid": key[0],
            "created_filetime": key[1],
            "image": process.get("image"),
        })
    return result


def summarize(events: list[dict]) -> tuple[list[dict], list[dict]]:
    identities: dict[tuple[int, str], dict] = {}
    witnesses: dict[tuple[int, str, str], dict] = {}
    for event in events:
        elapsed = event["elapsed_ms"]
        for process in event.get("verified_identities", []):
            key = (process["pid"], process["created_filetime"])
            current = identities.setdefault(key, {
                "pid": key[0],
                "created_filetime": key[1],
                "image": process.get("image"),
                "first_seen_ms": elapsed,
                "last_seen_ms": elapsed,
                "sample_count": 0,
                "identity_confirmed": True,
                "relationship": "registered_file_user_not_proven_rename_blocker",
            })
            current["last_seen_ms"] = elapsed
            current["sample_count"] += 1
            if current.get("image") is None and process.get("image") is not None:
                current["image"] = process["image"]
        attribution = event.get("attribution")
        if not isinstance(attribution, dict):
            continue
        for witness in attribution.get("witnesses", []):
            path = witness.get("relative_path")
            pid = witness.get("pid")
            birth = witness.get("created_filetime")
            if not isinstance(path, str) or not isinstance(pid, int) or not isinstance(birth, str):
                continue
            key = (pid, birth, path)
            current = witnesses.setdefault(key, {
                "pid": pid,
                "created_filetime": birth,
                "relative_path": path,
                "first_seen_ms": elapsed,
                "last_seen_ms": elapsed,
                "sample_count": 0,
                "relationship": "same_process_reported_for_single_registered_file_not_proven_rename_blocker",
                "rename_blocker_proven": False,
            })
            current["last_seen_ms"] = elapsed
            current["sample_count"] += 1
    return (
        sorted(identities.values(), key=lambda item: (item["first_seen_ms"], item["pid"])),
        sorted(witnesses.values(), key=lambda item: (item["first_seen_ms"], item["pid"], item["relative_path"])),
    )


def sample_window(
    version: Path,
    output: Path,
    stop_file: Path,
    ready_file: Path,
    interval_ms: int = DEFAULT_INTERVAL_MS,
    max_ms: int = DEFAULT_MAX_MS,
) -> dict:
    if not 10 <= interval_ms <= 1000:
        raise ValueError("interval-ms must be in 10..1000")
    if not 100 <= max_ms <= 10000:
        raise ValueError("max-ms must be in 100..10000")
    output.mkdir(parents=True, exist_ok=False)
    api = WindowsAPI()
    selection = select_files(version)
    files = selection.get("files", [])
    if not files:
        raise ValueError("version directory contains no bounded regular-file selection")

    started = time.monotonic()
    started_ns = time.time_ns()
    events: list[dict] = []
    stop_reason = "max_window"
    journal = output / "samples.jsonl"
    attribution_journal = output / "attribution-events.jsonl"
    with journal.open("x", encoding="utf-8") as stream, attribution_journal.open("x", encoding="utf-8") as details:
        for index in range(MAX_SAMPLES):
            cycle_started = time.monotonic()
            elapsed_ms = round((cycle_started - started) * 1000)
            if elapsed_ms >= max_ms:
                stop_reason = "max_window"
                break
            if stop_file.exists() and index > 0:
                stop_reason = "remove_finished"
                break
            if not version.exists():
                event = {
                    "sample": index,
                    "elapsed_ms": elapsed_ms,
                    "status": "target_absent",
                    "verified_identities": [],
                }
                events.append(event)
                stream.write(json.dumps(event, sort_keys=True) + "\n")
                stream.flush()
                if index == 0:
                    ready_file.write_text("ready\n", encoding="utf-8")
                stop_reason = "target_absent"
                break

            baseline = api.resource_users(files)
            verified = verified_identities(baseline)
            event = {
                "sample": index,
                "elapsed_ms": elapsed_ms,
                "status": baseline.get("status"),
                "verified_identities": verified,
                "registered_file_count": len(files),
                "rename_blocker_proven": False,
            }
            if verified:
                def record(value: dict) -> None:
                    details.write(json.dumps(dict(value, sample=index), sort_keys=True) + "\n")
                    details.flush()
                event["attribution"] = attribute_files(
                    version,
                    selection,
                    baseline,
                    api.resource_users,
                    record,
                    max_queries=MAX_ATTRIBUTION_QUERIES,
                    scheduling_seconds=ATTRIBUTION_SECONDS,
                )
            events.append(event)
            stream.write(json.dumps(event, sort_keys=True) + "\n")
            stream.flush()
            if index == 0:
                ready_file.write_text("ready\n", encoding="utf-8")

            spent_ms = (time.monotonic() - cycle_started) * 1000
            if spent_ms < interval_ms:
                time.sleep((interval_ms - spent_ms) / 1000)
        else:
            stop_reason = "sample_limit"

    identities, witnesses = summarize(events)
    report = {
        "schema_version": "1",
        "kind": "remove_window_regular_file_sampling",
        "version_directory": str(version),
        "sampler_pid": os.getpid(),
        "started_unix_ns": str(started_ns),
        "elapsed_ms": round((time.monotonic() - started) * 1000),
        "interval_ms": interval_ms,
        "max_ms": max_ms,
        "sample_count": len(events),
        "stop_reason": stop_reason,
        "selection": {
            "file_count": len(files),
            "entries_observed": selection.get("entries_observed"),
            "truncated": selection.get("truncated"),
            "reparse_points_skipped": selection.get("reparse_points_skipped"),
            "errors": selection.get("errors"),
        },
        "identities": identities,
        "witnesses": witnesses,
        "samples_with_verified_users": sum(bool(event.get("verified_identities")) for event in events),
        "samples_with_single_file_witnesses": sum(
            bool(event.get("attribution", {}).get("witnesses")) for event in events
        ),
        "rename_blocker_proven": False,
        "complete_handle_inventory": False,
        "root_cause_fixed": False,
        "public_release_ready": False,
        "limits": (
            "Restart Manager sampling covers only the bounded selected regular files and perturbs timing. "
            "A reported file user, including a singleton witness, does not expose handle share mode or prove rename causality."
        ),
    }
    write_new(output / "sampling-report.json", report)
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version-dir", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--stop-file", required=True, type=Path)
    parser.add_argument("--ready-file", required=True, type=Path)
    parser.add_argument("--interval-ms", type=int, default=DEFAULT_INTERVAL_MS)
    parser.add_argument("--max-ms", type=int, default=DEFAULT_MAX_MS)
    args = parser.parse_args()
    try:
        if os.name != "nt":
            raise RuntimeError("remove-file-window sampling requires Windows")
        if is_reparse(args.version_dir.lstat()) or not args.version_dir.is_dir():
            raise ValueError("version directory is absent or a reparse point")
        sample_window(
            args.version_dir.resolve(strict=True),
            args.output_dir.absolute(),
            args.stop_file.absolute(),
            args.ready_file.absolute(),
            args.interval_ms,
            args.max_ms,
        )
        return 0
    except (OSError, ValueError, RuntimeError) as error:
        print(json.dumps({"error": str(error), "root_cause_fixed": False}), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
