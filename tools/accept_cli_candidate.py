#!/usr/bin/env python3
"""Exercise packaged CLI with a fresh user directory and no development tools in PATH.

This is process-environment isolation on the current host, NOT a clean VM, network
sandbox, minimum-OS certification, legal approval or public release authorization.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import zipfile
import zlib

from build_cli_candidate import unpack_verified
from distribution_notices import engine_notice_inventory


HOSTS = {("Linux", "x86_64"): "x86_64-unknown-linux-gnu", ("Darwin", "arm64"): "aarch64-apple-darwin",
         ("Windows", "AMD64"): "x86_64-pc-windows-msvc", ("Windows", "x86_64"): "x86_64-pc-windows-msvc"}


def hash_file(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def load_json(path: Path) -> dict:
    if not path.is_file() or path.stat().st_size > 2 * 1024 * 1024:
        raise ValueError("metadata must be a bounded file")
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError("metadata must be an object")
    return value


def isolated_environment(root: Path, parent: dict[str, str]) -> dict[str, str]:
    keep = {"SYSTEMROOT", "WINDIR", "SYSTEMDRIVE", "COMSPEC", "PATHEXT"}
    env = {k: v for k, v in parent.items() if k.upper() in keep}
    home, temporary = root / "home", root / "temp"
    for directory in (home, temporary, home / "appdata", home / "local", home / "config", home / "cache", home / "data"):
        directory.mkdir(parents=True, exist_ok=True)
    env.update(PATH="", HOME=str(home), USERPROFILE=str(home), APPDATA=str(home / "appdata"),
               LOCALAPPDATA=str(home / "local"), XDG_CONFIG_HOME=str(home / "config"), XDG_CACHE_HOME=str(home / "cache"),
               XDG_DATA_HOME=str(home / "data"), TMP=str(temporary), TEMP=str(temporary), TMPDIR=str(temporary),
               YU_DATA_HOME=str(root / "yu-data"), NODE_OPTIONS="--require=must-not-load", NODE_PATH="must-not-load")
    return env


def tiny_png() -> bytes:
    def chunk(kind: bytes, payload: bytes) -> bytes:
        return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", zlib.crc32(kind + payload))
    pixels = b"\x00" + bytes([255, 0, 0, 255, 0, 255, 0, 255])
    pixels += b"\x00" + bytes([0, 0, 255, 255, 255, 255, 255, 255])
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 2, 2, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(pixels)) + chunk(b"IEND", b"")


def candidate_archive(directory: Path) -> Path:
    receipt = load_json(directory / "candidate.json")
    name = receipt.get("archive")
    if not isinstance(name, str) or not name or "/" in name or "\\" in name or name in (".", ".."):
        raise ValueError("candidate receipt must name a single archive")
    path = directory / name
    if receipt.get("public_release_ready") is not False or hash_file(path) != receipt.get("sha256"):
        raise ValueError("candidate receipt integrity/readiness mismatch")
    return path


def accept(candidate_dir: Path, manifest: Path, engine: Path, fixture: Path, output: Path, expected_source: str) -> dict:
    output.mkdir(parents=True, exist_ok=False)
    report = {"schema_version": "1", "kind": "isolated_candidate_acceptance", "status": "running", "steps": [],
              "public_release_ready": False, "root_cause_fixed": False, "clean_vm": False,
              "network_isolated": False, "environment": "fresh HOME/YU_DATA_HOME, empty PATH, outside checkout cwd",
              "runtime_trace_enabled": False, "failed_case_retries": 0, "host": {"os": platform.system(), "arch": platform.machine()}}
    inputs: dict[str, Path] = {}
    before: dict[str, str] = {}
    try:
        if not re.fullmatch(r"[0-9a-f]{40}", expected_source):
            raise ValueError("expected source must be an exact commit SHA")
        archive = candidate_archive(candidate_dir)
        inputs = {"candidate": archive, "manifest": manifest, "engine": engine, "fixture": fixture}
        before = {name: hash_file(path) for name, path in inputs.items()}
        manifest_data = load_json(manifest)
        if manifest_data.get("id") != "ag-psd":
            raise ValueError("this acceptance requires the Managed ag-psd engine")
        report["engine_notices"] = engine_notice_inventory(engine)
        if report["engine_notices"]["engine_version"] != manifest_data["version"]:
            raise ValueError("engine archive and manifest identities differ")
        with tempfile.TemporaryDirectory(prefix="yu-isolated-acceptance-") as temporary:
            root = Path(temporary).resolve()
            binary, build = unpack_verified(archive, root / "application")
            host = HOSTS.get((platform.system(), platform.machine()))
            if build.get("target") != host or build.get("source_commit") != expected_source or build.get("archive_layout_version") != 2:
                raise ValueError("candidate target/source/layout does not match acceptance")
            report.update(source_commit=build["source_commit"], target=build["target"], binary_sha256=hash_file(binary))
            inventory = load_json(root / "application/dependency-inventory.json")
            report["notice_components"] = len(inventory["components"])
            report["missing_notice_evidence"] = inventory["missing_notice_evidence"]
            env = isolated_environment(root, os.environ)
            cwd = root / "working directory"
            cwd.mkdir()
            png = cwd / "input.png"
            png.write_bytes(tiny_png())
            psd = cwd / "design.psd"
            shutil.copyfile(fixture, psd)
            source_hashes = {str(p): hash_file(p) for p in (png, psd)}
            local_manifest, local_engine = cwd / "manifest.json", cwd / "engine.zip"
            shutil.copyfile(manifest, local_manifest)
            shutil.copyfile(engine, local_engine)

            def call(label: str, args: list[str], exit_code: int = 0, error: str | None = None, as_json: bool = True):
                entry = {"name": label, "expected_exit": exit_code, "status": "running"}
                report["steps"].append(entry)
                began = time.monotonic()
                result = subprocess.run([str(binary), *args], env=env, cwd=cwd, capture_output=True, timeout=60, check=False)
                stem = f"{len(report['steps']):02d}-{label}"
                (output / f"{stem}.stdout").write_bytes(result.stdout)
                (output / f"{stem}.stderr").write_bytes(result.stderr)
                entry.update(exit_code=result.returncode, elapsed_ms=round((time.monotonic() - began) * 1000))
                if result.returncode != exit_code:
                    entry["status"] = "failed"
                    raise ValueError(f"{label}: unexpected exit {result.returncode}; original output retained")
                if not as_json:
                    value = result.stdout.decode().strip()
                else:
                    value = json.loads(result.stderr if error else result.stdout)
                    if value.get("schema_version") != "1" or (error and (result.stdout or value.get("error", {}).get("code") != error)):
                        raise ValueError(f"{label}: invalid result envelope")
                if as_json and not error and args[0] == "psd":
                    selected = value.get("engine", {})
                    if selected.get("id") != "ag-psd" or selected.get("version") != manifest_data["version"]:
                        raise ValueError(f"{label}: selected engine identity mismatch")
                entry["status"] = "passed"
                return value

            if call("version", ["--version"], as_json=False) != f"yu {build['version']}":
                raise ValueError("packaged version mismatch")
            call("doctor", ["doctor", "--json"])
            caps = call("initial-capabilities", ["capabilities", "--json"])
            if any(v["id"].startswith("psd.") for v in caps["result"]):
                raise ValueError("fresh environment unexpectedly has an active PSD engine")
            image = call("image-info", ["image", "info", str(png), "--json"])
            if (image["result"]["width"], image["result"]["height"]) != (2, 2):
                raise ValueError("built-in image inspection mismatch")
            resized = cwd / "resized.png"
            call("image-resize", ["image", "resize", str(png), "--width", "1", "-o", str(resized), "--json"])
            image = call("resized-info", ["image", "info", str(resized), "--json"])
            if (image["result"]["width"], image["result"]["height"]) != (1, 1):
                raise ValueError("built-in resize dimensions mismatch")
            call("psd-not-installed", ["psd", "inspect", str(psd), "--json"], 3, "ENGINE_UNAVAILABLE")
            call("offline-install", ["engine", "install", "--manifest", str(local_manifest), "--archive", str(local_engine), "--json"])
            call("psd-not-activated", ["psd", "inspect", str(psd), "--json"], 3, "ENGINE_UNAVAILABLE")
            version = manifest_data["version"]
            call("activate", ["engine", "activate", "ag-psd", version, "--json"])
            call("psd-inspect", ["psd", "inspect", str(psd), "--json"])
            call("psd-tree", ["psd", "tree", str(psd), "--json"])
            layers = call("layer-list", ["psd", "layer", "list", str(psd), "--json"])["result"]["layers"]
            if not layers:
                raise ValueError("acceptance fixture must contain a bitmap layer")
            layer = layers[0]["id"]
            call("layer-info", ["psd", "layer", "info", str(psd), "--id", layer, "--json"])
            exported = cwd / "layer.png"
            args = ["psd", "layer", "export", str(psd), "--id", layer, "-o", str(exported), "--json"]
            exported_result = call("layer-export", args)
            if exported_result["result"]["pixel_format"] != "rgba8" or exported.read_bytes()[:8] != b"\x89PNG\r\n\x1a\n":
                raise ValueError("expected stored-layer RGBA8 PNG")
            call("exported-image-info", ["image", "info", str(exported), "--json"])
            saved = hash_file(exported)
            call("no-clobber", args, 2, "OUTPUT_CONFLICT")
            if hash_file(exported) != saved:
                raise ValueError("conflicting export overwrote the existing file")
            call("deactivate", ["engine", "deactivate", "ag-psd", "--json"])
            call("psd-deactivated", ["psd", "inspect", str(psd), "--json"], 3, "ENGINE_UNAVAILABLE")
            removed = call("remove", ["engine", "remove", "ag-psd", version, "--json"])
            if removed["result"]["cleanup_complete"] is not True or (root / "yu-data/engines/ag-psd" / version).exists():
                raise ValueError("explicit removal did not complete")
            if any(hash_file(Path(name)) != value for name, value in source_hashes.items()):
                raise ValueError("acceptance source input changed")
            if hash_file(local_engine) != before["engine"] or hash_file(local_manifest) != before["manifest"]:
                raise ValueError("offline installation inputs changed")
            report.update(status="passed", source_files_unchanged=True, no_clobber=True, removal_complete=True)
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError, zipfile.BadZipFile) as error:
        report.update(status="failed", error=str(error))
        for step in report["steps"]:
            if step["status"] == "running":
                step["status"] = "failed"
    finally:
        report["input_sha256"] = before
        try:
            report["inputs_unchanged"] = bool(before) and all(hash_file(inputs[name]) == value for name, value in before.items())
        except OSError:
            report["inputs_unchanged"] = False
        if not report["inputs_unchanged"]:
            report["status"] = "failed"
        (output / "report.json").write_text(json.dumps(report, sort_keys=True, indent=2) + "\n", encoding="utf-8")
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("candidate-dir", "manifest", "engine-archive", "fixture", "output-dir"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--expected-source", required=True)
    args = parser.parse_args()
    try:
        result = accept(args.candidate_dir.resolve(), args.manifest.resolve(), args.engine_archive.resolve(), args.fixture.resolve(), args.output_dir.resolve(), args.expected_source)
        print(json.dumps({"status": result["status"], "steps": len(result["steps"]), "error": result.get("error"), "public_release_ready": False}))
        return 0 if result["status"] == "passed" else 1
    except OSError as error:
        print(json.dumps({"status": "failed", "error": str(error)}), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
