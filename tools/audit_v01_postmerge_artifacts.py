#!/usr/bin/env python3
"""Offline v0.1 post-PR42 exact-main CLI/Managed ZIP evidence auditor.

No downloads, extraction, signing, publication or release flag changes.
Only reports archive/metadata identity and presence, NOT legal clearance.
Run on already downloaded GitHub Actions artifacts for main@5526dc4.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import zipfile

MAIN = "5526dc4465c4e79ad7f2757d06b3b1693642b8af"
TREE = "6851265265d383bea9f1470b64635c3041790158"
ENGINE = "31.0.2+node22.23.3.yu2"
TARGETS = {
    "linux": "x86_64-unknown-linux-gnu",
    "macos": "aarch64-apple-darwin",
    "windows": "x86_64-pc-windows-msvc",
}
ENGINE_NAMES = {"node", "ag-psd", "base64-js", "pako"}
PACKAGE_COUNTS = {"linux": 169, "macos": 169, "windows": 166}


def digest(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def source_blob(source_repo: Path, filename: str) -> bytes:
    result = subprocess.run(
        ["git", "show", f"{MAIN}:{filename}"], cwd=source_repo,
        capture_output=True, timeout=15, check=True,
    )
    return result.stdout


def safe_archive(file: Path, maximum_mb: int = 240) -> zipfile.ZipFile:
    require(file.is_file() and not file.is_symlink(), f"archive missing/unsafe: {file}")
    require(0 < file.stat().st_size <= maximum_mb * 1024 * 1024, "archive size exceeded")
    z = zipfile.ZipFile(file)
    names = z.namelist()
    require(len(names) < 12000 and len(names) == len(set(names)), "duplicate/huge ZIP index")
    for n in names:
        parts = Path(n).parts
        require(not n.startswith("/") and ".." not in parts and "\\" not in n, "unsafe ZIP path")
    return z


def read(z: zipfile.ZipFile, name: str, bound: int = 4_000_000) -> bytes:
    info = z.getinfo(name)
    require(not info.is_dir() and not info.flag_bits & 1 and info.file_size <= bound,
            f"encrypted/unbounded ZIP member {name}")
    raw = z.read(name)
    require(len(raw) == info.file_size, f"ZIP member length mismatch: {name}")
    return raw


def cli(folder: Path, platform: str, repo: Path) -> dict:
    entries = list(folder.glob("*.zip"))
    require(len(entries) == 1, f"{platform}: expected exactly one CLI archive")
    archive = entries[0]
    manifest = json.loads((folder / "candidate.json").read_text())
    sha_line = (folder / "SHA256SUMS").read_text().strip()
    require(manifest["archive"] == archive.name, "candidate archive name mismatch")
    require(manifest["source_commit"] == MAIN, "candidate commit mismatch")
    require(manifest["public_release_ready"] is False, "candidate wrongly promoted")
    require(manifest["packaged_cli_smoke"] == "passed", "packaged candidate smoke not passed")
    require(manifest["sha256"] == digest(archive.read_bytes()), "candidate archive SHA mismatch")
    require(manifest["bytes"] == archive.stat().st_size, "candidate size mismatch")
    require(sha_line == f"{manifest['sha256']}  {archive.name}", "SHA256SUMS mismatch")

    expected_members = {
        "LICENSE-APACHE", "LICENSE-MIT", "RELEASE-STATUS.txt",
        "THIRD-PARTY-NOTICES.txt", "USAGE.md",
        "bin/yu.exe" if platform == "windows" else "bin/yu",
        "build-info.json", "dependency-inventory.json",
    }
    with safe_archive(archive) as z:
        require(set(z.namelist()) == expected_members, "unexpected candidate layout")
        info = json.loads(read(z, "build-info.json"))
        inventory = json.loads(read(z, "dependency-inventory.json"))
        require(info["archive_layout_version"] == 3, "candidate is not layout v3")
        require(info["source_commit"] == MAIN and info["source_tree"] == TREE,
                "candidate source SHA/tree differs")
        require(info["target"] == TARGETS[platform], "candidate target mismatch")
        require(info["project_license"] == "MIT OR Apache-2.0", "project license mismatch")
        for key in ["public_release_ready", "redistribution_review_accepted",
                    "signing_notarization_accepted"]:
            require(info[key] is False, f"{key} wrongly accepted")
        binary = "bin/yu.exe" if platform == "windows" else "bin/yu"
        require(digest(read(z, binary, 35_000_000)) == info["binary_sha256"],
                "candidate binary integrity failure")
        for name, hash_expected in info["auxiliary_sha256"].items():
            require(name in expected_members, "unexpected auxiliary name")
            require(digest(read(z, name)) == hash_expected,
                    f"{name} auxiliary integrity failure")
        require(len(info["auxiliary_sha256"]) == 5, "five auxiliary hashes required")
        # Git checkouts under Windows may normalize LF -> CRLF. Text
        # meaning is equal after newline normalization, and each ZIP's
        # exact bytes must also match its declared SHA above.
        license_lf = {}
        for name in ["LICENSE-MIT", "LICENSE-APACHE"]:
            package = read(z, name).replace(b"\r\n", b"\n")
            baseline = source_blob(repo, name).replace(b"\r\n", b"\n")
            license_lf[name] = package == baseline
            require(license_lf[name], f"project {name} text mismatch")
        require(inventory["source_commit"] == MAIN, "inventory source mismatch")
        require(inventory["target"] == TARGETS[platform], "inventory target mismatch")
        require(inventory["missing_notice_evidence"] == [], "missing notice evidence")
        require(len(inventory["components"]) == PACKAGE_COUNTS[platform],
                "unexpected dependency count")
        require(inventory["public_release_ready"] is False and
                inventory["redistribution_review_accepted"] is False,
                "dependency inventory overclaims acceptance")
        require(all(x["declared_license"] == "MIT OR Apache-2.0"
                    for x in inventory["project_components"]), "workspace license mismatch")
        require(len(inventory["project_components"]) >= 7, "project dependency list incomplete")
        return {
            "target": TARGETS[platform],
            "candidate_archive_name": archive.name,
            "archive_sha256": manifest["sha256"],
            "candidate_bytes": archive.stat().st_size,
            "archive_layout": 3, "source_commit": MAIN,
            "source_tree": TREE, "binary_and_all_auxiliary_hashes_match": True,
            "project_license_texts_match_frozen_main_normalized_newlines": license_lf,
            "resolved_third_party_components": len(inventory["components"]),
            "missing_notice_evidence": 0,
            "independent_legal_redistribution_acceptance": False,
            "public_release_ready": False,
        }


def managed(folder: Path, platform: str) -> dict:
    archives = list(folder.glob("*.zip"))
    require(len(archives) == 1, f"{platform}: exactly one Managed package expected")
    archive = archives[0]
    metas = list(folder.glob("metadata-*.json"))
    require(len(metas) == 1, f"{platform}: missing Managed metadata")
    meta = json.loads(metas[0].read_text())
    require(meta["sha256"] == digest(archive.read_bytes()), "engine archive SHA mismatch")
    require(meta["bytes"] == archive.stat().st_size, "engine size mismatch")
    require(meta["engine_version"] == ENGINE and meta["engine_id"] == "ag-psd",
            "engine version/identity mismatch")
    license_paths = {
        "node": "licenses/node-LICENSE",
        "ag-psd": "engine/node_modules/ag-psd/LICENSE",
        "base64-js": "engine/node_modules/base64-js/LICENSE",
        "pako": "engine/node_modules/pako/LICENSE",
    }
    with safe_archive(archive) as z:
        license_sizes = {component: len(read(z, name, 300_000))
                         for component, name in license_paths.items()}
        packages = {
            "ag-psd": ("31.0.2", "MIT"),
            "base64-js": ("1.5.1", "MIT"),
            "pako": ("2.1.0", "(MIT AND Zlib)"),
        }
        for pkg, (version, license_expr) in packages.items():
            data = json.loads(read(z, f"engine/node_modules/{pkg}/package.json"))
            require(data["version"] == version and data["license"] == license_expr,
                    f"managed dependency {pkg} version/license mismatch")
        require(meta["url"].startswith("https://example.invalid/"),
                "expected prototype engine distribution URL changed; audit policy needed")
        return {
            "engine_version": ENGINE, "archive_sha256":meta["sha256"],
            "archive_bytes":archive.stat().st_size,
            "notice_components":list(license_sizes),
            "notice_lengths":license_sizes,
            "placeholder_url_in_prototype_manifest":True,
            "legal_redistribution_acceptance":False,
            "public_release_ready":False,
        }


def audit(root: Path, repository: Path) -> dict:
    require(root.is_dir() and not root.is_symlink(), "missing artifacts root")
    actual_tree = subprocess.run(["git","rev-parse",f"{MAIN}^{{tree}}"],
                                 cwd=repository,capture_output=True,text=True,
                                 timeout=15,check=True).stdout.strip()
    require(actual_tree == TREE, "frozen source tree mismatch")
    candidates={p:cli(root / p,p,repository) for p in TARGETS}
    engines={p:managed(root / f"managed-{p}",p) for p in TARGETS}
    ci_isolated={}
    for p in TARGETS:
        f=root/f"acceptance-{p}"/"report.json"
        record=json.loads(f.read_text())
        require(record["source_commit"]==MAIN and record["target"]==TARGETS[p],
                f"{p} CI isolated acceptance source mismatch")
        require(record["status"]=="passed" and len(record["steps"])==20 and
                all(x["status"]=="passed" for x in record["steps"]),
                f"{p} CI isolated acceptance did not pass 20 steps")
        require(record["input_sha256"]["candidate"]==
                candidates[p]["archive_sha256"],f"{p} CI candidate artifact differs")
        require(record["input_sha256"]["engine"]==
                engines[p]["archive_sha256"],f"{p} CI engine artifact differs")
        require(record["failed_case_retries"]==0 and
                record["missing_notice_evidence"]==[] and
                record["public_release_ready"] is False,
                f"{p} isolated CI retries/missing notices or promotion")
        ci_isolated[p]={
            "CI_isolated_acceptance":"PASS",
            "steps":20,"source_commit":MAIN,
            "candidate_sha256":candidates[p]["archive_sha256"],
            "engine_sha256":engines[p]["archive_sha256"],
            "clean_vm":record["clean_vm"],
            "failed_case_retries":record["failed_case_retries"],
        }
    isolated = json.loads((root / "isolated-macos-recheck" / "report.json").read_text())
    require(isolated["status"]=="passed" and len(isolated["steps"])==20,
            "downloaded candidate isolated CLI acceptance must complete 20 steps")
    require(isolated["source_commit"]==MAIN and isolated["public_release_ready"] is False,
            "isolated candidate source or readiness mismatch")
    require(isolated["input_sha256"]["candidate"]==
            candidates["macos"]["archive_sha256"], "isolated Mac candidate mismatches")
    require(isolated["input_sha256"]["engine"]==
            engines["macos"]["archive_sha256"], "isolated Mac Managed engine mismatches")
    require(isolated["failed_case_retries"]==0 and isolated["missing_notice_evidence"]==[],
            "isolated Mac acceptance had retries or notice missing")
    return {
        "schema_version":"1","scope":"v0.1 post-PR42 exact-main technical artifact inspection",
        "frozen_main_sha":MAIN,"frozen_main_tree":TREE,
        "cli_candidate":candidates,"managed_engine":engines,
        "CI_three_platform_isolated_acceptance":ci_isolated,
        "local_mac_downloaded_candidate_reacceptance":{
            "steps":20,"status":"PASS","source_commit":MAIN,
            "candidate_sha256":candidates["macos"]["archive_sha256"],
            "engine_sha256":engines["macos"]["archive_sha256"],
            "clean_vm":False,"network_isolated":False,
        },
        "technical_artifacts_audit":"PASS",
        # External owner acceptance is in Issue #41 and cannot be
        # authenticated by an offline ZIP audit. Candidate flags
        # intentionally remain false; do not confuse those with Gate 2.
        "external_owner_gate_2_recorded_state":"ACCEPTED_IN_ISSUE_41",
        "external_owner_gate_2_receipt_url":
            "https://github.com/xiaodou997/yu-tool/issues/41#issuecomment-5998812116",
        "external_owner_receipt_authenticated_by_offline_audit":False,
        "developer_candidate_redistribution_review_flag":False,
        "public_release_ready":False,
        "public_release_authorized":False,
        "no_production_psd_writer":True,
    }


def main() -> None:
    parser=argparse.ArgumentParser()
    parser.add_argument("--artifacts-dir",type=Path,required=True)
    parser.add_argument("--repository",type=Path,default=Path.cwd())
    parser.add_argument("--output",type=Path)
    args=parser.parse_args()
    result=audit(args.artifacts_dir.resolve(),args.repository.resolve())
    text=json.dumps(result,sort_keys=True,indent=2,ensure_ascii=False)+"\n"
    if args.output:
        args.output.parent.mkdir(parents=True,exist_ok=True)
        args.output.write_text(text)
    print(json.dumps({"technical_artifacts_audit":result["technical_artifacts_audit"],
                      "frozen_main_sha":MAIN,"cli_targets":list(TARGETS),
                      "managed_targets":list(TARGETS),"CI_isolated_targets":list(TARGETS),
                      "local_mac_recheck_steps":20,
                      "public_release_ready":False}))


if __name__=="__main__":
    try: main()
    except (KeyError,ValueError,OSError,zipfile.BadZipFile,
            subprocess.CalledProcessError) as error:
        sys.exit(f"EXACT_MAIN_ARTIFACT_AUDIT_FAILED: {error}")
