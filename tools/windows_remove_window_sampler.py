#!/usr/bin/env python3
"""Sample exact-directory users only while a test-owned remove command is in flight."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import sys
import time

from windows_directory_users import directory_users
from windows_occupancy import WindowsAPI, is_reparse

DEFAULT_INTERVAL_MS = 25
DEFAULT_MAX_MS = 5000
MAX_SAMPLES = 256


def write_new(path: Path, value: dict) -> None:
    with path.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write("\n")


def identity_key(user: dict) -> tuple[int, str] | None:
    if user.get("identity_confirmed") is not True:
        return None
    pid = user.get("pid")
    created = user.get("created_filetime")
    if not isinstance(pid, int) or pid <= 0 or not isinstance(created, str) or not created.isdigit():
        return None
    return pid, created


def summarize(events: list[dict]) -> list[dict]:
    identities: dict[tuple[int, str], dict] = {}
    for event in events:
        elapsed = event["elapsed_ms"]
        for user in event.get("applications", []):
            key = identity_key(user)
            if key is None:
                continue
            current = identities.setdefault(key, {
                "pid": key[0],
                "created_filetime": key[1],
                "image": user.get("image"),
                "first_seen_ms": elapsed,
                "last_seen_ms": elapsed,
                "sample_count": 0,
                "identity_confirmed": True,
                "relationship": "exact_directory_user_not_proven_rename_blocker",
            })
            current["last_seen_ms"] = elapsed
            current["sample_count"] += 1
            if current.get("image") is None and user.get("image") is not None:
                current["image"] = user["image"]
    return sorted(identities.values(), key=lambda item: (item["first_seen_ms"], item["pid"]))


def sample_window(
    version: Path,
    output: Path,
    stop_file: Path,
    ready_file: Path,
    interval_ms: int = DEFAULT_INTERVAL_MS,
    max_ms: int = DEFAULT_MAX_MS,
) -> dict:
    if not 5 <= interval_ms <= 1000:
        raise ValueError("interval-ms must be in 5..1000")
    if not 100 <= max_ms <= 10000:
        raise ValueError("max-ms must be in 100..10000")
    output.mkdir(parents=True, exist_ok=False)
    started = time.monotonic()
    started_ns = time.time_ns()
    events: list[dict] = []
    stop_reason = "max_window"
    api = WindowsAPI() if os.name == "nt" else None
    journal = output / "samples.jsonl"
    with journal.open("x", encoding="utf-8") as stream:
        for index in range(MAX_SAMPLES):
            elapsed_ms = round((time.monotonic() - started) * 1000)
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
                    "applications": [],
                }
                events.append(event)
                stream.write(json.dumps(event, sort_keys=True) + "\n")
                stream.flush()
                if index == 0:
                    ready_file.write_text("ready\n", encoding="utf-8")
                stop_reason = "target_absent"
                break
            users = directory_users(version, api)
            event = {
                "sample": index,
                "elapsed_ms": elapsed_ms,
                "status": users.get("status"),
                "applications": users.get("applications", []),
                "confirmed_user_count": users.get("confirmed_user_count"),
                "directory_users_attributed": users.get("directory_users_attributed"),
                "rename_blocker_proven": False,
            }
            events.append(event)
            stream.write(json.dumps(event, sort_keys=True) + "\n")
            stream.flush()
            if index == 0:
                ready_file.write_text("ready\n", encoding="utf-8")
            remaining_ms = interval_ms - ((time.monotonic() - started) * 1000 - elapsed_ms)
            if remaining_ms > 0:
                time.sleep(remaining_ms / 1000)
        else:
            stop_reason = "sample_limit"
    report = {
        "schema_version": "1",
        "kind": "remove_window_directory_sampling",
        "version_directory": str(version),
        "sampler_pid": os.getpid(),
        "started_unix_ns": str(started_ns),
        "elapsed_ms": round((time.monotonic() - started) * 1000),
        "interval_ms": interval_ms,
        "max_ms": max_ms,
        "sample_count": len(events),
        "stop_reason": stop_reason,
        "identities": summarize(events),
        "query_status_counts": {
            status: sum(1 for event in events if event.get("status") == status)
            for status in sorted({str(event.get("status")) for event in events})
        },
        "rename_blocker_proven": False,
        "complete_handle_inventory": False,
        "root_cause_fixed": False,
        "public_release_ready": False,
        "limits": (
            "Sampling perturbs timing and may miss users between samples. "
            "Exact-directory use does not reveal individual handle/share mode or prove rename causality."
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
            raise RuntimeError("remove-window sampling requires Windows")
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
