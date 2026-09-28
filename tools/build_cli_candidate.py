#!/usr/bin/env python3
"""Build and smoke-test developer candidate archives. Never tags, signs or publishes."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {
    "x86_64-unknown-linux-gnu": "bin/yu",
    "aarch64-apple-darwin": "bin/yu",
    "x86_64-pc-windows-msvc": "bin/yu.exe",
}
LOCKS = ("Cargo.lock", "packaging/ag-psd-engine/package-lock.json", "rust-toolchain.toml")
MAX_BINARY = 512 * 1024 * 1024
NOTICE = "Developer validation artifact only. Not approved for public release. Project license, third-party notices, signing/notarization and release authorization remain unaccepted. No optional engine is bundled.\n"
BLOCKERS = ["project_license_and_redistribution_review", "third_party_notice_bundle", "platform_signing_policy_and_acceptance", "minimum_os_acceptance", "windows_lifecycle_root_cause", "durable_hosting_and_release_authorization"]


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def json_bytes(value: dict) -> bytes:
    return (json.dumps(value, sort_keys=True, indent=2) + "\n").encode("utf-8")


def command(args: list[str], *, env=None, cwd=ROOT, timeout=120) -> bytes:
    result = subprocess.run(args, cwd=cwd, env=env, capture_output=True, timeout=timeout)
    if result.returncode:
        raise RuntimeError(f"{args[0]} exited {result.returncode}: {result.stderr.decode('utf-8', errors='replace')[-8000:]}")
    return result.stdout


def git(*args: str) -> str:
    return command(["git", *args]).decode("utf-8").strip()


def clean_head() -> str:
    if git("status", "--porcelain", "--untracked-files=normal"):
        raise ValueError("candidate build requires a clean committed worktree (including lockfiles)")
    for name in LOCKS:
        git("ls-files", "--error-unmatch", name)
    return git("rev-parse", "HEAD")


def archive_bytes(binary: bytes, metadata: dict) -> dict[str, bytes]:
    target = metadata["target"]
    if target not in TARGETS or not 0 < len(binary) <= MAX_BINARY:
        raise ValueError("unsupported target or invalid binary size")
    if metadata["binary_sha256"] != digest(binary):
        raise ValueError("binary digest mismatch")
    if metadata.get("public_release_ready") is not False:
        raise ValueError("developer candidates must not assert release readiness")
    return {TARGETS[target]: binary, "build-info.json": json_bytes(metadata), "RELEASE-STATUS.txt": NOTICE.encode()}


def write_archive(path: Path, binary: bytes, metadata: dict) -> None:
    members = archive_bytes(binary, metadata)
    # Exclusive creation: never silently replace another candidate.
    with path.open("xb") as stream, zipfile.ZipFile(stream, "w", compression=zipfile.ZIP_STORED) as archive:
        for name, data in sorted(members.items()):
            info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = (stat.S_IFREG | (0o755 if name.startswith("bin/") else 0o644)) << 16
            archive.writestr(info, data)


def unpack_verified(path: Path, destination: Path) -> tuple[Path, dict]:
    """Validate the fixed candidate format before writing; checksums are not signatures."""
    with zipfile.ZipFile(path) as archive:
        infos = archive.infolist()
        names = [info.filename for info in infos]
        if len(names) != 3 or len(set(names)) != 3 or "build-info.json" not in names:
            raise ValueError("unexpected or duplicate candidate members")
        info = archive.getinfo("build-info.json")
        if info.file_size > 1024 * 1024:
            raise ValueError("oversized build metadata")
        metadata = json.loads(archive.read(info))
        target = metadata.get("target")
        if target not in TARGETS or metadata.get("public_release_ready") is not False:
            raise ValueError("invalid candidate target/readiness")
        executable = TARGETS[target]
        if set(names) != {executable, "build-info.json", "RELEASE-STATUS.txt"}:
            raise ValueError("candidate contains an unexpected path")
        for item in infos:
            mode = item.external_attr >> 16
            limit = MAX_BINARY if item.filename == executable else 1024 * 1024
            if not stat.S_ISREG(mode) or not 0 < item.file_size <= limit or item.flag_bits & 1:
                raise ValueError("candidate member must be bounded, regular and unencrypted")
        binary = archive.read(executable)
        if digest(binary) != metadata.get("binary_sha256"):
            raise ValueError("candidate binary checksum mismatch")
        if archive.read("RELEASE-STATUS.txt") != NOTICE.encode():
            raise ValueError("candidate notice mismatch")
        destination.mkdir(parents=True, exist_ok=False)
        binary_path = destination / executable
        binary_path.parent.mkdir()
        binary_path.write_bytes(binary)
        if os.name != "nt":
            binary_path.chmod(0o755)
        (destination / "build-info.json").write_bytes(json_bytes(metadata))
        (destination / "RELEASE-STATUS.txt").write_text(NOTICE, encoding="utf-8")
    return binary_path.resolve(), metadata


def smoke(binary: Path, version: str, cwd: Path) -> None:
    env = os.environ.copy()
    env.update(PATH="", YU_DATA_HOME=str(cwd / "isolated-data"), NODE_OPTIONS="--require=must-not-load", NODE_PATH="must-not-load")
    actual = command([str(binary), "--version"], env=env, cwd=cwd).decode().strip()
    if actual != f"yu {version}":
        raise ValueError(f"wrong candidate version: {actual!r}")
    doctor = json.loads(command([str(binary), "doctor", "--json"], env=env, cwd=cwd))
    caps = json.loads(command([str(binary), "capabilities", "--json"], env=env, cwd=cwd))
    if doctor["result"]["engines"]["built_in"] != 2:
        raise ValueError("built-in engine smoke failed")
    ids = {item["id"] for item in caps["result"]}
    if not {"image.info", "image.resize"} <= ids or any(item.startswith("psd.") for item in ids):
        raise ValueError("optional engines must not be required or auto-activated")


def build(target: str, output: Path) -> dict:
    head = clean_head()
    if output.exists():
        raise ValueError("candidate output directory already exists")
    config = tomllib.loads((ROOT / "Cargo.toml").read_text())
    version = config["workspace"]["package"]["version"]
    toolchain = tomllib.loads((ROOT / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    compiler = command(["rustc", "--version"]).decode().strip()
    if not compiler.startswith(f"rustc {toolchain} "):
        raise ValueError("active compiler does not match pinned toolchain")
    build_dir = ROOT / "target" / "candidate-build"
    # Always build from this source; do not label a caller-supplied/stale binary with HEAD.
    subprocess.run(["cargo", "build", "--locked", "--release", "--package", "yu-cli", "--bin", "yu", "--target", target, "--target-dir", str(build_dir)], cwd=ROOT, check=True, timeout=1200, stdout=sys.stderr)
    if clean_head() != head:
        raise ValueError("source changed while building candidate")
    binary_path = build_dir / target / "release" / Path(TARGETS[target]).name
    binary = binary_path.read_bytes()
    metadata = {
        "schema_version": "1", "kind": "developer_candidate", "version": version,
        "target": target, "source_commit": head, "source_tree": git("rev-parse", "HEAD^{tree}"),
        "rustc": compiler, "binary_sha256": digest(binary),
        "input_sha256": {name: digest((ROOT / name).read_bytes()) for name in LOCKS},
        "public_release_ready": False, "unaccepted_release_gates": BLOCKERS,
        "whole_application_bit_reproducibility_proven": False,
        "signing_notarization_accepted": False,
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".yu-candidate-", dir=output.parent) as temporary:
        stage = Path(temporary)
        name = f"yu-{version}-dev.{head[:12]}-{target}.zip"
        archive = stage / name
        write_archive(archive, binary, metadata)
        executable, _ = unpack_verified(archive, stage / "smoke")
        smoke(executable, version, stage)
        receipt = {"schema_version":"1", "archive":name, "sha256":digest(archive.read_bytes()), "bytes":archive.stat().st_size, "source_commit":head, "public_release_ready":False, "packaged_cli_smoke":"passed", "smoke_binary":f"smoke/{TARGETS[target]}"}
        (stage / "SHA256SUMS").write_text(f"{receipt['sha256']}  {name}\n", encoding="utf-8")
        (stage / "candidate.json").write_bytes(json_bytes(receipt))
        # Reserve the destination first; never clobber an existing candidate.
        output.mkdir(exist_ok=False)
        for item in stage.iterdir():
            shutil.move(str(item), str(output / item.name))
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, choices=TARGETS)
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args()
    try:
        print(json.dumps(build(args.target, args.output_dir.resolve()), sort_keys=True))
        return 0
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError, zipfile.BadZipFile) as error:
        print(json.dumps({"schema_version":"1", "error":str(error), "public_release_ready":False}), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
