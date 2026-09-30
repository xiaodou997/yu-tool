#!/usr/bin/env python3
"""Retain finite lifecycle diagnostic runs; never publish or retry failed cases."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]


def write_report(path: Path, report: dict) -> None:
    path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def run_plan(output: Path, repetitions: int, env: dict, source: str, runner=None) -> dict:
    if not 1 <= repetitions <= 20:
        raise ValueError("repetitions must be in 1..20")
    output.mkdir(parents=True, exist_ok=False)
    env = env.copy()
    if os.name == "nt":
        traces, evidence, remove_windows, remove_file_windows = (
            output / "owned-process-traces",
            output / "occupancy",
            output / "remove-window",
            output / "remove-file-window",
        )
        traces.mkdir()
        evidence.mkdir()
        remove_windows.mkdir()
        remove_file_windows.mkdir()
        env.update(
            YU_WINDOWS_LIFECYCLE_TRACE_DIR=str(traces),
            YU_TEST_FORENSICS_DIR=str(evidence),
            YU_TEST_FORENSICS_PYTHON=sys.executable,
            YU_TEST_REMOVE_WINDOW_SAMPLING_DIR=str(remove_windows),
            YU_TEST_REMOVE_FILE_SAMPLING_DIR=str(remove_file_windows),
        )
    runner = runner or subprocess.run
    plan = [
        ("transport", ["cargo", "test", "--locked", "-p", "yu-cli", "--test", "psd", "transport_lifecycle", "--", "--nocapture"], 5 if os.name == "nt" else 4),
        ("managed", ["cargo", "test", "--locked", "-p", "yu-cli", "--test", "psd_managed", "--", "--ignored", "--nocapture"], 3),
    ]
    report = {
        "schema_version": "1", "source_commit": source,
        "os": platform.system(), "arch": platform.machine(),
        "runner_image": env.get("ImageVersion") or env.get("IMAGEVERSION"), "workflow_run": env.get("GITHUB_RUN_ID"),
        "workflow_attempt": env.get("GITHUB_RUN_ATTEMPT"),
        "repetitions_requested": repetitions, "steps": [], "status": "running",
        "public_release_ready": False, "root_cause_fixed": False,
        "failed_case_retries": 0, "historical_failures_superseded": False,
        "forensics_enabled": os.name == "nt", "forensics_timing":"after_failed_command_before_test_root_teardown",
    }
    report_path = output / "report.json"
    write_report(report_path, report)
    for repetition in range(1, repetitions + 1):
        for suite, argv, minimum in plan:
            logfile = output / f"{repetition:02d}-{suite}.log"
            entry = {"repetition": repetition, "suite": suite, "argv": argv, "log": logfile.name, "status": "running"}
            report["steps"].append(entry)
            write_report(report_path, report)
            print(json.dumps({"repetition": repetition, "suite": suite, "event": "begin"}), flush=True)
            started = time.monotonic()
            try:
                with logfile.open("wb") as stream:
                    result = runner(argv, cwd=ROOT, env=env, stdout=stream, stderr=subprocess.STDOUT, timeout=600, check=False)
                entry["exit_code"] = result.returncode
                text = logfile.read_text(encoding="utf-8", errors="replace")
                summaries = re.findall(r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed;", text)
                passed = sum(int(item[1]) for item in summaries)
                failed = sum(int(item[2]) for item in summaries)
                all_ok = bool(summaries) and all(item[0] == "ok" and item[2] == "0" for item in summaries)
                entry["observed_passed_tests"] = passed
                entry["observed_failed_tests"] = failed
                entry["minimum_passed_tests"] = minimum
                entry["status"] = "passed" if result.returncode == 0 and all_ok and passed >= minimum else "failed"
                if result.returncode == 0 and (not summaries or passed < minimum):
                    entry["reason"] = "missing_executed_test_evidence"
                elif result.returncode == 0 and not all_ok:
                    entry["reason"] = "failure_evidence_despite_zero_exit"
            except subprocess.TimeoutExpired:
                entry.update(status="failed", reason="outer_command_timeout", exit_code=None)
            except OSError as error:
                entry.update(status="failed", reason="command_start_or_log_error", error=str(error), exit_code=None)
            entry["elapsed_ms"] = round((time.monotonic() - started) * 1000)
            entry["occupancy_capture_count"] = len(list((output / "occupancy").glob("*/context.json")))
            entry["owned_trace_file_count"] = len(list((output / "owned-process-traces").glob("*.json")))
            entry["remove_window_report_count"] = len(list((output / "remove-window").glob("*/report.json")))
            entry["remove_file_window_report_count"] = len(
                list((output / "remove-file-window").glob("*/sampling/sampling-report.json"))
            )
            write_report(report_path, report)
            print(json.dumps({"repetition": repetition, "suite": suite, "status": entry["status"]}), flush=True)
            if entry["status"] != "passed":
                report.update(status="failed", first_failed_step=len(report["steps"]) - 1)
                write_report(report_path, report)
                return report
    report["status"] = "passed"
    write_report(report_path, report)
    return report


def hash_path(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def verify_candidate(cli: Path, source: str) -> dict:
    metadata = json.loads((cli.parent.parent / "build-info.json").read_text(encoding="utf-8"))
    if not isinstance(metadata, dict) or metadata.get("kind") != "developer_candidate":
        raise ValueError("probe requires extracted developer candidate metadata")
    if metadata.get("source_commit") != source or metadata.get("public_release_ready") is not False:
        raise ValueError("candidate source/readiness does not match diagnostic checkout")
    if metadata.get("binary_sha256") != hash_path(cli):
        raise ValueError("candidate binary does not match build metadata")
    return metadata


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True, help="Explicit extracted developer candidate executable")
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    try:
        inputs = {name: path.resolve(strict=True) for name, path in [("YU_TEST_CLI", args.cli), ("YU_TEST_MANIFEST", args.manifest), ("YU_TEST_PACKAGE", args.archive)]}
        if any(not path.is_file() for path in inputs.values()):
            raise ValueError("all probe inputs must be files")
        source = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True, timeout=30).stdout.strip()
        if not re.fullmatch(r"[0-9a-f]{40}", source):
            raise ValueError("unexpected Git source identity")
        dirty = subprocess.run(["git", "status", "--porcelain", "--untracked-files=normal"], cwd=ROOT, capture_output=True, text=True, check=True, timeout=30).stdout
        if dirty:
            raise ValueError("diagnostic probe requires a clean committed checkout")
        candidate = verify_candidate(inputs["YU_TEST_CLI"], source)
        env = os.environ.copy()
        env.update({name: str(path) for name, path in inputs.items()})
        env["CARGO_TERM_COLOR"] = "never"
        before = {name: hash_path(path) for name, path in inputs.items()}
        report = run_plan(args.output_dir.resolve(), args.repetitions, env, source)
        report["input_sha256"] = before
        report["candidate_source_commit"] = candidate["source_commit"]
        report["inputs_unchanged"] = before == {name: hash_path(path) for name, path in inputs.items()}
        if not report["inputs_unchanged"]:
            report.update(status="failed", reason="probe_inputs_changed")
        write_report(args.output_dir.resolve() / "report.json", report)
        print(json.dumps({"status": report["status"], "steps_observed": len(report["steps"]), "public_release_ready": False}))
        return 0 if report["status"] == "passed" else 1
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(json.dumps({"error": str(error), "public_release_ready": False}), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
